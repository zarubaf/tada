//! The privacy notice of the organization (ADR 0045).
//!
//! An owner writes the text in Markdown. Without a text, the web client shows the template.
//! The notice holds no personal data of members: the club writes it for them.

use std::fmt::Debug;

use async_trait::async_trait;
use tada_domain::RecordVersion;

use crate::access::Principal;
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope, OrganizationRole};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The longest notice, in characters.
pub const MAX_LENGTH: usize = 20_000;

/// The notice of one organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyNotice {
    /// The Markdown text of the owner. `None` means that the template applies.
    pub markdown: Option<String>,
    /// A notice that no owner changed has version 1.
    pub version: RecordVersion,
}

/// The repository port of the privacy notice.
#[async_trait]
pub trait PrivacyStore: Debug + Send + Sync {
    /// The notice of the organization of `scope`.
    async fn get(&self, scope: OrgScope) -> Result<PrivacyNotice, StoreError>;

    /// Replaces the text and records `audit` in one transaction, if the notice has the version
    /// `expected_version`. Returns `None` on a version conflict.
    async fn set(
        &self,
        scope: OrgScope,
        markdown: Option<&str>,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<PrivacyNotice>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    /// Only an owner changes the notice.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the notice changed after the caller read it")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PrivacyError {
    /// All codes of the privacy commands and queries, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PrivacyError {
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

/// The notice of the caller's organization. Each member can read it.
pub async fn get_privacy_notice(
    caller: &MemberCaller,
    store: &dyn PrivacyStore,
) -> Result<PrivacyNotice, PrivacyError> {
    Ok(store.get(caller.scope()).await?)
}

/// Replaces the notice of the caller's organization. Only an owner can do this.
/// `None` gives the template back. The audit event names the organization and holds no text.
pub async fn set_privacy_notice(
    caller: &MemberCaller,
    markdown: Option<String>,
    expected_version: RecordVersion,
    store: &dyn PrivacyStore,
) -> Result<PrivacyNotice, PrivacyError> {
    if caller.organization_role() != OrganizationRole::Owner {
        return Err(PrivacyError::Forbidden);
    }
    if let Some(text) = &markdown {
        let code = if text.trim().is_empty() {
            Some("empty")
        } else if text.chars().count() > MAX_LENGTH {
            Some("too-long")
        } else {
            None
        };
        if let Some(code) = code {
            return Err(PrivacyError::Invalid(vec![FieldError::new(
                "markdown", code,
            )]));
        }
    }
    let scope = caller.scope();
    // The notice has no ID of its own: the record ID is its organization.
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationSetPrivacyNotice,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    );
    store
        .set(scope, markdown.as_deref(), expected_version, &audit)
        .await?
        .ok_or(PrivacyError::VersionConflict)
}
