//! The retention periods of an organization and the daily retention job (ADR 0071).
//!
//! An owner sets two periods in months: one for closed events and one for sources that belong to no event.
//! A period that is not set means that retention is off for it. The job redacts what is older than its period,
//! one target in one transaction (ADR 0072), and a failed target does not stop the others.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::OrganizationId;
use tada_domain::redaction::{RedactionTarget, RedactionTargetKind};

use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::blobs::BlobStore;
use crate::caller::{JobRunner, MemberCaller, OrgScope, ServiceCaller};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::redaction::{RedactionError, RedactionStore, redact_for_retention};
use crate::store::StoreError;

/// The longest period, in months.
pub const MAX_MONTHS: u8 = 120;

/// A period of 1 to 120 months.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Months(u8);

impl Months {
    pub fn new(months: i64) -> Option<Self> {
        u8::try_from(months)
            .ok()
            .filter(|months| (1..=MAX_MONTHS).contains(months))
            .map(Self)
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// The two periods of an organization. `None` means that retention is off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetentionPeriods {
    /// A closed event is redacted this long after it closed.
    pub closed_event: Option<Months>,
    /// A source that belongs to no event is redacted this long after its creation.
    pub source: Option<Months>,
}

/// The periods with the record version. `set_retention` needs the version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub periods: RetentionPeriods,
    /// Periods that no owner changed have version 1.
    pub version: RecordVersion,
}

/// The repository port of the retention periods and of the targets that are due.
#[async_trait]
pub trait RetentionStore: Debug + Send + Sync {
    async fn get(&self, scope: OrgScope) -> Result<Retention, StoreError>;

    /// Replaces both periods and records `audit` in one transaction, if the periods have the version
    /// `expected_version`. Returns `None` on a version conflict.
    async fn set(
        &self,
        scope: OrgScope,
        periods: RetentionPeriods,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<Retention>, StoreError>;

    /// The closed events and the sources without event whose period ended before `now`.
    /// A target that is redacted already is not due, so a run that stopped resumes with the rest.
    /// Infrastructure query (ADR 0039): the job covers all organizations, and each target names its own.
    async fn due_for_retention(
        &self,
        caller: &ServiceCaller<JobRunner>,
        now: Timestamp,
    ) -> Result<Vec<(OrganizationId, RedactionTarget)>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    /// Only an owner changes the periods. An owner or an admin reads them.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the periods changed after the caller read them")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RetentionError {
    /// All codes of the retention commands and queries, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for RetentionError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The periods of the caller's organization. An owner or an admin can read them.
pub async fn get_retention(
    caller: &MemberCaller,
    store: &dyn RetentionStore,
) -> Result<Retention, RetentionError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(RetentionError::Forbidden);
    }
    Ok(store.get(caller.scope()).await?)
}

/// Replaces both periods of the caller's organization. Only an owner can do this.
/// A value of `None` switches the period off. The audit event holds the two numbers and no content.
pub async fn set_retention(
    caller: &MemberCaller,
    closed_event_months: Option<i64>,
    source_months: Option<i64>,
    expected_version: RecordVersion,
    store: &dyn RetentionStore,
) -> Result<Retention, RetentionError> {
    if !access::is_owner(caller) {
        return Err(RetentionError::Forbidden);
    }
    let mut errors = Vec::new();
    let mut period = |field: &'static str, months: Option<i64>| match months.map(Months::new) {
        None => None,
        Some(Some(months)) => Some(months),
        Some(None) => {
            errors.push(FieldError::new(field, "out-of-range"));
            None
        }
    };
    let periods = RetentionPeriods {
        closed_event: period("closed_event_months", closed_event_months),
        source: period("source_months", source_months),
    };
    if !errors.is_empty() {
        return Err(RetentionError::Invalid(errors));
    }
    let scope = caller.scope();
    // The periods have no ID of their own: the record ID is their organization.
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationSetRetention,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    )
    .with_retention(periods);
    store
        .set(scope, periods, expected_version, &audit)
        .await?
        .ok_or(RetentionError::VersionConflict)
}

/// A target that the run could not redact. It holds IDs and the problem code, and no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailedTarget {
    pub organization_id: OrganizationId,
    pub target: RedactionTarget,
    pub code: ProblemCode,
}

/// What one run of the job did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RetentionReport {
    /// The number of closed events that the run redacted.
    pub events: usize,
    /// The number of sources that the run redacted.
    pub sources: usize,
    /// The number of targets that no longer qualified when the run reached them: an event that was reopened,
    /// or a source that an event holds now. The next run does not find them due.
    pub skipped: usize,
    /// The targets that failed. The next run tries them again.
    pub failed: Vec<FailedTarget>,
}

impl RetentionReport {
    /// True if no target failed. The worker marks the sweep done then, and failed otherwise.
    pub fn completed(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Redacts each target whose retention period ended before `now`, as the service `job-runner`.
/// A target that fails does not stop the others: the run lists it in `failed` and goes on.
/// The error is for the query of the due targets only.
pub async fn run(
    store: &dyn RetentionStore,
    redaction: &dyn RedactionStore,
    blobs: &dyn BlobStore,
    now: Timestamp,
) -> Result<RetentionReport, StoreError> {
    let caller = ServiceCaller::<JobRunner>::new();
    let mut report = RetentionReport::default();
    for (organization_id, target) in store.due_for_retention(&caller, now).await? {
        let scope = caller.scope(organization_id);
        match redact_for_retention(scope, target, redaction, blobs).await {
            Ok(_) => match target.kind {
                RedactionTargetKind::Event => report.events += 1,
                _ => report.sources += 1,
            },
            Err(RedactionError::InvalidTransition) => report.skipped += 1,
            Err(error) => report.failed.push(FailedTarget {
                organization_id,
                target,
                code: error.code(),
            }),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_period_is_one_to_120_months() {
        assert_eq!(Months::new(0), None);
        assert_eq!(Months::new(-1), None);
        assert_eq!(Months::new(121), None);
        assert_eq!(Months::new(300), None);
        assert_eq!(Months::new(1).map(Months::get), Some(1));
        assert_eq!(Months::new(120).map(Months::get), Some(120));
    }
}
