//! The approval of an exact draft version by an event manager (ADR 0051, ADR 0052).
//! An approved version never changes; a new draft is a new version, and its approval supersedes the older one.

use tada_domain::RecordVersion;
use tada_domain::ids::DocumentVersionId;

use super::{Approval, Approved, DocumentReads, VersionView};
use crate::access::{self, AccessError};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::MemberCaller;
use crate::clock::Clock;
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ApproveError {
    /// The version is not a draft version of the organization, or the caller has no event role in its event.
    #[error("the draft version does not exist or the caller cannot see it")]
    NotFound,
    /// Only an event manager approves (ADR 0052).
    #[error("the caller cannot approve documents in this event")]
    Forbidden,
    /// The document changed after the caller read it.
    #[error("the document has another version")]
    VersionConflict,
    /// The version is approved, superseded or archived, or a newer version is approved.
    #[error("the status of the version does not allow its approval")]
    InvalidTransition,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ApproveError {
    /// All codes that the approval can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::RecordVersionConflict,
        ProblemCode::InvalidTransition,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ApproveError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::InvalidTransition => ProblemCode::InvalidTransition,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AccessError> for ApproveError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

/// Approves the draft version `version_id`. `expected_version` is the record version of its document.
///
/// Only a member caller approves: an AI client never accepts anything (ADR 0040).
/// The caller needs the right to approve documents in the event of the document (ADR 0052).
pub async fn approve_version(
    caller: &MemberCaller,
    version_id: DocumentVersionId,
    expected_version: RecordVersion,
    stores: DocumentReads<'_>,
    clock: &dyn Clock,
) -> Result<VersionView, ApproveError> {
    let draft = stores
        .documents
        .draft(caller.scope(), version_id)
        .await?
        .ok_or(ApproveError::NotFound)?;
    if !access::event_access(caller, draft.event_id, stores.identity)
        .await?
        .can_approve_documents()
    {
        return Err(ApproveError::Forbidden);
    }
    let approval = Approval {
        version_id,
        expected_version,
        approved_by: caller.user_id(),
        approved_at: clock.now(),
    };
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::DocumentVersionApprove,
        Some(version_id.as_uuid()),
        Some(caller.scope()),
    );
    match stores
        .documents
        .approve(caller.scope(), &approval, &audit)
        .await?
    {
        Approved::Approved(version) => Ok(*version),
        Approved::NotFound => Err(ApproveError::NotFound),
        Approved::VersionConflict => Err(ApproveError::VersionConflict),
        Approved::InvalidTransition => Err(ApproveError::InvalidTransition),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_error_has_a_code_in_its_list() {
        for error in [
            ApproveError::NotFound,
            ApproveError::Forbidden,
            ApproveError::VersionConflict,
            ApproveError::InvalidTransition,
            ApproveError::Store(StoreError::Unavailable("test".into())),
            ApproveError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(ApproveError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
