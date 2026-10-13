//! `/api/v1/changesets`: the `CreateChangeset` command, the Review Inbox and the review of changesets (ADR 0050).

use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tada_app::dispute_notes::{DisputeNotes, NoteKey};
use tada_app::domain::ids::{ChangesetId, EventId, LocalIdKind, ProposalId, SourceVersionId};
use tada_app::domain::redaction::Redactable;
use tada_app::domain::sources::Excerpt as DomainExcerpt;
use tada_app::facts::FactVersionRef;
use tada_app::problem::ProblemCode;
use tada_app::proposals::{
    self as proposals, Changeset as AppChangeset, Created, FactStateInput, NewChangeset,
    NewProposal, ProposeError, ProposeStores,
};
use tada_app::review::{
    self as app, Applied, ApplyError, ApplyInput, ChangesetCursor, ChangesetReview,
    ConflictReason as AppConflictReason, Edit, InboxChangeset, Link, LocalRecord,
    ProposalReview as AppProposalReview, ProposalStatus as AppProposalStatus, RecordEditInput,
    ReviewQueryError, ReviewStores,
};
use utoipa::openapi::RefOr;
use utoipa::openapi::schema::Schema;
use utoipa::{IntoParams, PartialSchema, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::actors::Author;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::dispute_notes::{DisputeNote, shown};
use crate::documents::DraftRendering;
use crate::extract::{Caller, Json, Path, Query, page_limit};
use crate::json_schema;
use crate::parties::PartyRef;
use crate::problem::{ApiError, Problem};
use crate::values::{FactState, Passage, Value, stored_state_parts};

mod operation;

pub use operation::Operation;

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_event_changesets, create_changeset))
        .routes(routes!(list_changesets))
        .routes(routes!(get_changeset))
        .routes(routes!(apply_changeset))
        .routes(routes!(reject_proposals))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "create_changeset",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, ProposeError::CODES]),
        ),
        (
            "list_event_changesets",
            codes(&[AUTHENTICATED, PATH, QUERY, ReviewQueryError::CODES]),
        ),
        (
            "list_changesets",
            codes(&[AUTHENTICATED, QUERY, ReviewQueryError::CODES]),
        ),
        (
            "get_changeset",
            codes(&[AUTHENTICATED, PATH, ReviewQueryError::CODES]),
        ),
        (
            "apply_changeset",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, ApplyError::CODES]),
        ),
        (
            "reject_proposals",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, ApplyError::CODES]),
        ),
    ]
}

/// The input of `CreateChangeset`: the text of one intake and the proposals that it supports.
/// The event of the path is the event of each proposal.
/// The proposals have the shape of the MCP tools (ADR 0040).
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateChangesetRequest {
    /// The UUIDv7 of the new changeset. A client that sends it can retry the request safely.
    #[serde(default)]
    pub id: Option<Uuid>,
    /// The member's own words. tada stores them as a source version.
    /// The passages count characters (Unicode scalar values) of this text after normalization: Unicode NFC with `\n` line ends.
    /// Leave it out if the proposals rest only on stored source versions (ADR 0079). Each passage then names its
    /// `source_version_id`. A text of white space only counts as empty.
    #[serde(default)]
    pub source_text: String,
    #[schemars(length(max = proposals::MAX_PROPOSALS))]
    pub proposals: Vec<NewProposal>,
}

/// The body contains the words of members, so `Debug` shows the name of the type only (ADR 0035).
impl std::fmt::Debug for CreateChangesetRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CreateChangesetRequest(..)")
    }
}

impl PartialSchema for CreateChangesetRequest {
    fn schema() -> RefOr<Schema> {
        json_schema::schema::<Self>()
    }
}

impl ToSchema for CreateChangesetRequest {}

/// A new changeset.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedChangeset {
    pub id: Uuid,
    /// The event of the changeset. It is absent for a changeset of the organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    /// The source version that holds the member's own words of the intake.
    /// It is absent if the changeset has no source text: its proposals cite only stored source versions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version_id: Option<Uuid>,
    pub created_at: Timestamp,
    /// The IDs of the proposals, in the order of their IDs.
    pub proposal_ids: Vec<Uuid>,
}

impl From<AppChangeset> for CreatedChangeset {
    fn from(changeset: AppChangeset) -> Self {
        let mut proposal_ids: Vec<Uuid> = changeset
            .proposals
            .iter()
            .map(|proposal| proposal.id.as_uuid())
            .collect();
        proposal_ids.sort();
        Self {
            id: changeset.id.as_uuid(),
            event_id: changeset.event_id.map(EventId::as_uuid),
            source_version_id: changeset.source_version_id.map(SourceVersionId::as_uuid),
            created_at: changeset.created_at,
            proposal_ids,
        }
    }
}

/// The filter of a list of changesets.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ChangesetFilter {
    /// The changesets with at least one open proposal.
    Open,
}

/// The parameters of a list of changesets.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListChangesetsQuery {
    /// Only `open` exists: the changesets with at least one open proposal.
    #[param(inline)]
    pub status: ChangesetFilter,
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
}

/// A changeset with open proposals.
#[derive(Debug, Serialize, ToSchema)]
pub struct OpenChangeset {
    pub id: Uuid,
    /// The event of the changeset. It is absent for a changeset of the organization, for example one that creates an event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    pub author: Author,
    pub created_at: Timestamp,
    /// The number of its open proposals in the Review Inbox of the caller (ADR 0067). "My Work" counts the same proposals.
    pub open_proposals: u32,
    /// True if its open proposals are older than 14 days (ADR 0050). A stale proposal does not change.
    pub stale: bool,
}

/// One page of changesets with open proposals, oldest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct OpenChangesetPage {
    pub items: Vec<OpenChangeset>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A changeset as a reviewer sees it.
#[derive(Debug, Serialize, ToSchema)]
pub struct Changeset {
    pub id: Uuid,
    /// The event of the changeset. It is absent for a changeset of the organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    pub author: Author,
    /// The source version that holds the member's own words of the intake.
    /// It is absent if the changeset has no source text: its proposals cite only stored source versions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version_id: Option<Uuid>,
    pub created_at: Timestamp,
    /// The proposals in the order of their IDs.
    pub proposals: Vec<Proposal>,
    /// The proposals that a legal redaction emptied, in the order of their IDs (ADR 0072). They have no operation
    /// values, no reason and no evidence, and nobody reviews them.
    pub redacted_proposals: Vec<RedactedProposal>,
}

/// A proposal that a legal redaction emptied (ADR 0072).
#[derive(Debug, Serialize, ToSchema)]
pub struct RedactedProposal {
    pub id: Uuid,
    /// The kind of the operation, for example `set_fact`.
    pub operation_kind: String,
    /// The proposals of the same changeset that had to apply before this one.
    pub depends_on: Vec<Uuid>,
    pub status: ProposalStatus,
    /// Always true.
    pub redacted: bool,
}

/// One proposal with its evidence, its status and the current value of its target.
#[derive(Debug, Serialize, ToSchema)]
pub struct Proposal {
    pub id: Uuid,
    pub operation: Operation,
    /// The proposals of the same changeset that must apply before this one.
    pub depends_on: Vec<Uuid>,
    /// The short reason of the proposal.
    pub reason: String,
    /// The passages of the source text that support the proposal, each with the text around it.
    pub evidence: Vec<ProposalEvidence>,
    pub status: ProposalStatus,
    /// True if the proposal is open and older than 14 days (ADR 0050).
    pub stale: bool,
    /// True if the proposal is open and older than 3 days. The Review Inbox of an event manager shows an overdue
    /// proposal also when it goes to another reviewer (ADR 0067).
    pub overdue: bool,
    /// True if the review routing gives the proposal to the caller: as the owner of the record, as the lead of the
    /// workstream, or as an event manager when it goes to no other reviewer (ADR 0067).
    /// An event manager can review each proposal of the event, also when this is false.
    pub routed_to_me: bool,
    /// True if the caller reviews the open proposal now, so a client offers accept and reject for it: it is in the
    /// Review Inbox of the caller (ADR 0067). For an event manager, this includes an overdue proposal of another
    /// reviewer. It is false if the caller cannot review an open dependency that an apply of the proposal accepts
    /// with it. Apply and reject accept each proposal with this flag.
    pub can_review: bool,
    /// Why the proposal conflicts, at the time of the read. It is present only if the status is `conflict`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_reason: Option<ConflictReason>,
    /// The current version of the fact that a `set-fact` proposal sets. It is absent if the event has no such fact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<CurrentFact>,
    /// A `create-document-draft` proposal as the reviewer sees it: its Markdown, its lint warnings and the target
    /// of each `tada:` link. It is absent for each other operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<DraftRendering>,
    /// For an open `create-person` or `create-institution` proposal: at most 5 existing records of the same kind
    /// with a similar name (ADR 0069). The reviewer can link the proposal to one of them. It is absent for each
    /// other proposal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicates: Option<Vec<PartyRef>>,
    /// The dispute notes about the proposal (ADR 0073), oldest first.
    /// It is absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schema(required = false)]
    pub dispute_notes: Vec<DisputeNote>,
}

/// One passage of a source version, with the text around it.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProposalEvidence {
    /// The source version of the passage: the source text of the changeset or another source version,
    /// for example a text file of the event.
    pub source_version_id: Uuid,
    pub passage: Passage,
    pub excerpt: Excerpt,
    /// The dispute notes about the source of the passage (ADR 0073), oldest first.
    /// It is absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schema(required = false)]
    pub dispute_notes: Vec<DisputeNote>,
}

/// A passage with the text around it: at most 100 characters before and after it.
#[derive(Serialize, ToSchema)]
pub struct Excerpt {
    /// Empty after a legal redaction of the source (ADR 0072).
    pub before: String,
    pub quote: String,
    pub after: String,
    /// True after a legal redaction of the source: the texts are empty (ADR 0072).
    pub redacted: bool,
}

/// The excerpt copies the source text, so `Debug` shows its lengths only (ADR 0035).
impl std::fmt::Debug for Excerpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let length = |text: &str| text.chars().count();
        write!(
            f,
            "Excerpt({} + {} + {} characters)",
            length(&self.before),
            length(&self.quote),
            length(&self.after)
        )
    }
}

impl From<Redactable<DomainExcerpt>> for Excerpt {
    fn from(excerpt: Redactable<DomainExcerpt>) -> Self {
        match excerpt {
            Redactable::Present(excerpt) => Self {
                before: excerpt.before,
                quote: excerpt.quote,
                after: excerpt.after,
                redacted: false,
            },
            Redactable::Redacted => Self {
                before: String::new(),
                quote: String::new(),
                after: String::new(),
                redacted: true,
            },
        }
    }
}

/// The status of a proposal from its latest review result (ADR 0050). The list of values is open.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalStatus {
    Open,
    Accepted,
    AcceptedWithEdit,
    Rejected,
    Conflict,
    Withdrawn,
}

impl From<AppProposalStatus> for ProposalStatus {
    fn from(status: AppProposalStatus) -> Self {
        match status {
            AppProposalStatus::Open => Self::Open,
            AppProposalStatus::Accepted => Self::Accepted,
            AppProposalStatus::AcceptedWithEdit => Self::AcceptedWithEdit,
            AppProposalStatus::Rejected => Self::Rejected,
            AppProposalStatus::Conflict => Self::Conflict,
            AppProposalStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

/// Why a proposal conflicts. The list of reasons is open.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictReason {
    /// The fact has another version than the proposal expects.
    FactChanged,
    /// Another target record changed, for example a field that is deprecated now.
    TargetChanged,
}

impl From<AppConflictReason> for ConflictReason {
    fn from(reason: AppConflictReason) -> Self {
        match reason {
            AppConflictReason::FactChanged => Self::FactChanged,
            AppConflictReason::TargetChanged => Self::TargetChanged,
        }
    }
}

/// The current version of a fact.
#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentFact {
    pub fact_id: Uuid,
    pub version: i64,
    pub state: FactState,
    /// The value. It is absent if the state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// True for an approximate value, for example "about 20,000". It is absent if the state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
    /// True after a legal redaction (ADR 0072): the state stays, the value and `approximate` are absent.
    pub redacted: bool,
    /// The dispute notes about the current fact version (ADR 0073), oldest first.
    /// It is absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schema(required = false)]
    pub dispute_notes: Vec<DisputeNote>,
}

impl CurrentFact {
    fn new(current: FactVersionRef, notes: &DisputeNotes) -> Self {
        let (state, value, approximate, redacted) = stored_state_parts(&current.state);
        Self {
            dispute_notes: shown(notes, &NoteKey::fact_version(current.id)),
            fact_id: current.fact_id.as_uuid(),
            version: current.number.get(),
            state,
            value,
            approximate,
            redacted,
        }
    }
}

impl From<ChangesetReview> for Changeset {
    fn from(review: ChangesetReview) -> Self {
        Self {
            id: review.id.as_uuid(),
            event_id: review.event_id.map(EventId::as_uuid),
            author: review.author.into(),
            source_version_id: review.source_version_id.map(SourceVersionId::as_uuid),
            created_at: review.created_at,
            proposals: review.proposals.into_iter().map(Proposal::from).collect(),
            redacted_proposals: review
                .redacted
                .into_iter()
                .map(|redacted| RedactedProposal {
                    id: redacted.proposal.id.as_uuid(),
                    operation_kind: redacted.proposal.operation_kind,
                    depends_on: redacted
                        .proposal
                        .depends_on
                        .iter()
                        .map(|id| id.as_uuid())
                        .collect(),
                    status: redacted.status.into(),
                    redacted: true,
                })
                .collect(),
        }
    }
}

impl From<AppProposalReview> for Proposal {
    fn from(review: AppProposalReview) -> Self {
        let proposal = review.proposal;
        Self {
            id: proposal.id.as_uuid(),
            operation: (&proposal.operation).into(),
            depends_on: proposal.depends_on.iter().map(|id| id.as_uuid()).collect(),
            reason: proposal.reason.as_str().to_owned(),
            evidence: proposal
                .evidence
                .iter()
                .zip(review.excerpts)
                .map(|(evidence, excerpt)| ProposalEvidence {
                    source_version_id: evidence.source_version_id.as_uuid(),
                    passage: (&evidence.passage).into(),
                    excerpt: excerpt.into(),
                    dispute_notes: shown(
                        &review.dispute_notes,
                        &NoteKey::source_version(evidence.source_version_id),
                    ),
                })
                .collect(),
            status: review.status.into(),
            stale: review.stale,
            overdue: review.overdue,
            routed_to_me: review.routed_to_me,
            can_review: review.can_review,
            conflict_reason: review.conflict.map(ConflictReason::from),
            current: review
                .current
                .map(|current| CurrentFact::new(current, &review.dispute_notes)),
            draft: review.draft.map(DraftRendering::from),
            duplicates: review
                .duplicates
                .map(|parties| parties.into_iter().map(PartyRef::from).collect()),
            dispute_notes: shown(&review.dispute_notes, &NoteKey::proposal(proposal.id)),
        }
    }
}

/// The input of `ApplyChangeset`.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyChangesetRequest {
    /// The proposals to accept. tada adds the dependencies of each one that an earlier apply did not accept.
    pub selected: Vec<Uuid>,
    /// The changes of the reviewer before the acceptance: the value of a proposal that sets a fact,
    /// or fields of a proposal that creates an action, a commitment, a person or an institution.
    #[serde(default)]
    pub edits: Vec<EditRequest>,
    /// Proposed persons and institutions that use an existing record instead of a new one (ADR 0069).
    #[serde(default)]
    pub links: Vec<LinkRequest>,
}

/// The reviewer links a proposal that creates a person or an institution to an existing record of the same kind.
/// tada creates no record for it, and the other selected proposals use the existing record.
/// An invalid link gives `validation-failed` with the field code `invalid-link`; a link whose proposal has open
/// dependents outside the selection gives `dependents-not-selected`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkRequest {
    /// A selected `create-person` or `create-institution` proposal.
    pub proposal_id: Uuid,
    /// The existing person or institution of the organization.
    pub record_id: Uuid,
}

/// A change of the reviewer before the acceptance. A `set-fact` proposal takes `state`;
/// a proposal that creates an action, a commitment, a person or an institution takes `fields`.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    /// A selected proposal.
    pub proposal_id: Uuid,
    /// The state and value that the reviewer accepts instead of the proposed one.
    // The schema stays the one of a fact state: an absent field is the only way to leave it out.
    #[serde(default)]
    #[schemars(with = "FactStateInput")]
    pub state: Option<FactStateInput>,
    /// The fields of the new record that the reviewer replaces.
    #[serde(default)]
    pub fields: Option<RecordEditInput>,
}

/// The body contains values, so `Debug` shows the name of the type only (ADR 0035).
impl std::fmt::Debug for ApplyChangesetRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApplyChangesetRequest(..)")
    }
}

impl std::fmt::Debug for EditRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EditRequest(..)")
    }
}

impl PartialSchema for ApplyChangesetRequest {
    fn schema() -> RefOr<Schema> {
        json_schema::schema::<Self>()
    }
}

impl ToSchema for ApplyChangesetRequest {}

/// The input of `RejectProposals`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RejectProposalsRequest {
    /// The open proposals to reject. tada also rejects the open proposals that depend on them.
    pub proposal_ids: Vec<Uuid>,
}

/// The proposals that a review changed, with their new status.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewResult {
    pub proposals: Vec<ReviewedProposal>,
    /// The new open questions with their event-local IDs.
    pub open_questions: Vec<NewOpenQuestion>,
    /// The new documents with their organization-local IDs.
    pub documents: Vec<NewDocument>,
    /// The new actions with their event-local IDs.
    pub actions: Vec<NewLocalRecord>,
    /// The new commitments with their event-local IDs.
    pub commitments: Vec<NewLocalRecord>,
    /// The new persons with their organization-local IDs.
    pub persons: Vec<NewLocalRecord>,
    /// The new institutions with their organization-local IDs.
    pub institutions: Vec<NewLocalRecord>,
    /// The new milestones with their event-local IDs.
    pub milestones: Vec<NewLocalRecord>,
    /// The new decisions with their event-local IDs.
    pub decisions: Vec<NewLocalRecord>,
}

/// A proposal with its new status.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewedProposal {
    pub id: Uuid,
    pub status: ProposalStatus,
}

/// A new open question with its event-local ID.
#[derive(Debug, Serialize, ToSchema)]
pub struct NewOpenQuestion {
    pub id: Uuid,
    /// The event-local ID, for example `QST-001` (ADR 0038).
    pub local_id: String,
}

/// A new document with its readable ID.
#[derive(Debug, Serialize, ToSchema)]
pub struct NewDocument {
    pub id: Uuid,
    /// The readable ID, for example `DOC-001` (ADR 0038).
    pub readable_id: String,
}

/// A new record with its readable ID.
#[derive(Debug, Serialize, ToSchema)]
pub struct NewLocalRecord {
    pub id: Uuid,
    /// The readable ID, for example `ACT-001`, `COM-001`, `PER-001`, `INS-001`, `MIL-001` or `DEC-001`
    /// (ADR 0038).
    pub local_id: String,
}

impl NewLocalRecord {
    fn new(id: Uuid, kind: LocalIdKind, number: u64) -> Self {
        Self {
            id,
            local_id: kind.readable_id(number),
        }
    }
}

impl From<Applied> for ReviewResult {
    fn from(applied: Applied) -> Self {
        let mut open_questions = Vec::new();
        let mut documents = Vec::new();
        let (mut actions, mut commitments) = (Vec::new(), Vec::new());
        let (mut persons, mut institutions) = (Vec::new(), Vec::new());
        let (mut milestones, mut decisions) = (Vec::new(), Vec::new());
        for local in applied.local_ids {
            let number = local.local_number;
            match local.record {
                LocalRecord::OpenQuestion(id) => open_questions.push(NewOpenQuestion {
                    id: id.as_uuid(),
                    local_id: LocalIdKind::OpenQuestion.readable_id(number),
                }),
                LocalRecord::Document(id) => documents.push(NewDocument {
                    id: id.as_uuid(),
                    readable_id: LocalIdKind::Document.readable_id(number),
                }),
                LocalRecord::Action(id) => {
                    actions.push(NewLocalRecord::new(
                        id.as_uuid(),
                        LocalIdKind::Action,
                        number,
                    ));
                }
                LocalRecord::Commitment(id) => commitments.push(NewLocalRecord::new(
                    id.as_uuid(),
                    LocalIdKind::Commitment,
                    number,
                )),
                LocalRecord::Person(id) => {
                    persons.push(NewLocalRecord::new(
                        id.as_uuid(),
                        LocalIdKind::Person,
                        number,
                    ));
                }
                LocalRecord::Institution(id) => institutions.push(NewLocalRecord::new(
                    id.as_uuid(),
                    LocalIdKind::Institution,
                    number,
                )),
                LocalRecord::Milestone(id) => milestones.push(NewLocalRecord::new(
                    id.as_uuid(),
                    LocalIdKind::Milestone,
                    number,
                )),
                LocalRecord::Decision(id) => decisions.push(NewLocalRecord::new(
                    id.as_uuid(),
                    LocalIdKind::Decision,
                    number,
                )),
            }
        }
        Self {
            actions,
            commitments,
            persons,
            institutions,
            milestones,
            decisions,
            proposals: applied
                .proposals
                .into_iter()
                .map(|(id, status)| ReviewedProposal {
                    id: id.as_uuid(),
                    status: status.into(),
                })
                .collect(),
            open_questions,
            documents,
        }
    }
}

fn review_stores(state: &ApiState) -> ReviewStores<'_> {
    ReviewStores {
        identity: state.identity.as_ref(),
        facts: state.facts.as_ref(),
        proposals: state.proposals.as_ref(),
        review: state.review.as_ref(),
        sources: state.sources.as_ref(),
        workstreams: state.workstreams.as_ref(),
        work: state.work.as_ref(),
        milestones: state.milestones.as_ref(),
        parties: state.parties.as_ref(),
    }
}

/// Creates a changeset of proposals in an event, with the channel `web`.
/// Contributors and event managers can propose; a viewer gets `forbidden`.
///
/// If a changeset with the same `id` and the same content exists, the response is this changeset with the
/// status 200, and nothing changes.
#[utoipa::path(
    post,
    path = "/events/{event_id}/changesets",
    operation_id = "create_changeset",
    tag = "review",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateChangesetRequest,
    responses(
        (status = CREATED, description = "The new changeset.", body = CreatedChangeset),
        (status = OK, description = "The changeset exists with this ID and the same content.", body = CreatedChangeset),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_changeset(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateChangesetRequest>,
) -> Result<(StatusCode, axum::Json<CreatedChangeset>), ApiError> {
    let input = NewChangeset {
        id: request.id,
        event_id: Some(event_id),
        source_text: request.source_text,
        proposals: request.proposals,
    };
    let stores = ProposeStores {
        identity: state.identity.as_ref(),
        facts: state.facts.as_ref(),
        proposals: state.proposals.as_ref(),
        sources: state.sources.as_ref(),
        documents: state.documents.as_ref(),
        workstreams: state.workstreams.as_ref(),
        parties: state.parties.as_ref(),
        work: state.work.as_ref(),
        milestones: state.milestones.as_ref(),
        decisions: state.decisions.as_ref(),
    };
    match proposals::create_changeset(&caller, input, stores, state.clock.as_ref()).await? {
        Created::New(changeset) => Ok((StatusCode::CREATED, axum::Json(changeset.into()))),
        Created::Existing(changeset) => Ok((StatusCode::OK, axum::Json(changeset.into()))),
    }
}

/// Lists the changesets of an event with open proposals in the Review Inbox of the caller, oldest first (ADR 0067).
/// A viewer of the event gets `forbidden`.
#[utoipa::path(
    get,
    path = "/events/{event_id}/changesets",
    operation_id = "list_event_changesets",
    tag = "review",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListChangesetsQuery),
    responses(
        (status = OK, description = "One page of changesets.", body = OpenChangesetPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_event_changesets(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListChangesetsQuery>,
) -> Result<axum::Json<OpenChangesetPage>, ApiError> {
    list(&state, &caller, Some(EventId::from_uuid(event_id)), query).await
}

/// The Review Inbox: the changesets with open proposals that the review routing gives the caller, oldest first (ADR 0067).
/// A proposal goes to the owner of the record that it changes, to the lead of the workstream of a new record,
/// or to the event managers. An event manager also sees the overdue proposals of the event.
/// Owners and admins also see the changesets of the organization, for example one that creates an event.
#[utoipa::path(
    get,
    path = "/changesets",
    operation_id = "list_changesets",
    tag = "review",
    params(ListChangesetsQuery),
    responses(
        (status = OK, description = "One page of changesets.", body = OpenChangesetPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_changesets(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Query(query): Query<ListChangesetsQuery>,
) -> Result<axum::Json<OpenChangesetPage>, ApiError> {
    list(&state, &caller, None, query).await
}

async fn list(
    state: &ApiState,
    caller: &tada_app::caller::MemberCaller,
    event_id: Option<EventId>,
    query: ListChangesetsQuery,
) -> Result<axum::Json<OpenChangesetPage>, ApiError> {
    let ChangesetFilter::Open = query.status;
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_open_changesets(
        caller,
        event_id,
        after,
        limit,
        review_stores(state),
        state.clock.as_ref(),
    )
    .await?;
    let now = state.clock.now();
    Ok(axum::Json(OpenChangesetPage {
        items: page
            .items
            .into_iter()
            .map(|changeset| open_changeset(changeset, now))
            .collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

fn open_changeset(item: InboxChangeset, now: Timestamp) -> OpenChangeset {
    let InboxChangeset {
        changeset,
        in_inbox,
    } = item;
    OpenChangeset {
        id: changeset.id.as_uuid(),
        event_id: changeset.event_id.map(EventId::as_uuid),
        author: changeset.author.into(),
        created_at: changeset.created_at,
        open_proposals: in_inbox,
        stale: changeset.is_stale(now),
    }
}

/// Reads a changeset with each proposal, its evidence, its status and the current value of its target.
/// Only a reviewer of at least one of its proposals can read it (ADR 0067).
#[utoipa::path(
    get,
    path = "/changesets/{changeset_id}",
    operation_id = "get_changeset",
    tag = "review",
    params(("changeset_id" = Uuid, Path, description = "The ID of the changeset.")),
    responses(
        (status = OK, description = "The changeset.", body = Changeset),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_changeset(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(changeset_id): Path<Uuid>,
) -> Result<axum::Json<Changeset>, ApiError> {
    let review = app::get_changeset(
        &caller,
        ChangesetId::from_uuid(changeset_id),
        review_stores(&state),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(review.into()))
}

/// Accepts the selected proposals and their dependencies, all or nothing (ADR 0050).
/// A changed target gives `record-version-conflict`, and the proposals concerned get the status `conflict`.
/// A selected proposal that is not open gives `invalid-transition`.
#[utoipa::path(
    post,
    path = "/changesets/{changeset_id}/apply",
    operation_id = "apply_changeset",
    tag = "review",
    params(("changeset_id" = Uuid, Path, description = "The ID of the changeset.")),
    request_body = ApplyChangesetRequest,
    responses(
        (status = OK, description = "The accepted proposals.", body = ReviewResult),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn apply_changeset(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(changeset_id): Path<Uuid>,
    Json(request): Json<ApplyChangesetRequest>,
) -> Result<axum::Json<ReviewResult>, ApiError> {
    let input = ApplyInput {
        selected: request
            .selected
            .into_iter()
            .map(ProposalId::from_uuid)
            .collect(),
        edits: request
            .edits
            .into_iter()
            .map(|edit| Edit {
                proposal_id: ProposalId::from_uuid(edit.proposal_id),
                state: edit.state,
                fields: edit.fields,
            })
            .collect(),
        links: request
            .links
            .into_iter()
            .map(|link| Link {
                proposal_id: ProposalId::from_uuid(link.proposal_id),
                record_id: link.record_id,
            })
            .collect(),
    };
    let applied = app::apply_changeset(
        &caller,
        ChangesetId::from_uuid(changeset_id),
        input,
        review_stores(&state),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(applied.into()))
}

/// Rejects open proposals and the open proposals that depend on them.
/// A proposal that is not open gives `invalid-transition`.
#[utoipa::path(
    post,
    path = "/changesets/{changeset_id}/reject",
    operation_id = "reject_proposals",
    tag = "review",
    params(("changeset_id" = Uuid, Path, description = "The ID of the changeset.")),
    request_body = RejectProposalsRequest,
    responses(
        (status = OK, description = "The rejected proposals.", body = ReviewResult),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn reject_proposals(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(changeset_id): Path<Uuid>,
    Json(request): Json<RejectProposalsRequest>,
) -> Result<axum::Json<ReviewResult>, ApiError> {
    let rejected = app::reject_proposals(
        &caller,
        ChangesetId::from_uuid(changeset_id),
        request
            .proposal_ids
            .into_iter()
            .map(ProposalId::from_uuid)
            .collect(),
        review_stores(&state),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(ReviewResult {
        proposals: rejected
            .proposals
            .into_iter()
            .map(|id| ReviewedProposal {
                id: id.as_uuid(),
                status: ProposalStatus::Rejected,
            })
            .collect(),
        open_questions: Vec::new(),
        documents: Vec::new(),
        actions: Vec::new(),
        commitments: Vec::new(),
        persons: Vec::new(),
        institutions: Vec::new(),
        milestones: Vec::new(),
        decisions: Vec::new(),
    }))
}

/// The cursor is opaque for clients (ADR 0044): the creation time and the ID, in Base64.
fn encode_cursor(cursor: &ChangesetCursor) -> String {
    cursor::encode(format!("{} {}", cursor.created_at, cursor.id))
}

fn decode_cursor(text: &str) -> Result<ChangesetCursor, ApiError> {
    let text = cursor::decode_text(text)?;
    let (created_at, id) = text.split_once(' ').ok_or_else(cursor::invalid)?;
    Ok(ChangesetCursor {
        created_at: created_at.parse().map_err(|_| cursor::invalid())?,
        id: ChangesetId::from_uuid(id.parse().map_err(|_| cursor::invalid())?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_survives_its_round_trip() {
        let cursor = ChangesetCursor {
            created_at: "2030-05-18T08:00:00.123456Z".parse().unwrap(),
            id: ChangesetId::from_uuid(Uuid::now_v7()),
        };
        let decoded = decode_cursor(&encode_cursor(&cursor)).unwrap();
        assert_eq!(decoded, cursor);
        assert!(decode_cursor("not a cursor").is_err());
    }
}
