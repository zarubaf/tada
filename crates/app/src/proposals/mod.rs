//! The `CreateChangeset` command (ADR 0050): members and their AI clients propose changes with evidence.
//!
//! A proposal never changes accepted state. A member with the right to review applies it later.

mod input;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::facts::{
    ChoiceValue, FactState, FieldKey, FieldScope, FieldStatus, Label, ValueType,
};
use tada_domain::ids::{
    self, ChangesetId, EventId, FieldDefinitionId, ProposalId, SourceVersionId,
};
use tada_domain::proposals::{DependencyError, Operation, Proposal, Reason, check_dependencies};
use tada_domain::sources::{Passage, PassageError, SourceText};
use uuid::Uuid;

pub use self::input::{
    ChoiceInput, FactStateInput, GranularityInput, NewChangeset, NewProposal, OperationInput,
    PassageInput, ReferenceTargetInput, ValueInput, ValueTypeInput,
};
use self::input::{text_error_code, value_error_code};
use crate::access::{self, AccessError, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{Actor, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::facts::FactStore;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// A caller that can create proposals (ADR 0039): a member, or an AI client of a member.
/// A service identity cannot propose. The Telegram gateway proposes through a `MemberCaller` with the channel Telegram.
pub trait MayPropose: Principal {
    /// The author of the changeset and of its audit event.
    fn actor(&self) -> Actor;
}

impl MayPropose for MemberCaller {
    fn actor(&self) -> Actor {
        MemberCaller::actor(self)
    }
}

/// A stored changeset with its proposals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changeset {
    pub id: ChangesetId,
    /// `None` for a changeset of the organization, for example one that creates an event.
    pub event_id: Option<EventId>,
    pub author: Actor,
    /// The source version of the intake text. The evidence of each proposal points into it.
    pub source_version_id: SourceVersionId,
    pub created_at: Timestamp,
    pub proposals: Vec<Proposal>,
}

/// The result of `ProposalStore::insert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inserted {
    Inserted,
    /// A changeset or a proposal with one of the IDs exists. The store changed nothing.
    IdTaken,
}

/// The repository port for changesets and proposals. Each method that reads or writes organization data stays inside `scope`.
#[async_trait]
pub trait ProposalStore: Debug + Send + Sync {
    /// The IDs of `ids` that a changeset, a proposal, an event, a field definition or an open question has,
    /// in any organization. A new record ID must be free in the whole installation (ADR 0038).
    async fn taken_ids(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError>;

    /// Stores `source` as a source version of the kind `member-text` with the ID `changeset.source_version_id`,
    /// then the changeset with its proposals, their dependencies and their evidence, then `audit`.
    /// It writes all of them in one transaction, or nothing.
    async fn insert(
        &self,
        scope: OrgScope,
        changeset: &Changeset,
        source: &SourceText,
        audit: &AuditEvent,
    ) -> Result<Inserted, StoreError>;
}

/// The ports that `create_changeset` reads and writes.
#[derive(Debug, Clone, Copy)]
pub struct ProposeStores<'a> {
    pub identity: &'a dyn IdentityStore,
    pub facts: &'a dyn FactStore,
    pub proposals: &'a dyn ProposalStore,
}

#[derive(Debug, thiserror::Error)]
pub enum ProposeError {
    /// The event is not in the caller's organization, or the caller has no event role in it.
    #[error("the event does not exist or the caller cannot see it")]
    NotFound,
    /// A viewer cannot propose (ADR 0052). Only owners and admins propose for the organization, for example a new event.
    #[error("the caller cannot propose here")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ProposeError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl From<AccessError> for ProposeError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

impl CommandError for ProposeError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
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

/// Creates a changeset of proposals with their evidence (ADR 0050).
///
/// 1. A changeset of an event needs the right to propose in the event (ADR 0052).
///    A changeset of the organization, for example a new event, needs an owner or admin as principal.
/// 2. Each proposal has at least one passage, and each passage matches the source text (ADR 0040).
/// 3. Each new ID is a UUIDv7 and is free (ADR 0038).
/// 4. The dependencies stay inside the changeset, have no cycle, and include the proposals that create the records that a proposal uses.
/// 5. Each value matches the value type of its field. A new field of the same changeset counts.
///
/// It stores the source text as a source version, the changeset and an audit event in one transaction.
/// If a check fails, it stores nothing.
pub async fn create_changeset(
    caller: &impl MayPropose,
    input: NewChangeset,
    stores: ProposeStores<'_>,
    clock: &dyn Clock,
) -> Result<Changeset, ProposeError> {
    let scope = caller.scope();
    let event_id = input.event_id.map(EventId::from_uuid);
    authorize(caller, event_id, stores.identity).await?;

    let source = SourceText::normalize(&input.source_text);
    let (id, proposals) = parse(input, &source)?;
    check_structure(event_id, &proposals)?;
    check_catalog(scope, event_id, &proposals, stores).await?;
    check_free_ids(id, &proposals, stores.proposals).await?;

    let changeset = Changeset {
        id,
        event_id,
        author: caller.actor(),
        source_version_id: SourceVersionId::from_uuid(Uuid::now_v7()),
        created_at: clock.now(),
        proposals,
    };
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::ChangesetCreate,
        Some(id.as_uuid()),
        Some(scope),
    );
    match stores
        .proposals
        .insert(scope, &changeset, &source, &audit)
        .await?
    {
        Inserted::Inserted => Ok(changeset),
        Inserted::IdTaken => Err(invalid("id", "taken")),
    }
}

async fn authorize(
    caller: &impl MayPropose,
    event_id: Option<EventId>,
    identity: &dyn IdentityStore,
) -> Result<(), ProposeError> {
    let allowed = match event_id {
        Some(event) => access::event_access(caller, event, identity)
            .await?
            .can_propose(),
        None => access::sees_all_events(caller),
    };
    if allowed {
        Ok(())
    } else {
        Err(ProposeError::Forbidden)
    }
}

fn invalid(field: &'static str, code: &'static str) -> ProposeError {
    ProposeError::Invalid(vec![FieldError::new(field, code)])
}

/// Fails with all errors, if there are any.
fn finish(errors: Vec<FieldError>) -> Result<(), ProposeError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ProposeError::Invalid(errors))
    }
}

fn record_id_error(uuid: Uuid) -> Option<&'static str> {
    (!ids::is_record_id(uuid)).then_some("not-uuid-v7")
}

/// Maps the input to the domain, and checks the evidence of each proposal against the source text.
fn parse(
    input: NewChangeset,
    source: &SourceText,
) -> Result<(ChangesetId, Vec<Proposal>), ProposeError> {
    let mut errors = Vec::new();
    let id = match input.id {
        Some(id) => {
            if let Some(code) = record_id_error(id) {
                errors.push(FieldError::new("id", code));
            }
            id
        }
        None => Uuid::now_v7(),
    };
    if input.proposals.is_empty() {
        errors.push(FieldError::new("proposals", "empty"));
    }
    let mut proposals = Vec::new();
    for (index, proposal) in input.proposals.into_iter().enumerate() {
        let path = |field: &str| format!("proposals/{index}/{field}");
        if let Some(code) = record_id_error(proposal.id) {
            errors.push(FieldError::new(path("id"), code));
        }
        if proposal.evidence.is_empty() {
            errors.push(FieldError::new(path("evidence"), "evidence-missing"));
        }
        let mut evidence = Vec::new();
        for (number, passage) in proposal.evidence.into_iter().enumerate() {
            let passage = Passage {
                start: passage.start,
                end: passage.end,
                quote: passage.quote,
                page: passage.page,
            };
            match passage.check(source.as_str()) {
                Ok(()) => evidence.push(passage),
                Err(error) => errors.push(FieldError::new(
                    path(&format!("evidence/{number}")),
                    passage_error_code(error),
                )),
            }
        }
        let reason = Reason::parse(&proposal.reason)
            .map_err(|error| errors.push(FieldError::new(path("reason"), text_error_code(error))))
            .ok();
        let operation = Operation::try_from(proposal.operation)
            .map_err(|nested| {
                errors.extend(nested.into_iter().map(|error| {
                    FieldError::new(path(&format!("operation/{}", error.field)), error.code)
                }));
            })
            .ok();
        if let (Some(operation), Some(reason)) = (operation, reason) {
            proposals.push(Proposal {
                id: ProposalId::from_uuid(proposal.id),
                operation,
                depends_on: proposal
                    .depends_on
                    .into_iter()
                    .map(ProposalId::from_uuid)
                    .collect(),
                evidence,
                reason,
            });
        }
    }
    finish(errors)?;
    Ok((ChangesetId::from_uuid(id), proposals))
}

fn passage_error_code(error: PassageError) -> &'static str {
    match error {
        PassageError::Empty => "empty",
        PassageError::OutOfRange => "out-of-range",
        PassageError::QuoteMismatch => "quote-mismatch",
        PassageError::Page => "page",
    }
}

/// Checks the rules between the proposals of a changeset: unique IDs, the event of each operation, and the dependencies.
fn check_structure(event_id: Option<EventId>, proposals: &[Proposal]) -> Result<(), ProposeError> {
    let mut errors = Vec::new();
    let path = |index: usize, field: &str| format!("proposals/{index}/{field}");

    // The proposal that creates each new record of the changeset.
    let mut creators: HashMap<Uuid, ProposalId> = HashMap::new();
    let mut seen = HashSet::new();
    for (index, proposal) in proposals.iter().enumerate() {
        if !seen.insert(proposal.id.as_uuid()) {
            errors.push(FieldError::new(path(index, "id"), "duplicate"));
        }
        if let Some(record) = proposal.operation.new_record() {
            if let Some(code) = record_id_error(record.as_uuid()) {
                errors.push(FieldError::new(path(index, "operation/id"), code));
            }
            if !seen.insert(record.as_uuid()) {
                errors.push(FieldError::new(path(index, "operation/id"), "duplicate"));
            }
            creators.insert(record.as_uuid(), proposal.id);
        }
    }

    for (index, proposal) in proposals.iter().enumerate() {
        let operation = &proposal.operation;
        // A changeset of an event works in that event only. A changeset of the organization works in its new events only.
        let in_scope = match (event_id, operation) {
            (Some(_), Operation::CreateEvent { .. }) => false,
            (None, Operation::CreateEvent { .. }) => true,
            (Some(event), _) => operation.event_id().is_none_or(|own| own == event),
            (None, _) => operation
                .event_id()
                .is_none_or(|own| creators.contains_key(&own.as_uuid())),
        };
        if !in_scope {
            errors.push(FieldError::new(path(index, "operation"), "event-mismatch"));
        }
        // A proposal that uses a new record of the changeset depends on the proposal that creates it.
        let uses = [
            operation
                .event_id()
                .filter(|_| !matches!(operation, Operation::CreateEvent { .. }))
                .map(EventId::as_uuid),
            operation
                .field_id()
                .filter(|_| !matches!(operation, Operation::AddFieldDefinition { .. }))
                .map(FieldDefinitionId::as_uuid),
        ];
        let missing = uses
            .into_iter()
            .flatten()
            .filter_map(|record| creators.get(&record))
            .any(|creator| !proposal.depends_on.contains(creator));
        if missing {
            errors.push(FieldError::new(
                path(index, "depends_on"),
                "dependency-missing",
            ));
        }
    }
    finish(errors)?;

    let graph: Vec<(ProposalId, Vec<ProposalId>)> = proposals
        .iter()
        .map(|proposal| (proposal.id, proposal.depends_on.clone()))
        .collect();
    check_dependencies(&graph).map_err(|error| {
        let (proposal, code) = match error {
            DependencyError::Outside { proposal } => (proposal, "outside-changeset"),
            DependencyError::Cycle { proposal } => (proposal, "cycle"),
        };
        let index = proposals
            .iter()
            .position(|candidate| candidate.id == proposal)
            .unwrap_or_default();
        ProposeError::Invalid(vec![FieldError::new(path(index, "depends_on"), code)])
    })
}

/// A field of the catalog of an event, or a new field of the changeset.
struct CatalogField {
    key: FieldKey,
    value_type: ValueType,
    status: FieldStatus,
    scope: FieldScope,
}

/// Checks each operation against the field catalog, with the new fields and choices of the changeset,
/// and checks that the owner of each new open question is a member of the organization.
async fn check_catalog(
    scope: OrgScope,
    event_id: Option<EventId>,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<(), ProposeError> {
    let mut errors = Vec::new();
    let path = |index: usize, field: &str| format!("proposals/{index}/operation/{field}");

    // A new event has the shipped fields only, so the catalog of any new event gives them.
    let catalog_event = event_id.or_else(|| {
        proposals
            .iter()
            .find_map(|proposal| match proposal.operation {
                Operation::CreateEvent { id, .. } => Some(id),
                _ => None,
            })
    });
    let mut fields: HashMap<FieldDefinitionId, CatalogField> = HashMap::new();
    if let Some(event) = catalog_event {
        for field in stores.facts.catalog(scope, event).await? {
            let in_scope = event_id.is_some() || field.scope == FieldScope::Shipped;
            if in_scope {
                fields.insert(
                    field.id,
                    CatalogField {
                        key: field.key,
                        value_type: field.value_type,
                        status: field.status,
                        scope: field.scope,
                    },
                );
            }
        }
    }

    // The owners of new open questions, checked after the loop.
    let mut owners = Vec::new();
    // The new fields first, then their new choices, then the facts that use them.
    for (index, proposal) in proposals.iter().enumerate() {
        if let Operation::AddFieldDefinition {
            id,
            event_id,
            key,
            value_type,
            ..
        } = &proposal.operation
        {
            let taken = fields.values().any(|field| {
                &field.key == key
                    && (field.scope == FieldScope::Shipped
                        || field.scope == FieldScope::Event(*event_id))
            });
            if taken {
                errors.push(FieldError::new(path(index, "key"), "taken"));
            }
            fields.insert(
                *id,
                CatalogField {
                    key: key.clone(),
                    value_type: value_type.clone(),
                    status: FieldStatus::Active,
                    scope: FieldScope::Event(*event_id),
                },
            );
        }
    }
    for (index, proposal) in proposals.iter().enumerate() {
        match &proposal.operation {
            Operation::AddChoiceValue {
                field_id,
                key,
                label,
            } => match event_field(&mut fields, *field_id) {
                Err(code) => errors.push(FieldError::new(path(index, "field_id"), code)),
                Ok(CatalogField {
                    value_type: ValueType::Choice { values, .. },
                    ..
                }) => {
                    if values.iter().any(|value| &value.key == key) {
                        errors.push(FieldError::new(path(index, "key"), "taken"));
                    }
                    values.push(ChoiceValue {
                        key: key.clone(),
                        label: Label::Text(label.clone()),
                    });
                }
                Ok(_) => errors.push(FieldError::new(path(index, "field_id"), "not-choice")),
            },
            Operation::DeprecateField { field_id } => {
                if let Err(code) = event_field(&mut fields, *field_id) {
                    errors.push(FieldError::new(path(index, "field_id"), code));
                }
            }
            _ => {}
        }
    }
    for (index, proposal) in proposals.iter().enumerate() {
        match &proposal.operation {
            Operation::SetFact {
                event_id,
                field_id,
                state,
                ..
            } => {
                let field = fields.get(field_id).filter(|field| {
                    field.scope == FieldScope::Shipped
                        || field.scope == FieldScope::Event(*event_id)
                });
                let Some(field) = field else {
                    errors.push(FieldError::new(path(index, "field_id"), "unknown-field"));
                    continue;
                };
                if field.status == FieldStatus::Deprecated {
                    errors.push(FieldError::new(path(index, "field_id"), "deprecated"));
                }
                let value = match state {
                    FactState::Accepted(valued) | FactState::Assumption(valued) => {
                        Some(&valued.value)
                    }
                    FactState::Unknown => None,
                };
                if let Some(Err(error)) = value.map(|value| value.check(&field.value_type)) {
                    errors.push(FieldError::new(
                        path(index, "state/value"),
                        value_error_code(error),
                    ));
                }
            }
            Operation::CreateOpenQuestion { owner, .. } => owners.push((index, *owner)),
            _ => {}
        }
    }
    for (index, owner) in owners {
        if stores.identity.membership(scope, owner).await?.is_none() {
            errors.push(FieldError::new(path(index, "owner"), "unknown-member"));
        }
    }
    finish(errors)
}

/// The field of an event that a proposal changes. Shipped fields change only with the catalog in code (ADR 0049).
fn event_field(
    fields: &mut HashMap<FieldDefinitionId, CatalogField>,
    id: FieldDefinitionId,
) -> Result<&mut CatalogField, &'static str> {
    match fields.get_mut(&id) {
        None => Err("unknown-field"),
        Some(field) if field.scope == FieldScope::Shipped => Err("shipped-field"),
        Some(field) => Ok(field),
    }
}

/// Checks that the IDs of the changeset, its proposals and their new records are free.
async fn check_free_ids(
    id: ChangesetId,
    proposals: &[Proposal],
    store: &dyn ProposalStore,
) -> Result<(), ProposeError> {
    let mut wanted = vec![(id.as_uuid(), "id".to_owned())];
    for (index, proposal) in proposals.iter().enumerate() {
        wanted.push((proposal.id.as_uuid(), format!("proposals/{index}/id")));
        if let Some(record) = proposal.operation.new_record() {
            wanted.push((record.as_uuid(), format!("proposals/{index}/operation/id")));
        }
    }
    let ids: Vec<Uuid> = wanted.iter().map(|(id, _)| *id).collect();
    let taken = store.taken_ids(&ids).await?;
    finish(
        wanted
            .into_iter()
            .filter(|(id, _)| taken.contains(id))
            .map(|(_, field)| FieldError::new(field, "taken"))
            .collect(),
    )
}
