//! `/api/v1/changesets`: the `CreateChangeset` command, the Review Inbox and the review of changesets (ADR 0050).

use axum::extract::State;
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{ChangesetId, EventId, LocalIdKind, ProposalId};
use tada_app::domain::proposals::{
    DraftDocument as DomainDraftDocument, Operation as DomainOperation,
};
use tada_app::domain::sources::Excerpt as DomainExcerpt;
use tada_app::facts::FactVersionRef;
use tada_app::problem::ProblemCode;
use tada_app::proposals::{
    self as proposals, Changeset as AppChangeset, Created, FactStateInput, NewChangeset,
    NewProposal, ProposeError, ProposeStores,
};
use tada_app::review::{
    self as app, Applied, ApplyError, ApplyInput, ChangesetCursor, ChangesetReview,
    ConflictReason as AppConflictReason, Edit, LocalRecord, OpenChangeset as AppOpenChangeset,
    ProposalReview as AppProposalReview, ProposalStatus as AppProposalStatus, ReviewQueryError,
    ReviewStores,
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
use crate::documents::DraftRendering;
use crate::extract::{Caller, Json, Path, Query, page_limit};
use crate::json_schema;
use crate::problem::{ApiError, Problem};
use crate::values::{FactState, Label, Passage, Value, ValueType, state_parts};

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
    pub source_text: String,
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
    /// The source version that holds the text of the intake.
    pub source_version_id: Uuid,
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
            source_version_id: changeset.source_version_id.as_uuid(),
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
    /// The number of its open proposals.
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
    /// The source version that holds the text of the intake.
    pub source_version_id: Uuid,
    pub created_at: Timestamp,
    /// The proposals in the order of their IDs.
    pub proposals: Vec<Proposal>,
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
}

/// One passage of a source version, with the text around it.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProposalEvidence {
    /// The source version of the passage: the source text of the changeset or another source version,
    /// for example a text file of the event.
    pub source_version_id: Uuid,
    pub passage: Passage,
    pub excerpt: Excerpt,
}

/// A passage with the text around it: at most 100 characters before and after it.
#[derive(Serialize, ToSchema)]
pub struct Excerpt {
    pub before: String,
    pub quote: String,
    pub after: String,
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

impl From<DomainExcerpt> for Excerpt {
    fn from(excerpt: DomainExcerpt) -> Self {
        Self {
            before: excerpt.before,
            quote: excerpt.quote,
            after: excerpt.after,
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
}

impl From<FactVersionRef> for CurrentFact {
    fn from(current: FactVersionRef) -> Self {
        let (state, value, approximate) = state_parts(&current.state);
        Self {
            fact_id: current.fact_id.as_uuid(),
            version: current.number.get(),
            state,
            value,
            approximate,
        }
    }
}

/// The change that a proposal suggests. `kind` names the operation. The list of kinds is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Operation {
    CreateEvent {
        id: Uuid,
        key: String,
        name: String,
        time_zone: String,
    },
    SetFact {
        event_id: Uuid,
        field_id: Uuid,
        state: FactState,
        /// The proposed value. It is absent if the proposed state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// True for an approximate value. It is absent if the proposed state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        approximate: Option<bool>,
        /// The fact version that the proposal expects. It is absent if the event had no fact of the field.
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_version: Option<i64>,
    },
    AddFieldDefinition {
        id: Uuid,
        event_id: Uuid,
        key: String,
        label: Label,
        value_type: ValueType,
        description: String,
        module: String,
    },
    AddChoiceValue {
        event_id: Uuid,
        field_id: Uuid,
        key: String,
        label: Label,
    },
    DeprecateField {
        event_id: Uuid,
        field_id: Uuid,
    },
    CreateOpenQuestion {
        id: Uuid,
        event_id: Uuid,
        text: String,
        /// The user ID of the member who owns the question.
        owner: Uuid,
    },
    CreateDocumentDraft {
        event_id: Uuid,
        document: DraftDocument,
        /// The Markdown of the draft.
        markdown: String,
    },
}

/// The document of a draft.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DraftDocument {
    /// A new document of the event.
    New { id: Uuid, name: String },
    /// An existing document of the event.
    Existing {
        document_id: Uuid,
        /// The record version of the document that the proposal expects.
        expected_version: i64,
    },
}

impl From<&DomainDraftDocument> for DraftDocument {
    fn from(document: &DomainDraftDocument) -> Self {
        match document {
            DomainDraftDocument::New { id, name } => Self::New {
                id: id.as_uuid(),
                name: name.as_str().to_owned(),
            },
            DomainDraftDocument::Existing {
                document_id,
                expected_version,
            } => Self::Existing {
                document_id: document_id.as_uuid(),
                expected_version: expected_version.get(),
            },
        }
    }
}

impl From<&DomainOperation> for Operation {
    fn from(operation: &DomainOperation) -> Self {
        let text_label = |text: &tada_app::domain::facts::ShortText| Label::Text {
            text: text.as_str().to_owned(),
        };
        match operation {
            DomainOperation::CreateEvent {
                id,
                key,
                name,
                time_zone,
            } => Self::CreateEvent {
                id: id.as_uuid(),
                key: key.as_str().to_owned(),
                name: name.as_str().to_owned(),
                time_zone: time_zone.as_str().to_owned(),
            },
            DomainOperation::SetFact {
                event_id,
                field_id,
                state,
                expected_version,
            } => {
                let (state, value, approximate) = state_parts(state);
                Self::SetFact {
                    event_id: event_id.as_uuid(),
                    field_id: field_id.as_uuid(),
                    state,
                    value,
                    approximate,
                    expected_version: expected_version.map(|version| version.get()),
                }
            }
            DomainOperation::AddFieldDefinition {
                id,
                event_id,
                key,
                label,
                value_type,
                description,
                module,
            } => Self::AddFieldDefinition {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                key: key.as_str().to_owned(),
                label: text_label(label),
                value_type: value_type.into(),
                description: description.as_str().to_owned(),
                module: module.as_str().to_owned(),
            },
            DomainOperation::AddChoiceValue {
                event_id,
                field_id,
                key,
                label,
            } => Self::AddChoiceValue {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
                key: key.as_str().to_owned(),
                label: text_label(label),
            },
            DomainOperation::DeprecateField { event_id, field_id } => Self::DeprecateField {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
            },
            DomainOperation::CreateOpenQuestion {
                id,
                event_id,
                text,
                owner,
            } => Self::CreateOpenQuestion {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                text: text.as_str().to_owned(),
                owner: owner.as_uuid(),
            },
            DomainOperation::CreateDocumentDraft {
                event_id,
                document,
                markdown,
            } => Self::CreateDocumentDraft {
                event_id: event_id.as_uuid(),
                document: document.into(),
                markdown: markdown.as_str().to_owned(),
            },
        }
    }
}

impl From<ChangesetReview> for Changeset {
    fn from(review: ChangesetReview) -> Self {
        Self {
            id: review.id.as_uuid(),
            event_id: review.event_id.map(EventId::as_uuid),
            author: review.author.into(),
            source_version_id: review.source_version_id.as_uuid(),
            created_at: review.created_at,
            proposals: review.proposals.into_iter().map(Proposal::from).collect(),
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
                })
                .collect(),
            status: review.status.into(),
            stale: review.stale,
            conflict_reason: review.conflict.map(ConflictReason::from),
            current: review.current.map(CurrentFact::from),
            draft: review.draft.map(DraftRendering::from),
        }
    }
}

/// The input of `ApplyChangeset`.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyChangesetRequest {
    /// The proposals to accept. tada adds the dependencies of each one that an earlier apply did not accept.
    pub selected: Vec<Uuid>,
    /// The values that the reviewer changes before the acceptance. Only a proposal that sets a fact has a value.
    #[serde(default)]
    pub edits: Vec<EditRequest>,
}

/// A value that the reviewer changes before the acceptance.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    /// A selected proposal that sets a fact.
    pub proposal_id: Uuid,
    /// The state and value that the reviewer accepts instead of the proposed one.
    pub state: FactStateInput,
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

impl From<Applied> for ReviewResult {
    fn from(applied: Applied) -> Self {
        let mut open_questions = Vec::new();
        let mut documents = Vec::new();
        for local in applied.local_ids {
            match local.record {
                LocalRecord::OpenQuestion(id) => open_questions.push(NewOpenQuestion {
                    id: id.as_uuid(),
                    local_id: LocalIdKind::OpenQuestion.readable_id(local.local_number),
                }),
                LocalRecord::Document(id) => documents.push(NewDocument {
                    id: id.as_uuid(),
                    readable_id: LocalIdKind::Document.readable_id(local.local_number),
                }),
            }
        }
        Self {
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
    };
    match proposals::create_changeset(&caller, input, stores, state.clock.as_ref()).await? {
        Created::New(changeset) => Ok((StatusCode::CREATED, axum::Json(changeset.into()))),
        Created::Existing(changeset) => Ok((StatusCode::OK, axum::Json(changeset.into()))),
    }
}

/// Lists the changesets of an event with open proposals, oldest first. Only its event managers see them.
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

/// The Review Inbox: the changesets with open proposals that the caller can review, oldest first.
/// They are the changesets of each event that the caller manages and, for owners and admins,
/// the changesets of the organization, for example one that creates an event.
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
        state.identity.as_ref(),
        state.review.as_ref(),
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

fn open_changeset(changeset: AppOpenChangeset, now: Timestamp) -> OpenChangeset {
    OpenChangeset {
        id: changeset.id.as_uuid(),
        event_id: changeset.event_id.map(EventId::as_uuid),
        author: changeset.author.into(),
        created_at: changeset.created_at,
        open_proposals: changeset.open_proposals,
        stale: changeset.is_stale(now),
    }
}

/// Reads a changeset with each proposal, its evidence, its status and the current value of its target.
/// Only the reviewers of the changeset can read it.
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
    }))
}

/// The cursor is opaque for clients (ADR 0044): the creation time and the ID, in Base64.
fn encode_cursor(cursor: &ChangesetCursor) -> String {
    URL_SAFE_NO_PAD.encode(format!("{} {}", cursor.created_at, cursor.id))
}

fn decode_cursor(text: &str) -> Result<ChangesetCursor, ApiError> {
    let invalid =
        || ApiError::new(ProblemCode::MalformedRequest).with_detail("The cursor is not valid.");
    let bytes = URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())?;
    let text = String::from_utf8(bytes).map_err(|_| invalid())?;
    let (created_at, id) = text.split_once(' ').ok_or_else(invalid)?;
    Ok(ChangesetCursor {
        created_at: created_at.parse().map_err(|_| invalid())?,
        id: ChangesetId::from_uuid(id.parse().map_err(|_| invalid())?),
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
