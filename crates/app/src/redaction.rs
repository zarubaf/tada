//! Legal redaction (ADRs 0045 and 0072).
//!
//! A redaction replaces the content of its targets with a tombstone and keeps their IDs. This module owns the
//! rules: who can redact, which reason applies, and what happens after the commit. The `RedactionStore` port
//! writes the tombstones; only it switches on the redaction setting of the immutability triggers.
//!
//! Each redaction adds the job `write-redaction-journal` in its transaction (ADR 0054).

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::ids::{OrganizationId, RedactionId};
use tada_domain::redaction::{RedactionReason, RedactionTarget, RedactionTargetKind};

use crate::access;
use crate::blobs::{BlobKey, BlobStore};
use crate::caller::{Actor, JobRunner, MemberCaller, OrgScope, ServiceCaller};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The job kind that writes the journal object of one redaction.
/// Payload version 1 is `{"redaction_id": "<uuid>"}`.
pub const JOURNAL_JOB: &str = "write-redaction-journal";

/// What one call of `RedactionStore::redact` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionOutcome {
    /// The redactions of the targets that had content.
    pub applied: Vec<RedactionId>,
    /// The number of targets that were redacted already.
    pub already: usize,
    /// The objects of the redacted upload versions. They go after the commit.
    pub blob_keys: Vec<BlobKey>,
}

/// One redaction: its target, its reason and its time. It holds no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redaction {
    pub id: RedactionId,
    pub organization_id: OrganizationId,
    pub target: RedactionTarget,
    pub reason: RedactionReason,
    pub redacted_at: Timestamp,
}

/// The repository port of legal redactions. Only its implementation switches on the redaction setting
/// of the immutability triggers (ADR 0072).
#[async_trait]
pub trait RedactionStore: Debug + Send + Sync {
    /// Redacts each target and the content that copies it, in one transaction: the tombstones, a `redaction`
    /// row, an audit event `<kind>.redact` with the reason, and a journal job for each target with content.
    /// An open proposal that the redaction empties is rejected with the actor as reviewer.
    /// Returns `None` if a target does not exist in the organization; then nothing changes.
    async fn redact(
        &self,
        scope: OrgScope,
        targets: &[RedactionTarget],
        reason: RedactionReason,
        actor: &Actor,
    ) -> Result<Option<RedactionOutcome>, StoreError>;

    /// True if the target is redacted. False also if it does not exist.
    async fn is_redacted(
        &self,
        scope: OrgScope,
        target: RedactionTarget,
    ) -> Result<bool, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RedactionError {
    /// Only an owner redacts.
    #[error("the caller cannot do this")]
    Forbidden,
    /// A target does not exist in the caller's organization.
    #[error("a target does not exist")]
    NotFound,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RedactionError {
    /// All codes of the redaction command, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for RedactionError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
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

/// What a redaction did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    pub applied: Vec<RedactionId>,
    pub already: usize,
    /// The number of objects that the delete after the commit could not remove.
    /// The sweep of orphan objects removes them later (ADR 0074); the adapter logs the number.
    pub undeleted_objects: usize,
}

/// Redacts the targets on the request of a person (ADR 0072). Only an owner can do this.
/// The type takes a member only, so an AI client cannot redact:
///
/// ```compile_fail
/// use tada_app::blobs::BlobStore;
/// use tada_app::caller::AiCaller;
/// use tada_app::redaction::{RedactionStore, redact};
///
/// async fn by_ai(caller: &AiCaller, store: &dyn RedactionStore, blobs: &dyn BlobStore) {
///     let _ = redact(caller, &[], store, blobs).await;
/// }
/// ```
///
/// The target `event` is for the retention job only. All targets change in one transaction, and a redacted
/// target stays as it is. The objects of redacted upload versions go after the commit.
pub async fn redact(
    caller: &MemberCaller,
    targets: &[RedactionTarget],
    store: &dyn RedactionStore,
    blobs: &dyn BlobStore,
) -> Result<Redacted, RedactionError> {
    if !access::is_owner(caller) {
        return Err(RedactionError::Forbidden);
    }
    let targets = checked(targets)?;
    let outcome = store
        .redact(
            caller.scope(),
            &targets,
            RedactionReason::Request,
            &caller.actor(),
        )
        .await?
        .ok_or(RedactionError::NotFound)?;
    Ok(finish(outcome, blobs).await)
}

/// Redacts one target at the end of its retention period (ADR 0071), as the service `job-runner`.
pub async fn redact_for_retention(
    scope: OrgScope,
    target: RedactionTarget,
    store: &dyn RedactionStore,
    blobs: &dyn BlobStore,
) -> Result<Redacted, RedactionError> {
    let outcome = store
        .redact(
            scope,
            &[target],
            RedactionReason::Retention,
            &ServiceCaller::<JobRunner>::new().actor(),
        )
        .await?
        .ok_or(RedactionError::NotFound)?;
    Ok(finish(outcome, blobs).await)
}

/// The targets without repetitions, sorted by kind and ID. An empty list and the target `event` are invalid.
/// The store locks the targets in this order, so two redactions of the same targets cannot deadlock.
fn checked(targets: &[RedactionTarget]) -> Result<Vec<RedactionTarget>, RedactionError> {
    if targets.is_empty() {
        return Err(RedactionError::Invalid(vec![FieldError::new(
            "targets", "empty",
        )]));
    }
    let errors: Vec<FieldError> = targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.kind == RedactionTargetKind::Event)
        .map(|(index, _)| FieldError::new(format!("targets/{index}/kind"), "invalid"))
        .collect();
    if !errors.is_empty() {
        return Err(RedactionError::Invalid(errors));
    }
    let mut targets = targets.to_vec();
    targets.sort_unstable();
    targets.dedup();
    Ok(targets)
}

/// Deletes the objects of the redacted upload versions after the commit.
async fn finish(outcome: RedactionOutcome, blobs: &dyn BlobStore) -> Redacted {
    Redacted {
        undeleted_objects: delete_objects(blobs, &outcome.blob_keys).await,
        applied: outcome.applied,
        already: outcome.already,
    }
}

/// Deletes each object and returns the number of failed deletes. The delete is best effort: an object that
/// stays has no version with content, so the sweep of orphan objects deletes it (ADR 0074).
async fn delete_objects(blobs: &dyn BlobStore, keys: &[BlobKey]) -> usize {
    let mut failed = 0;
    for key in keys {
        if blobs.delete(key).await.is_err() {
            failed += 1;
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn target(kind: RedactionTargetKind) -> RedactionTarget {
        RedactionTarget {
            kind,
            id: Uuid::from_u128(1),
        }
    }

    #[test]
    fn checked_refuses_an_empty_list_and_the_event_and_drops_repetitions() {
        assert!(matches!(
            checked(&[]),
            Err(RedactionError::Invalid(errors)) if errors[0].code == "empty"
        ));
        let event = checked(&[
            target(RedactionTargetKind::Person),
            target(RedactionTargetKind::Event),
        ]);
        assert!(matches!(
            event,
            Err(RedactionError::Invalid(errors)) if errors[0].field == "targets/1/kind"
        ));
        let person = target(RedactionTargetKind::Person);
        assert_eq!(checked(&[person, person]).unwrap(), vec![person]);
    }

    /// Two owners who redact the same targets in another order lock them in one order.
    #[test]
    fn checked_sorts_the_targets_by_kind_and_id() {
        let proposal = |id| RedactionTarget {
            kind: RedactionTargetKind::Proposal,
            id: Uuid::from_u128(id),
        };
        let source = target(RedactionTargetKind::SourceItem);
        let sorted = checked(&[proposal(2), source, proposal(1)]).unwrap();
        assert_eq!(
            sorted,
            checked(&[proposal(1), proposal(2), source]).unwrap()
        );
        assert_eq!(sorted[1..], [proposal(1), proposal(2)]);
    }
}
