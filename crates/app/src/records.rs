//! The conventions that the record kinds share: workstreams, actions, commitments, persons and institutions
//! (ADR 0067, ADR 0068, ADR 0069).
//!
//! - A create takes an optional ID that the client chooses: a UUIDv7 (`record_id`). A taken ID is the field code `taken`.
//! - A change needs at least one field. A change without a field is `validation-failed` without field errors.
//! - A change compares the expected version before it checks the values.
//! - The app clock gives the time of each write. The store keeps the time that it gets.
//! - A read takes a `Principal`, so that an AI client reads what its member reads.
//! - A list pages by the local number of the records (`NumberCursor`).
//! - A record that a proposal can create or change shows its evidence (`Shown`), as far as the caller can read the
//!   sources (`access::source_reach`).

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::{
    self, ActionId, CommitmentId, InstitutionId, PersonId, ProposalId, SourceVersionId,
};
use uuid::Uuid;

use crate::access::{self, Principal, SourceReach};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::problem::FieldError;
use crate::store::StoreError;

/// Collects the field errors of one input, so that the caller sees all of them at once.
#[derive(Debug, Default)]
pub(crate) struct Checker(Vec<FieldError>);

impl Checker {
    /// The parsed value, or `None` after it records the error of `field`.
    pub(crate) fn parse<T, E>(
        &mut self,
        field: &'static str,
        input: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> Option<T> {
        parse(input)
            .map_err(|error| self.push(field, code(error)))
            .ok()
    }

    pub(crate) fn push(&mut self, field: &'static str, code: &'static str) {
        self.0.push(FieldError::new(field, code));
    }

    pub(crate) fn add(&mut self, error: FieldError) {
        self.0.push(error);
    }

    /// The value if no field has an error. A value without errors is never `None`.
    pub(crate) fn finish<T>(self, value: Option<T>) -> Result<T, Vec<FieldError>> {
        match value {
            Some(value) if self.0.is_empty() => Ok(value),
            _ => Err(self.0),
        }
    }
}

/// The result of a create in a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created<T> {
    Created(T),
    /// A record with this ID exists, in this organization or in another one.
    IdTaken,
}

/// The result of a change in a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed<T> {
    Changed(T),
    /// The scope has no such record.
    NotFound,
    /// The record has another version.
    VersionConflict,
}

/// The position after the last record of a page: its local number (ADR 0038, ADR 0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberCursor(pub u64);

/// The page of `items`, which hold one record more than `limit` if a next page exists.
pub(crate) fn page<T>(
    mut items: Vec<T>,
    limit: PageLimit,
    number: impl Fn(&T) -> u64,
) -> Page<T, NumberCursor> {
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| NumberCursor(number(last)));
    Page { items, next }
}

/// The ID of a new record: the UUIDv7 of the client, or a new one (ADR 0038).
pub(crate) fn record_id(id: Option<Uuid>) -> Result<Uuid, FieldError> {
    match id {
        Some(id) if !ids::is_record_id(id) => Err(FieldError::new("id", "not-uuid-v7")),
        Some(id) => Ok(id),
        None => Ok(Uuid::now_v7()),
    }
}

/// The audit event of a direct command on the record `record`.
pub(crate) fn audit(caller: &MemberCaller, action: AuditAction, record: Uuid) -> AuditEvent {
    AuditEvent::new(caller.actor(), action, Some(record), Some(caller.scope()))
}

/// A record that accepted proposals support with evidence (ADR 0068, ADR 0069).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordRef {
    Action(ActionId),
    Commitment(CommitmentId),
    Person(PersonId),
    Institution(InstitutionId),
}

/// A passage that supports one version of a record (ADR 0068).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordEvidenceView {
    /// The record version that the accepted change produced.
    pub record_version: RecordVersion,
    pub proposal_id: ProposalId,
    pub source_version_id: SourceVersionId,
    /// The capture time of the source version.
    pub captured_at: Timestamp,
    pub start_offset: u32,
    pub end_offset: u32,
    pub quote: String,
    pub page: Option<u32>,
}

/// The repository port for the evidence of records. The record stores extend it.
#[async_trait]
pub trait EvidenceStore: Debug + Send + Sync {
    /// The evidence of `records` whose source version `reach` can read, in the order of record version and offset.
    async fn evidence_of(
        &self,
        scope: OrgScope,
        records: &[RecordRef],
        reach: &SourceReach,
    ) -> Result<Vec<(RecordRef, RecordEvidenceView)>, StoreError>;
}

/// A record as one caller reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown<T> {
    pub record: T,
    /// The evidence of the accepted proposals that created or changed the record, as far as the caller can read
    /// their sources. A direct command adds none.
    pub evidence: Vec<RecordEvidenceView>,
    /// True if the caller can change the record now, by the rule of its kind.
    pub can_change: bool,
}

impl<T> Shown<T> {
    /// A record that a direct create made: a direct command carries no evidence (ADR 0068),
    /// and a new record has no earlier version.
    pub(crate) fn created(record: T, can_change: bool) -> Self {
        Self {
            record,
            evidence: Vec::new(),
            can_change,
        }
    }
}

/// The records with the evidence that `caller` can read, and with the right of `caller` to change each.
pub(crate) async fn shown<T>(
    caller: &impl Principal,
    records: Vec<T>,
    record_ref: impl Fn(&T) -> RecordRef,
    can_change: impl Fn(&T) -> bool,
    identity: &dyn IdentityStore,
    store: &dyn EvidenceStore,
) -> Result<Vec<Shown<T>>, StoreError> {
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let reach = access::source_reach(caller, identity).await?;
    let refs: Vec<RecordRef> = records.iter().map(&record_ref).collect();
    let mut evidence: HashMap<RecordRef, Vec<RecordEvidenceView>> = HashMap::new();
    for (record, passage) in store.evidence_of(caller.scope(), &refs, &reach).await? {
        evidence.entry(record).or_default().push(passage);
    }
    Ok(records
        .into_iter()
        .map(|record| Shown {
            evidence: evidence.remove(&record_ref(&record)).unwrap_or_default(),
            can_change: can_change(&record),
            record,
        })
        .collect())
}

/// One record with the evidence that `caller` can read, and with the right of `caller` to change it.
pub(crate) async fn shown_one<T>(
    caller: &impl Principal,
    record: T,
    record_ref: RecordRef,
    can_change: bool,
    identity: &dyn IdentityStore,
    store: &dyn EvidenceStore,
) -> Result<Shown<T>, StoreError> {
    let reach = access::source_reach(caller, identity).await?;
    let evidence = store
        .evidence_of(caller.scope(), &[record_ref], &reach)
        .await?
        .into_iter()
        .map(|(_, passage)| passage)
        .collect();
    Ok(Shown {
        record,
        evidence,
        can_change,
    })
}
