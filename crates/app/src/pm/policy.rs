//! The reminder policy of an organization (ADR 0076). An owner or an admin reads and changes it.
//!
//! An organization without a stored policy has the defaults. A change also moves the schedule of the
//! daily check, in the same transaction.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::civil::Time;
use tada_domain::RecordVersion;
use tada_domain::reminders::PmPolicy;

use crate::access::Principal;
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The repository port of the policy.
#[async_trait]
pub trait PmPolicyStore: Debug + Send + Sync {
    /// The policy and its version. An organization without a stored policy has the defaults with version 1.
    async fn get(&self, scope: OrgScope) -> Result<(PmPolicy, RecordVersion), StoreError>;

    /// Replaces the policy if it has the version `expected`, records `audit` and writes the schedule of the
    /// daily check, all in one transaction. The schedule starts at the first slot after the database time.
    /// Returns the new version, or `None` on a version conflict.
    async fn set(
        &self,
        scope: OrgScope,
        policy: &PmPolicy,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<RecordVersion>, StoreError>;
}

/// The values of a policy change, as a client sends them. `set_pm_policy` checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmPolicyInput {
    /// The local time of the daily check.
    pub local_time: Time,
    /// The IANA time zone of the check, for example `Europe/Zurich`.
    pub time_zone: String,
    /// The days of the check: bit 0 is Monday, bit 6 is Sunday.
    pub weekdays: i64,
    pub quiet_start: Time,
    pub quiet_end: Time,
    pub cooldown_hours: i64,
    pub follow_up_working_days: i64,
    pub look_ahead_working_days: i64,
    pub paused: bool,
    pub ai_enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PmPolicyError {
    /// Only an owner or an admin reads or changes the policy.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the policy changed after the caller read it")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PmPolicyError {
    /// All codes of the policy commands and queries, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PmPolicyError {
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

/// The policy of the caller's organization. An owner or an admin can read it.
pub async fn get_pm_policy(
    caller: &MemberCaller,
    store: &dyn PmPolicyStore,
) -> Result<(PmPolicy, RecordVersion), PmPolicyError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(PmPolicyError::Forbidden);
    }
    Ok(store.get(caller.scope()).await?)
}

/// Replaces the policy of the caller's organization. An owner or an admin can do this.
/// The audit event names the organization and holds no values.
pub async fn set_pm_policy(
    caller: &MemberCaller,
    input: PmPolicyInput,
    expected_version: RecordVersion,
    store: &dyn PmPolicyStore,
) -> Result<(PmPolicy, RecordVersion), PmPolicyError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(PmPolicyError::Forbidden);
    }
    // A number outside `u8` is outside each range too.
    let small = |value: i64| u8::try_from(value).unwrap_or(u8::MAX);
    let policy = PmPolicy::new(
        input.local_time,
        &input.time_zone,
        small(input.weekdays),
        input.quiet_start,
        input.quiet_end,
        small(input.cooldown_hours),
        small(input.follow_up_working_days),
        small(input.look_ahead_working_days),
        input.paused,
        input.ai_enabled,
    )
    .map_err(|errors| {
        PmPolicyError::Invalid(
            errors
                .into_iter()
                .map(|error| FieldError::new(error.field, error.code))
                .collect(),
        )
    })?;
    let scope = caller.scope();
    // The policy has no ID of its own: the record ID is its organization.
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationSetPmPolicy,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    );
    let version = store
        .set(scope, &policy, expected_version, &audit)
        .await?
        .ok_or(PmPolicyError::VersionConflict)?;
    Ok((policy, version))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tada_domain::identity::OrganizationRole;
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use super::*;

    /// A policy store in memory that counts the writes.
    #[derive(Debug, Default)]
    struct Memory {
        stored: Mutex<Option<(PmPolicy, RecordVersion)>>,
        writes: Mutex<u32>,
    }

    #[async_trait]
    impl PmPolicyStore for Memory {
        async fn get(&self, _: OrgScope) -> Result<(PmPolicy, RecordVersion), StoreError> {
            Ok(self
                .stored
                .lock()
                .unwrap()
                .clone()
                .unwrap_or((PmPolicy::default(), RecordVersion::FIRST)))
        }

        async fn set(
            &self,
            scope: OrgScope,
            policy: &PmPolicy,
            expected: RecordVersion,
            _: &AuditEvent,
        ) -> Result<Option<RecordVersion>, StoreError> {
            let (_, current) = self.get(scope).await?;
            if current != expected {
                return Ok(None);
            }
            let next = RecordVersion::new(current.get() + 1).unwrap();
            *self.stored.lock().unwrap() = Some((policy.clone(), next));
            *self.writes.lock().unwrap() += 1;
            Ok(Some(next))
        }
    }

    fn caller(role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            OrganizationId::from_uuid(Uuid::from_u128(2)),
            role,
        )
    }

    fn input() -> PmPolicyInput {
        PmPolicyInput {
            local_time: Time::constant(7, 45, 0, 0),
            time_zone: "Europe/Berlin".to_owned(),
            weekdays: 0b001_1111,
            quiet_start: Time::constant(21, 0, 0, 0),
            quiet_end: Time::constant(7, 0, 0, 0),
            cooldown_hours: 24,
            follow_up_working_days: 3,
            look_ahead_working_days: 0,
            paused: false,
            ai_enabled: true,
        }
    }

    #[tokio::test]
    async fn member_cannot_set_policy() {
        let store = Memory::default();
        let member = caller(OrganizationRole::Member);
        assert!(matches!(
            set_pm_policy(&member, input(), RecordVersion::FIRST, &store).await,
            Err(PmPolicyError::Forbidden)
        ));
        assert!(matches!(
            get_pm_policy(&member, &store).await,
            Err(PmPolicyError::Forbidden)
        ));
        assert_eq!(*store.writes.lock().unwrap(), 0);
        for role in [OrganizationRole::Admin, OrganizationRole::Owner] {
            let (_, version) = get_pm_policy(&caller(role), &store).await.unwrap();
            set_pm_policy(&caller(role), input(), version, &store)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn stale_version_conflicts() {
        let store = Memory::default();
        let owner = caller(OrganizationRole::Owner);
        let (policy, version) = set_pm_policy(&owner, input(), RecordVersion::FIRST, &store)
            .await
            .unwrap();
        assert_eq!(version, RecordVersion::new(2).unwrap());
        assert_eq!(policy.schedule.zone.as_str(), "Europe/Berlin");
        assert!(matches!(
            set_pm_policy(&owner, input(), RecordVersion::FIRST, &store).await,
            Err(PmPolicyError::VersionConflict)
        ));
        assert_eq!(*store.writes.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn invalid_time_zone_is_a_field_error() {
        let store = Memory::default();
        let owner = caller(OrganizationRole::Owner);
        let wrong = PmPolicyInput {
            time_zone: "Mars/Olympus".to_owned(),
            cooldown_hours: 1000,
            weekdays: 0,
            look_ahead_working_days: -1,
            ..input()
        };
        let Err(PmPolicyError::Invalid(errors)) =
            set_pm_policy(&owner, wrong, RecordVersion::FIRST, &store).await
        else {
            panic!("the values are invalid");
        };
        let found: Vec<(&str, &str)> = errors
            .iter()
            .map(|error| (error.field.as_ref(), error.code))
            .collect();
        assert_eq!(
            found,
            [
                ("time_zone", "invalid-time-zone"),
                ("weekdays", "out-of-range"),
                ("cooldown_hours", "out-of-range"),
                ("look_ahead_working_days", "out-of-range"),
            ]
        );
        assert_eq!(*store.writes.lock().unwrap(), 0);
    }
}
