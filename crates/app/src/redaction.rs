//! Legal redaction and the deletion journal (ADRs 0045 and 0072).
//!
//! A redaction replaces the content of its targets with a tombstone and keeps their IDs. This module owns the
//! rules: who can redact, which reason applies, and what happens after the commit. The `RedactionStore` port
//! writes the tombstones; only it switches on the redaction setting of the immutability triggers.
//!
//! Each redaction adds the job `write-redaction-journal` in its transaction. The job writes one journal object
//! into the object storage, outside the database, so that `replay` can apply the redactions again after a
//! restore of the database.

use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_domain::ids::{OrganizationId, RedactionId};
use tada_domain::redaction::{RedactionReason, RedactionTarget, RedactionTargetKind};
use uuid::Uuid;

use crate::access;
use crate::blobs::{BlobError, BlobKey, BlobStore, ByteStream, JOURNAL_PREFIX};
use crate::caller::{Actor, JobRunner, MemberCaller, OrgScope, RedactionReplay, ServiceCaller};
use crate::jobs::{Job, JobFailed, JobHandler, JobWarning};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The job kind that writes the journal object of one redaction.
/// Payload version 1 is `{"redaction_id": "<uuid>"}`.
pub const JOURNAL_JOB: &str = "write-redaction-journal";

/// A journal object is small; a larger object is not one that the job wrote.
const JOURNAL_MAX_BYTES: u64 = 4096;

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

/// One redaction as the deletion journal holds it. It holds IDs, the reason and the time, and no content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEntry {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub reason: String,
    pub redacted_at: Timestamp,
}

/// A journal entry with its values checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redaction {
    pub id: RedactionId,
    pub organization_id: OrganizationId,
    pub target: RedactionTarget,
    pub reason: RedactionReason,
    pub redacted_at: Timestamp,
}

impl Redaction {
    pub fn entry(&self) -> JournalEntry {
        JournalEntry {
            id: self.id.as_uuid(),
            organization_id: self.organization_id.as_uuid(),
            target_kind: self.target.kind.as_str().to_owned(),
            target_id: self.target.id,
            reason: self.reason.as_str().to_owned(),
            redacted_at: self.redacted_at,
        }
    }

    /// The redaction of an entry, or `None` if a value is not valid.
    pub fn from_entry(entry: &JournalEntry) -> Option<Self> {
        Some(Self {
            id: RedactionId::from_uuid(entry.id),
            organization_id: OrganizationId::from_uuid(entry.organization_id),
            target: RedactionTarget {
                kind: entry.target_kind.parse().ok()?,
                id: entry.target_id,
            },
            reason: entry.reason.parse().ok()?,
            redacted_at: entry.redacted_at,
        })
    }
}

/// Why the store redacted nothing. The transaction changed no row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A target does not exist in the organization.
    NotFound,
    /// The target is an event that is not closed. Only a closed event can be redacted (ADR 0071).
    EventNotClosed,
}

/// What a replay of one journal entry found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replayed {
    /// The target had content again. The store redacted it and returns the objects to delete.
    Applied(Vec<BlobKey>),
    /// The target is redacted.
    Already,
    /// The target does not exist, for example in a backup that is older than the target.
    Missing,
}

/// The repository port of legal redactions. Only its implementation switches on the redaction setting
/// of the immutability triggers (ADR 0072).
#[async_trait]
pub trait RedactionStore: Debug + Send + Sync {
    /// Redacts each target and the content that copies it, in one transaction: the tombstones, a `redaction`
    /// row, an audit event `<kind>.redact` with the reason, and a journal job for each target with content.
    /// An open proposal that the redaction empties is rejected with the actor as reviewer.
    /// Returns a refusal if a target does not exist in the organization or is an open event; then nothing changes.
    async fn redact(
        &self,
        scope: OrgScope,
        targets: &[RedactionTarget],
        reason: RedactionReason,
        actor: &Actor,
    ) -> Result<Result<RedactionOutcome, Refused>, StoreError>;

    /// True if the target is redacted, false if it has content, and `None` if it does not exist in the organization.
    async fn redacted(
        &self,
        scope: OrgScope,
        target: RedactionTarget,
    ) -> Result<Option<bool>, StoreError>;

    /// The redaction with the ID, as its journal object holds it.
    async fn redaction(
        &self,
        scope: OrgScope,
        id: RedactionId,
    ) -> Result<Option<Redaction>, StoreError>;

    /// Applies a redaction of the journal again, as `redact` does, with its ID, reason and time.
    /// An event that the restored database holds open is closed first, with the audit event `event.close`.
    /// It adds no journal job: the journal holds the redaction already.
    async fn replay(
        &self,
        scope: OrgScope,
        redaction: &Redaction,
        actor: &Actor,
    ) -> Result<Replayed, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RedactionError {
    /// Only an owner redacts.
    #[error("the caller cannot do this")]
    Forbidden,
    /// A target does not exist in the caller's organization.
    #[error("a target does not exist")]
    NotFound,
    /// The target is an event that is not closed.
    #[error("the event is not closed")]
    InvalidTransition,
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
        ProblemCode::InvalidTransition,
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
            Self::InvalidTransition => ProblemCode::InvalidTransition,
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
        .map_err(refused)?;
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
        .map_err(refused)?;
    Ok(finish(outcome, blobs).await)
}

fn refused(refused: Refused) -> RedactionError {
    match refused {
        Refused::NotFound => RedactionError::NotFound,
        Refused::EventNotClosed => RedactionError::InvalidTransition,
    }
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

/// The handler of the journal job: it writes the journal object of one redaction.
/// A journal object is append-only: the handler never writes an object that exists.
pub struct WriteRedactionJournal {
    store: Arc<dyn RedactionStore>,
    blobs: Arc<dyn BlobStore>,
    caller: ServiceCaller<JobRunner>,
}

impl Debug for WriteRedactionJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteRedactionJournal")
            .finish_non_exhaustive()
    }
}

impl WriteRedactionJournal {
    pub fn new(store: Arc<dyn RedactionStore>, blobs: Arc<dyn BlobStore>) -> Self {
        Self {
            store,
            blobs,
            caller: ServiceCaller::new(),
        }
    }
}

#[async_trait]
impl JobHandler for WriteRedactionJournal {
    fn kind(&self) -> &'static str {
        JOURNAL_JOB
    }

    async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed> {
        if job.version != 1 {
            return Err(JobFailed(format!(
                "unknown payload version {}",
                job.version
            )));
        }
        let id = job
            .payload
            .get("redaction_id")
            .and_then(|id| id.as_str())
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(RedactionId::from_uuid)
            .ok_or_else(|| JobFailed("the payload has no redaction ID".into()))?;
        let organization_id = job
            .organization_id
            .ok_or_else(|| JobFailed("the job has no organization".into()))?;
        // The job names its organization, so the handler reads the redaction in it only (ADR 0039).
        let redaction = self
            .store
            .redaction(self.caller.scope(organization_id), id)
            .await
            .map_err(|error| JobFailed(format!("the store failed: {error}")))?
            .ok_or_else(|| {
                JobFailed("the redaction does not exist in the organization of the job".into())
            })?;
        let key = BlobKey::journal(organization_id, id);
        if self.blobs.head(&key).await.map_err(blob_failed)?.is_some() {
            return Ok(None);
        }
        let json = serde_json::to_vec(&redaction.entry())
            .map_err(|error| JobFailed(format!("cannot write the journal entry: {error}")))?;
        let body: ByteStream = Box::pin(futures::stream::iter([Ok(Bytes::from(json))]));
        self.blobs
            .put(&key, body, JOURNAL_MAX_BYTES)
            .await
            .map_err(blob_failed)?;
        Ok(None)
    }
}

fn blob_failed(error: BlobError) -> JobFailed {
    JobFailed(format!("the object storage failed: {error}"))
}

/// What a replay of the journal did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReplayReport {
    /// The journal objects that the replay read.
    pub read: usize,
    /// The redactions that the replay applied again.
    pub applied: usize,
    /// The redactions whose targets were redacted already.
    pub already: usize,
    /// The redactions whose targets do not exist in the database.
    pub missing: usize,
    /// The objects that the delete after a commit could not remove. The sweep removes them later.
    pub undeleted_objects: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error(transparent)]
    Blobs(#[from] BlobError),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// An object under the journal prefix is not a journal object that the job wrote.
    #[error("the journal object {0} is not valid")]
    InvalidObject(String),
}

/// Applies each redaction of the deletion journal that the database does not hold, in the order of the
/// redaction times (ADR 0072). The operator runs it after each restore of the database. It is idempotent.
pub async fn replay(
    store: &dyn RedactionStore,
    blobs: &dyn BlobStore,
) -> Result<ReplayReport, ReplayError> {
    let mut redactions = Vec::new();
    let mut objects = blobs.list(JOURNAL_PREFIX);
    while let Some(object) = objects.try_next().await? {
        redactions.push(read_entry(blobs, &object.key).await?);
    }
    redactions.sort_by_key(|redaction| (redaction.redacted_at, redaction.id));

    let caller = ServiceCaller::<RedactionReplay>::new();
    let mut report = ReplayReport {
        read: redactions.len(),
        ..ReplayReport::default()
    };
    for redaction in &redactions {
        // The journal object names its organization, so the replay writes in it only (ADR 0039).
        let scope = caller.scope(redaction.organization_id);
        match store.replay(scope, redaction, &caller.actor()).await? {
            Replayed::Applied(keys) => {
                report.applied += 1;
                report.undeleted_objects += delete_objects(blobs, &keys).await;
            }
            Replayed::Already => report.already += 1,
            Replayed::Missing => report.missing += 1,
        }
    }
    Ok(report)
}

/// Reads and checks one journal object. Its key and its content must name the same redaction.
async fn read_entry(blobs: &dyn BlobStore, name: &str) -> Result<Redaction, ReplayError> {
    let invalid = || ReplayError::InvalidObject(name.to_owned());
    let (organization_id, id) = BlobKey::parse_journal(name).ok_or_else(invalid)?;
    let key = BlobKey::journal(organization_id, id);
    let mut stream = blobs.get(&key).await?.ok_or_else(invalid)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.try_next().await.map_err(BlobError::Upload)? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > JOURNAL_MAX_BYTES {
            return Err(invalid());
        }
    }
    let entry: JournalEntry = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let redaction = Redaction::from_entry(&entry).ok_or_else(invalid)?;
    if redaction.id != id || redaction.organization_id != organization_id {
        return Err(invalid());
    }
    Ok(redaction)
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn a_journal_entry_holds_ids_the_reason_and_the_time_only() {
        let redaction = Redaction {
            id: RedactionId::from_uuid(Uuid::from_u128(2)),
            organization_id: OrganizationId::from_uuid(Uuid::from_u128(3)),
            target: target(RedactionTargetKind::SourceItem),
            reason: RedactionReason::Request,
            redacted_at: "2030-05-18T08:00:00Z".parse().unwrap(),
        };
        let json = serde_json::to_value(redaction.entry()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000002",
                "organization_id": "00000000-0000-0000-0000-000000000003",
                "target_kind": "source-item",
                "target_id": "00000000-0000-0000-0000-000000000001",
                "reason": "request",
                "redacted_at": "2030-05-18T08:00:00Z",
            })
        );
        let entry: JournalEntry = serde_json::from_value(json).unwrap();
        assert_eq!(Redaction::from_entry(&entry), Some(redaction));
        let unknown = JournalEntry {
            target_kind: "account".to_owned(),
            ..entry
        };
        assert_eq!(Redaction::from_entry(&unknown), None);
    }
}
