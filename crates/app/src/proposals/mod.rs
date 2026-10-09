//! The `CreateChangeset` command (ADR 0050): members and their AI clients propose changes with evidence.
//!
//! A proposal never changes accepted state. A member with the right to review applies it later.

mod drafts;
mod input;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::facts::{
    ChoiceKey, ChoiceValue, FactState, FactValue, FieldKey, FieldScope, FieldStatus, Label,
    ValueType,
};
use tada_domain::ids::{
    self, ChangesetId, EventId, FieldDefinitionId, ProposalId, SourceVersionId,
};
use tada_domain::proposals::{DependencyError, Operation, Proposal, Reason, check_dependencies};
use tada_domain::sources::{Evidence, Passage, PassageError, SourceText};
use uuid::Uuid;

pub use self::input::{
    ChoiceInput, DraftDocumentInput, FactStateInput, GranularityInput, NewChangeset, NewProposal,
    OperationInput, PassageInput, ReferenceTargetInput, ValueInput, ValueTypeInput,
};
pub(crate) use self::input::{state_from_input, text_error_code, value_error_code};
use crate::access::{self, AccessError, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{Actor, AiCaller, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::documents::DocumentStore;
use crate::drafts::DraftProvenance;
use crate::facts::FactStore;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::sources::SourceStore;
use crate::store::StoreError;
use crate::tokens::TokenScope;

/// A caller that can create proposals (ADR 0039): a member, or an AI client of a member.
/// A service identity cannot propose. The Telegram gateway proposes through a `MemberCaller` with the channel Telegram.
///
/// The trait is sealed: only `MemberCaller` and `AiCaller` implement it (ADR 0039).
/// Another crate cannot make a caller whose credential allows proposals:
///
/// ```compile_fail,E0277
/// use tada_app::access::Principal;
/// use tada_app::caller::{Actor, AiCaller, OrgScope, OrganizationRole};
/// use tada_app::domain::ids::UserId;
/// use tada_app::proposals::MayPropose;
///
/// struct Wrapper(AiCaller);
///
/// impl Principal for Wrapper {
///     fn user_id(&self) -> UserId {
///         self.0.user_id()
///     }
///     fn scope(&self) -> OrgScope {
///         self.0.scope()
///     }
///     fn organization_role(&self) -> OrganizationRole {
///         self.0.organization_role()
///     }
/// }
///
/// impl MayPropose for Wrapper {
///     fn actor(&self) -> Actor {
///         self.0.actor()
///     }
///     fn credential_allows_proposals(&self) -> bool {
///         true
///     }
/// }
/// ```
pub trait MayPropose: Principal + crate::caller::sealed::Sealed {
    /// The author of the changeset and of its audit event.
    fn actor(&self) -> Actor;

    /// True if the credential of the caller allows proposals at all. The event role decides the rest.
    fn credential_allows_proposals(&self) -> bool;
}

impl MayPropose for MemberCaller {
    fn actor(&self) -> Actor {
        MemberCaller::actor(self)
    }

    fn credential_allows_proposals(&self) -> bool {
        true
    }
}

/// An AI client proposes only with a `propose` token (ADR 0039), and only where its member can propose (ADR 0052).
impl MayPropose for AiCaller {
    fn actor(&self) -> Actor {
        AiCaller::actor(self)
    }

    fn credential_allows_proposals(&self) -> bool {
        self.token_scope() == TokenScope::Propose
    }
}

/// A stored changeset with its proposals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changeset {
    pub id: ChangesetId,
    /// `None` for a changeset of the organization, for example one that creates an event.
    pub event_id: Option<EventId>,
    pub author: Actor,
    /// The source version of the intake text. The evidence of a proposal can also cite other source versions.
    pub source_version_id: SourceVersionId,
    pub created_at: Timestamp,
    pub proposals: Vec<Proposal>,
    /// The provenance manifest and the lint warnings of each draft proposal, fixed at its creation (ADR 0051).
    pub drafts: Vec<DraftProvenance>,
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
    /// in any organization, or that a proposal names as the ID of its new record.
    /// A new record ID must be free in the whole installation (ADR 0038).
    /// Infrastructure query (ADR 0039): a record ID must be free in the whole installation (ADR 0038).
    async fn taken_ids(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError>;

    /// Stores `source` as a source version of the kind `member-text` with the ID `changeset.source_version_id`,
    /// then the changeset with its proposals, their dependencies, their evidence and the provenance of its drafts, then `audit`.
    /// It writes all of them in one transaction, or nothing.
    async fn insert(
        &self,
        scope: OrgScope,
        changeset: &Changeset,
        source: &SourceText,
        audit: &AuditEvent,
    ) -> Result<Inserted, StoreError>;

    /// The changeset `id` of the organization with the normalized text of its source version, or `None`.
    /// Its proposals and drafts are in the order of their IDs.
    async fn get(
        &self,
        scope: OrgScope,
        id: ChangesetId,
    ) -> Result<Option<(Changeset, SourceText)>, StoreError>;
}

/// The result of a successful `create_changeset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    New(Changeset),
    /// A retry (ADR 0038): the changeset with this ID and the same content exists, and nothing changed.
    Existing(Changeset),
}

/// The ports that `create_changeset` reads and writes.
#[derive(Debug, Clone, Copy)]
pub struct ProposeStores<'a> {
    pub identity: &'a dyn IdentityStore,
    pub facts: &'a dyn FactStore,
    pub proposals: &'a dyn ProposalStore,
    /// The source versions that drafts cite.
    pub sources: &'a dyn SourceStore,
    /// The existing documents of drafts.
    pub documents: &'a dyn DocumentStore,
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

/// The longest source text of a changeset, in characters after the normalization.
/// The search index of one text is limited to 1 MB, and each passage check reads the text.
pub const MAX_SOURCE_TEXT_CHARS: usize = 100_000;

/// Creates a changeset of proposals with their evidence (ADR 0050).
///
/// 1. A changeset of an event needs the right to propose in the event (ADR 0052).
///    A changeset of the organization, for example a new event, needs an owner or admin as principal.
/// 2. The source text has at most `MAX_SOURCE_TEXT_CHARS` characters.
///    Each proposal has at least one passage, and each passage matches the text of its source version (ADR 0040).
///    A passage names no source version for the source text of the changeset. A passage of another source version
///    must be readable in the event of the proposal (`access::event_source_reach`), for example a text file of the event.
/// 3. Each new ID is a UUIDv7 and is free (ADR 0038).
/// 4. The dependencies stay inside the changeset, have no cycle, and include the proposals that create the records that a proposal uses.
///    A fact that uses a new choice of the changeset depends on the proposal that adds the choice.
/// 5. Each value matches the value type of its field. A new field of the same changeset counts.
/// 6. Each link of a draft resolves to a fact version of its event or a visible source passage (ADR 0051).
///    The changeset keeps the provenance manifest and the lint warnings of each draft.
///
/// It stores the source text as a source version, the changeset and an audit event in one transaction.
/// If a check fails, it stores nothing.
/// A retry with the ID of a stored changeset and the same content returns the stored changeset (ADR 0038).
pub async fn create_changeset(
    caller: &impl MayPropose,
    input: NewChangeset,
    stores: ProposeStores<'_>,
    clock: &dyn Clock,
) -> Result<Created, ProposeError> {
    let scope = caller.scope();
    let event_id = input.event_id.map(EventId::from_uuid);
    authorize(caller, event_id, stores.identity).await?;

    let source = SourceText::normalize(&input.source_text);
    if source.as_str().chars().count() > MAX_SOURCE_TEXT_CHARS {
        return Err(invalid("source_text", "length"));
    }
    let source_version_id = SourceVersionId::from_uuid(Uuid::now_v7());
    let retry = input.id.is_some();
    let (id, proposals) = parse(input, &source, source_version_id)?;
    let intake = Intake {
        event_id,
        author: caller.actor(),
        source: &source,
        source_version_id,
        proposals: &proposals,
    };
    if retry && let Some(stored) = stores.proposals.get(scope, id).await? {
        return intake.existing(stored);
    }
    check_structure(event_id, &proposals)?;
    check_catalog(scope, event_id, &proposals, stores).await?;
    check_cited_sources(caller, source_version_id, &proposals, stores).await?;
    let drafts = drafts::check_drafts(caller, &proposals, stores).await?;
    check_free_ids(id, &proposals, stores.proposals).await?;

    let changeset = Changeset {
        id,
        event_id,
        author: caller.actor(),
        source_version_id,
        created_at: clock.now(),
        proposals: proposals.clone(),
        drafts,
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
        Inserted::Inserted => Ok(Created::New(changeset)),
        // A concurrent request stored a changeset with one of the IDs first.
        Inserted::IdTaken => match stores.proposals.get(scope, id).await? {
            Some(stored) => intake.existing(stored),
            None => Err(invalid("id", "taken")),
        },
    }
}

/// The content of a new changeset, to compare it with a stored changeset of the same ID.
struct Intake<'a> {
    event_id: Option<EventId>,
    author: Actor,
    source: &'a SourceText,
    /// The new source version of `source`. A stored changeset has its own.
    source_version_id: SourceVersionId,
    proposals: &'a [Proposal],
}

impl Intake<'_> {
    /// The stored changeset if it has the same content, else `id` `taken`.
    /// The same author is the same party for the same principal; the channel and the request can differ on a retry.
    fn existing(&self, (stored, source): (Changeset, SourceText)) -> Result<Created, ProposeError> {
        let author = |actor: &Actor| (actor.kind(), actor.id(), actor.principal());
        let mut proposals = self.proposals.to_vec();
        proposals.sort_by_key(|proposal| proposal.id);
        // The evidence in the intake text names the source version of the stored changeset.
        let intake = |mut evidence: Evidence| {
            if evidence.source_version_id == self.source_version_id {
                evidence.source_version_id = stored.source_version_id;
            }
            evidence
        };
        let proposals = proposals.into_iter().map(|mut proposal| {
            proposal.evidence = proposal.evidence.into_iter().map(intake).collect();
            proposal
        });
        let normalized = |mut proposal: Proposal| {
            proposal.depends_on.sort();
            proposal
        };
        let same = stored.event_id == self.event_id
            && author(&stored.author) == author(&self.author)
            && &source == self.source
            && stored.proposals.len() == self.proposals.len()
            && stored
                .proposals
                .iter()
                .cloned()
                .map(normalized)
                .eq(proposals.map(normalized));
        if same {
            Ok(Created::Existing(stored))
        } else {
            Err(invalid("id", "taken"))
        }
    }
}

async fn authorize(
    caller: &impl MayPropose,
    event_id: Option<EventId>,
    identity: &dyn IdentityStore,
) -> Result<(), ProposeError> {
    if !caller.credential_allows_proposals() {
        return Err(ProposeError::Forbidden);
    }
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

/// Maps the input to the domain, and checks the evidence of each proposal against the source text,
/// which tada stores as the source version `source_version_id`.
fn parse(
    input: NewChangeset,
    source: &SourceText,
    source_version_id: SourceVersionId,
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
        for (number, input) in proposal.evidence.into_iter().enumerate() {
            let passage = Passage {
                start: input.start,
                end: input.end,
                quote: input.quote,
                page: input.page,
            };
            // `check_cited_sources` checks a passage of another source version.
            if let Some(cited) = input.source_version_id {
                evidence.push(Evidence {
                    source_version_id: SourceVersionId::from_uuid(cited),
                    passage,
                });
                continue;
            }
            match check_passage(&passage, source) {
                Ok(()) => evidence.push(Evidence {
                    source_version_id,
                    passage,
                }),
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

/// Checks each passage that cites a source version other than the source text of the changeset `intake`.
///
/// The source version must be citable in the event of the proposal (`access::citable_reach`), so a proposal never shows a text of another
/// event to the members of its event: `unknown-source` for any other ID, also of another organization.
/// A source version without a text, for example a PDF, gives `no-text`.
async fn check_cited_sources(
    caller: &impl MayPropose,
    intake: SourceVersionId,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<(), ProposeError> {
    // All cited source versions of one event, so that each event needs one reach and one read.
    let mut cited: HashMap<EventId, Vec<SourceVersionId>> = HashMap::new();
    for proposal in proposals {
        cited
            .entry(proposal.operation.event_id())
            .or_default()
            .extend(
                proposal
                    .evidence
                    .iter()
                    .map(|evidence| evidence.source_version_id)
                    .filter(|id| *id != intake),
            );
    }
    let mut texts: HashMap<(EventId, SourceVersionId), Option<SourceText>> = HashMap::new();
    for (event, ids) in cited {
        if ids.is_empty() {
            continue;
        }
        let reach = access::citable_reach(caller, event, stores.identity).await?;
        for source in stores.sources.texts(caller.scope(), &reach, &ids).await? {
            texts.insert((event, source.id), source.text);
        }
    }
    let mut errors = Vec::new();
    for (index, proposal) in proposals.iter().enumerate() {
        let event = proposal.operation.event_id();
        for (number, evidence) in proposal.evidence.iter().enumerate() {
            if evidence.source_version_id == intake {
                continue;
            }
            let code = match texts.get(&(event, evidence.source_version_id)) {
                None => Some("unknown-source"),
                Some(None) => Some("no-text"),
                Some(Some(text)) => check_passage(&evidence.passage, text)
                    .err()
                    .map(passage_error_code),
            };
            if let Some(code) = code {
                errors.push(FieldError::new(
                    format!("proposals/{index}/evidence/{number}"),
                    code,
                ));
            }
        }
    }
    finish(errors)
}

/// Checks a passage against the text of its source version. The source versions with a text are member texts and
/// text files, which have no pages (ADR 0050).
fn check_passage(passage: &Passage, text: &SourceText) -> Result<(), PassageError> {
    match passage.page {
        Some(_) => Err(PassageError::Page),
        None => passage.check(text.as_str()),
    }
}

fn passage_error_code(error: PassageError) -> &'static str {
    match error {
        PassageError::Empty => "empty",
        PassageError::OutOfRange => "out-of-range",
        PassageError::QuoteMismatch => "quote-mismatch",
        PassageError::Page => "page",
    }
}

/// The choices that a `SetFact` with a choice value uses, with their field.
fn used_choices(operation: &Operation) -> impl Iterator<Item = (FieldDefinitionId, &ChoiceKey)> {
    let keys = match operation {
        Operation::SetFact {
            field_id,
            state: FactState::Accepted(valued) | FactState::Assumption(valued),
            ..
        } => match &valued.value {
            FactValue::Choice(keys) => Some((*field_id, keys)),
            _ => None,
        },
        _ => None,
    };
    keys.into_iter()
        .flat_map(|(field, keys)| keys.iter().map(move |key| (field, key)))
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

    // The proposal that adds each new choice of the changeset. A choice creates no record, but a fact that uses
    // the choice is valid only with it (ADR 0049), so the fact depends on it like on a new record.
    let mut choice_creators: HashMap<(FieldDefinitionId, &ChoiceKey), ProposalId> = HashMap::new();
    for proposal in proposals {
        if let Operation::AddChoiceValue { field_id, key, .. } = &proposal.operation {
            choice_creators.insert((*field_id, key), proposal.id);
        }
    }

    let new_events: HashSet<EventId> = proposals
        .iter()
        .filter_map(|proposal| match proposal.operation {
            Operation::CreateEvent { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    for (index, proposal) in proposals.iter().enumerate() {
        let operation = &proposal.operation;
        let creates_event = matches!(operation, Operation::CreateEvent { .. });
        let mut dependencies = HashSet::new();
        if !proposal.depends_on.iter().all(|id| dependencies.insert(id)) {
            errors.push(FieldError::new(path(index, "depends_on"), "duplicate"));
        }
        // A changeset of an event works in that event only. A changeset of the organization works in its new events only.
        let in_scope = match event_id {
            Some(event) => !creates_event && operation.event_id() == event,
            None => new_events.contains(&operation.event_id()),
        };
        if !in_scope {
            errors.push(FieldError::new(path(index, "operation"), "event-mismatch"));
        }
        // A proposal that uses a new record of the changeset depends on the proposal that creates it.
        let uses = [
            (!creates_event).then(|| operation.event_id().as_uuid()),
            operation
                .field_id()
                .filter(|_| !matches!(operation, Operation::AddFieldDefinition { .. }))
                .map(FieldDefinitionId::as_uuid),
        ];
        let record_creators = uses
            .into_iter()
            .flatten()
            .filter_map(|record| creators.get(&record));
        let choice_creators =
            used_choices(operation).filter_map(|(field, key)| choice_creators.get(&(field, key)));
        let missing = record_creators
            .chain(choice_creators)
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
/// and checks that the owner of each new open question is a member of its event.
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
    let mut new_fields = HashSet::new();
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
            new_fields.insert(*id);
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
                event_id,
                field_id,
                key,
                label,
            } => match event_field(&mut fields, *event_id, *field_id) {
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
            Operation::DeprecateField { event_id, field_id } => {
                if let Err(code) = event_field(&mut fields, *event_id, *field_id) {
                    errors.push(FieldError::new(path(index, "field_id"), code));
                }
            }
            _ => {}
        }
    }
    for (index, proposal) in proposals.iter().enumerate() {
        match &proposal.operation {
            Operation::SetFact {
                event_id: fact_event,
                field_id,
                state,
                expected_version,
            } => {
                // A new event or a new field has no fact yet, so a proposal cannot expect a version of it.
                // `event_id` is the event of the changeset; `fact_event` is the event of the fact.
                let new_fact = event_id.is_none() || new_fields.contains(field_id);
                if new_fact && expected_version.is_some() {
                    errors.push(FieldError::new(path(index, "expected_version"), "invalid"));
                }
                let field = fields.get(field_id).filter(|field| {
                    field.scope == FieldScope::Shipped
                        || field.scope == FieldScope::Event(*fact_event)
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
            Operation::CreateOpenQuestion {
                event_id, owner, ..
            } => owners.push((index, *event_id, *owner)),
            _ => {}
        }
    }
    for (index, event, owner) in owners {
        // The owner of a work record is a member of its event (ADR 0052).
        let access = access::member_access(scope, event, owner, stores.identity).await?;
        if access.is_none() {
            errors.push(FieldError::new(path(index, "owner"), "unknown-member"));
        }
    }
    finish(errors)
}

/// The field of the event `event` that a proposal changes. Shipped fields change only with the catalog in code (ADR 0049).
fn event_field(
    fields: &mut HashMap<FieldDefinitionId, CatalogField>,
    event: EventId,
    id: FieldDefinitionId,
) -> Result<&mut CatalogField, &'static str> {
    match fields.get_mut(&id) {
        Some(field) if field.scope == FieldScope::Shipped => Err("shipped-field"),
        Some(field) if field.scope == FieldScope::Event(event) => Ok(field),
        _ => Err("unknown-field"),
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
