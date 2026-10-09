//! `/api/v1/events/{event_id}/actions`, `/api/v1/events/{event_id}/commitments` and `/api/v1/me/work` (ADR 0068).

use axum::extract::State;
use axum::http::StatusCode;
use jiff::civil;
use serde::{Deserialize, Deserializer, Serialize};
use tada_app::caller::MemberCaller;
use tada_app::domain::ids::{
    ActionId, CommitmentId, EventId, InstitutionId, PersonId, UserId, WorkstreamId,
};
use tada_app::domain::parties::Party;
use tada_app::domain::work::{
    ActionStatus as DomainActionStatus, CommitmentStatus as DomainStatus,
};
use tada_app::problem::ProblemCode;
use tada_app::work::{
    self as app, ActionChange, ActionView, CommitmentChange, CommitmentView, FirmInput, InEvent,
    NewAction, NewCommitment, RecordEvidenceView, WorkCursor, WorkError, WorkPorts, WorkQuery,
};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_actions, create_action))
        .routes(routes!(get_action, change_action))
        .routes(routes!(list_commitments, create_commitment))
        .routes(routes!(get_commitment, change_commitment))
        .routes(routes!(make_commitment_firm))
        .routes(routes!(my_work))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let read = || codes(&[AUTHENTICATED, PATH, WorkError::READ_CODES]);
    let list = || codes(&[AUTHENTICATED, PATH, QUERY, WorkError::READ_CODES]);
    let write = || codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkError::CODES]);
    vec![
        ("list_actions", list()),
        ("create_action", write()),
        ("get_action", read()),
        ("change_action", write()),
        ("list_commitments", list()),
        ("create_commitment", write()),
        ("get_commitment", read()),
        ("change_commitment", write()),
        ("make_commitment_firm", write()),
        ("my_work", codes(&[AUTHENTICATED, WorkError::READ_CODES])),
    ]
}

fn ports(state: &ApiState) -> WorkPorts<'_> {
    WorkPorts {
        identity: state.identity.as_ref(),
        work: state.work.as_ref(),
        workstreams: state.workstreams.as_ref(),
        parties: state.parties.as_ref(),
        clock: state.clock.as_ref(),
    }
}

/// The status of an action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActionStatus {
    Open,
    InProgress,
    Blocked,
    Done,
    Canceled,
}

impl From<ActionStatus> for DomainActionStatus {
    fn from(status: ActionStatus) -> Self {
        match status {
            ActionStatus::Open => Self::Open,
            ActionStatus::InProgress => Self::InProgress,
            ActionStatus::Blocked => Self::Blocked,
            ActionStatus::Done => Self::Done,
            ActionStatus::Canceled => Self::Canceled,
        }
    }
}

impl From<DomainActionStatus> for ActionStatus {
    fn from(status: DomainActionStatus) -> Self {
        match status {
            DomainActionStatus::Open => Self::Open,
            DomainActionStatus::InProgress => Self::InProgress,
            DomainActionStatus::Blocked => Self::Blocked,
            DomainActionStatus::Done => Self::Done,
            DomainActionStatus::Canceled => Self::Canceled,
        }
    }
}

/// The status of a commitment. Only "make firm" changes a commitment to `firm`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CommitmentStatus {
    Conditional,
    Firm,
    Fulfilled,
    Broken,
    Withdrawn,
}

impl From<CommitmentStatus> for DomainStatus {
    fn from(status: CommitmentStatus) -> Self {
        match status {
            CommitmentStatus::Conditional => Self::Conditional,
            CommitmentStatus::Firm => Self::Firm,
            CommitmentStatus::Fulfilled => Self::Fulfilled,
            CommitmentStatus::Broken => Self::Broken,
            CommitmentStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

impl From<DomainStatus> for CommitmentStatus {
    fn from(status: DomainStatus) -> Self {
        match status {
            DomainStatus::Conditional => Self::Conditional,
            DomainStatus::Firm => Self::Firm,
            DomainStatus::Fulfilled => Self::Fulfilled,
            DomainStatus::Broken => Self::Broken,
            DomainStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

/// An action of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Action {
    pub id: Uuid,
    /// The readable ID, for example `ACT-001`.
    pub local_id: String,
    pub event_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    /// The member who owns the action: a contributor or manager of the event.
    pub owner_user_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub due_date: Option<civil::Date>,
    pub status: ActionStatus,
    /// The record version. A change needs it.
    pub version: i64,
}

impl From<ActionView> for Action {
    fn from(action: ActionView) -> Self {
        let local_id = action.local_id();
        let fields = action.fields;
        Self {
            id: action.id.as_uuid(),
            local_id,
            event_id: action.event_id.as_uuid(),
            title: fields.title.as_str().to_owned(),
            description: fields.description.map(|text| text.as_str().to_owned()),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            due_date: fields.due_date,
            status: fields.status.into(),
            version: action.version.get(),
        }
    }
}

/// The kind of a promisor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PromisorKind {
    Person,
    Institution,
}

/// The person or institution that makes a commitment.
#[derive(Debug, Serialize, ToSchema)]
pub struct Promisor {
    pub kind: PromisorKind,
    pub id: Uuid,
    /// The readable ID, for example `INS-001`.
    pub local_id: String,
    pub name: String,
}

/// A passage that supports one version of a commitment.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecordEvidence {
    /// The record version that the accepted change produced.
    pub record_version: i64,
    pub proposal_id: Uuid,
    pub source_version_id: Uuid,
    /// The capture time of the source version.
    pub captured_at: jiff::Timestamp,
    pub start_offset: u32,
    pub end_offset: u32,
    pub quote: String,
    pub page: Option<u32>,
}

impl From<RecordEvidenceView> for RecordEvidence {
    fn from(evidence: RecordEvidenceView) -> Self {
        Self {
            record_version: evidence.record_version.get(),
            proposal_id: evidence.proposal_id.as_uuid(),
            source_version_id: evidence.source_version_id.as_uuid(),
            captured_at: evidence.captured_at,
            start_offset: evidence.start_offset,
            end_offset: evidence.end_offset,
            quote: evidence.quote,
            page: evidence.page,
        }
    }
}

/// A commitment of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Commitment {
    pub id: Uuid,
    /// The readable ID, for example `COM-001`.
    pub local_id: String,
    pub event_id: Uuid,
    pub text: String,
    /// The condition. It never changes, and it stays after the commitment becomes firm.
    pub condition: Option<String>,
    pub promisor: Promisor,
    /// The member who follows the commitment up: a contributor or manager of the event.
    pub owner_user_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub due_date: Option<civil::Date>,
    pub status: CommitmentStatus,
    /// The reason that made the commitment firm. A commitment that started firm has none.
    pub firm_reason: Option<String>,
    /// The record version. A change needs it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or changed the commitment.
    pub evidence: Vec<RecordEvidence>,
}

impl From<CommitmentView> for Commitment {
    fn from(commitment: CommitmentView) -> Self {
        let local_id = commitment.local_id();
        let fields = commitment.fields;
        let (kind, id) = match commitment.promisor.party {
            Party::Person(id) => (PromisorKind::Person, id.as_uuid()),
            Party::Institution(id) => (PromisorKind::Institution, id.as_uuid()),
        };
        Self {
            id: commitment.id.as_uuid(),
            local_id,
            event_id: commitment.event_id.as_uuid(),
            text: fields.text.as_str().to_owned(),
            condition: commitment.condition.map(|text| text.as_str().to_owned()),
            promisor: Promisor {
                kind,
                id,
                local_id: commitment.promisor.local_id,
                name: commitment.promisor.name.as_str().to_owned(),
            },
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            due_date: fields.due_date,
            status: fields.status.into(),
            firm_reason: fields.firm_reason.map(|text| text.as_str().to_owned()),
            version: commitment.version.get(),
            evidence: commitment
                .evidence
                .into_iter()
                .map(RecordEvidence::from)
                .collect(),
        }
    }
}

/// One page of actions.
#[derive(Debug, Serialize, ToSchema)]
pub struct ActionPage {
    pub items: Vec<Action>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One page of commitments.
#[derive(Debug, Serialize, ToSchema)]
pub struct CommitmentPage {
    pub items: Vec<Commitment>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of a list of actions.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListActionsQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// `me`, or the ID of a member: only the actions that this member owns.
    pub owner: Option<String>,
    pub status: Option<ActionStatus>,
    /// Only the actions of this workstream.
    pub workstream: Option<Uuid>,
}

/// The parameters of a list of commitments.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListCommitmentsQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// `me`, or the ID of a member: only the commitments that this member owns.
    pub owner: Option<String>,
    pub status: Option<CommitmentStatus>,
    /// Only the commitments of this workstream.
    pub workstream: Option<Uuid>,
}

/// The input of `CreateAction`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateActionRequest {
    /// The UUIDv7 of the new action. A client that sends it can retry the request safely.
    pub id: Option<Uuid>,
    /// 1 to 200 characters.
    #[schema(example = "Generator bestellen")]
    pub title: String,
    /// 1 to 4000 characters.
    pub description: Option<String>,
    /// A member with the contributor or manager role in the event.
    pub owner_user_id: Uuid,
    /// An active workstream of the event.
    pub workstream_id: Option<Uuid>,
    pub due_date: Option<civil::Date>,
}

/// Tells an absent field from a field that is `null`: the first keeps the value, the second clears it.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

/// The input of `ChangeAction`. An absent field stays as it is. `null` clears an optional field.
/// A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeActionRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    pub owner_user_id: Option<Uuid>,
    /// An active workstream of the event. A record keeps a workstream that closes later.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    pub workstream_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<civil::Date>)]
    pub due_date: Option<Option<civil::Date>>,
    pub status: Option<ActionStatus>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The promisor of a new commitment: a person or an institution of the organization.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PromisorInput {
    pub kind: PromisorKind,
    pub id: Uuid,
}

/// The input of `CreateCommitment`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCommitmentRequest {
    /// The UUIDv7 of the new commitment. A client that sends it can retry the request safely.
    pub id: Option<Uuid>,
    /// 1 to 500 characters.
    #[schema(example = "Generator delivery Friday 15:00")]
    pub text: String,
    /// 1 to 500 characters. A commitment with a condition starts `conditional`, else `firm`.
    #[schema(example = "subject to signed order")]
    pub condition: Option<String>,
    pub promisor: PromisorInput,
    /// A member with the contributor or manager role in the event.
    pub owner_user_id: Uuid,
    /// An active workstream of the event.
    pub workstream_id: Option<Uuid>,
    pub due_date: Option<civil::Date>,
}

/// The input of `ChangeCommitment`. An absent field stays as it is. `null` clears an optional field.
/// The condition never changes. The status `firm` needs "make firm".
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeCommitmentRequest {
    pub text: Option<String>,
    pub owner_user_id: Option<Uuid>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    pub workstream_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<civil::Date>)]
    pub due_date: Option<Option<civil::Date>>,
    pub status: Option<CommitmentStatus>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The input of `MakeCommitmentFirm`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct MakeFirmRequest {
    /// Why the condition is met: 1 to 500 characters. The commitment keeps it.
    #[schema(example = "The order is signed.")]
    pub reason: String,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

fn encode_cursor(cursor: &WorkCursor) -> String {
    cursor::encode(cursor.0.to_string())
}

fn decode_cursor(text: &str) -> Result<WorkCursor, ApiError> {
    cursor::decode_text(text)?
        .parse()
        .map(WorkCursor)
        .map_err(|_| cursor::invalid())
}

/// The `owner` filter: `me` or the ID of a member.
fn owner_filter(caller: &MemberCaller, owner: Option<&str>) -> Result<Option<UserId>, ApiError> {
    match owner {
        None => Ok(None),
        Some("me") => Ok(Some(caller.user_id())),
        Some(text) => Uuid::parse_str(text)
            .map(|id| Some(UserId::from_uuid(id)))
            .map_err(|_| {
                ApiError::new(ProblemCode::MalformedRequest)
                    .with_detail("The owner must be `me` or the ID of a member.")
            }),
    }
}

fn promisor(input: &PromisorInput) -> Party {
    match input.kind {
        PromisorKind::Person => Party::Person(PersonId::from_uuid(input.id)),
        PromisorKind::Institution => Party::Institution(InstitutionId::from_uuid(input.id)),
    }
}

/// Lists the actions of an event in the order of their numbers. Each reader of the event can do it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/actions",
    operation_id = "list_actions",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListActionsQuery),
    responses(
        (status = OK, description = "One page of actions.", body = ActionPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_actions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListActionsQuery>,
) -> Result<axum::Json<ActionPage>, ApiError> {
    let query = WorkQuery {
        owner: owner_filter(&caller, query.owner.as_deref())?,
        status: query.status.map(DomainActionStatus::from),
        workstream: query.workstream.map(WorkstreamId::from_uuid),
        after: query.cursor.as_deref().map(decode_cursor).transpose()?,
        limit: page_limit(query.limit)?,
    };
    let page = app::list_actions(
        &caller,
        EventId::from_uuid(event_id),
        query,
        state.identity.as_ref(),
        state.work.as_ref(),
    )
    .await?;
    Ok(axum::Json(ActionPage {
        items: page.items.into_iter().map(Action::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates an action. A contributor or manager of the event can do it.
/// An owner without the contributor or manager role gives `validation-failed` with the field code `unknown-member`.
/// A closed workstream gives the field code `closed`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/actions",
    operation_id = "create_action",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateActionRequest,
    responses(
        (status = CREATED, description = "The new action.", body = Action),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_action(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateActionRequest>,
) -> Result<(StatusCode, axum::Json<Action>), ApiError> {
    let input = NewAction {
        id: request.id,
        title: request.title,
        description: request.description,
        owner: UserId::from_uuid(request.owner_user_id),
        workstream: request.workstream_id.map(WorkstreamId::from_uuid),
        due_date: request.due_date,
    };
    let action =
        app::create_action(&caller, EventId::from_uuid(event_id), input, ports(&state)).await?;
    Ok((StatusCode::CREATED, axum::Json(action.into())))
}

/// Reads one action of the event.
#[utoipa::path(
    get,
    path = "/events/{event_id}/actions/{id}",
    operation_id = "get_action",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the action."),
    ),
    responses(
        (status = OK, description = "The action.", body = Action),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_action(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::Json<Action>, ApiError> {
    let action = app::get_action(
        &caller,
        EventId::from_uuid(event_id),
        ActionId::from_uuid(id),
        state.identity.as_ref(),
        state.work.as_ref(),
    )
    .await?;
    Ok(axum::Json(action.into()))
}

/// Changes an action. Its owner, the lead of its workstream or an event manager can do it.
/// A status that the current status cannot change to gives `invalid-transition`.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/actions/{id}",
    operation_id = "change_action",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the action."),
    ),
    request_body = ChangeActionRequest,
    responses(
        (status = OK, description = "The changed action.", body = Action),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_action(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeActionRequest>,
) -> Result<axum::Json<Action>, ApiError> {
    let change = ActionChange {
        title: request.title,
        description: request.description,
        owner: request.owner_user_id.map(UserId::from_uuid),
        workstream: request
            .workstream_id
            .map(|id| id.map(WorkstreamId::from_uuid)),
        due_date: request.due_date,
        status: request.status.map(DomainActionStatus::from),
        expected_version: record_version(request.expected_version)?,
    };
    let action = app::change_action(
        &caller,
        EventId::from_uuid(event_id),
        ActionId::from_uuid(id),
        change,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(action.into()))
}

/// Lists the commitments of an event in the order of their numbers. Each reader of the event can do it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/commitments",
    operation_id = "list_commitments",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListCommitmentsQuery),
    responses(
        (status = OK, description = "One page of commitments.", body = CommitmentPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_commitments(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListCommitmentsQuery>,
) -> Result<axum::Json<CommitmentPage>, ApiError> {
    let query = WorkQuery {
        owner: owner_filter(&caller, query.owner.as_deref())?,
        status: query.status.map(DomainStatus::from),
        workstream: query.workstream.map(WorkstreamId::from_uuid),
        after: query.cursor.as_deref().map(decode_cursor).transpose()?,
        limit: page_limit(query.limit)?,
    };
    let page = app::list_commitments(
        &caller,
        EventId::from_uuid(event_id),
        query,
        state.identity.as_ref(),
        state.work.as_ref(),
    )
    .await?;
    Ok(axum::Json(CommitmentPage {
        items: page.items.into_iter().map(Commitment::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates a commitment. A contributor or manager of the event can do it.
/// A promisor that is not a person or institution of the organization gives the field code `unknown-record`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/commitments",
    operation_id = "create_commitment",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateCommitmentRequest,
    responses(
        (status = CREATED, description = "The new commitment.", body = Commitment),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_commitment(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateCommitmentRequest>,
) -> Result<(StatusCode, axum::Json<Commitment>), ApiError> {
    let input = NewCommitment {
        id: request.id,
        text: request.text,
        condition: request.condition,
        promisor: promisor(&request.promisor),
        owner: UserId::from_uuid(request.owner_user_id),
        workstream: request.workstream_id.map(WorkstreamId::from_uuid),
        due_date: request.due_date,
    };
    let commitment =
        app::create_commitment(&caller, EventId::from_uuid(event_id), input, ports(&state)).await?;
    Ok((StatusCode::CREATED, axum::Json(commitment.into())))
}

/// Reads one commitment of the event, with its evidence.
#[utoipa::path(
    get,
    path = "/events/{event_id}/commitments/{id}",
    operation_id = "get_commitment",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the commitment."),
    ),
    responses(
        (status = OK, description = "The commitment.", body = Commitment),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_commitment(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::Json<Commitment>, ApiError> {
    let commitment = app::get_commitment(
        &caller,
        EventId::from_uuid(event_id),
        CommitmentId::from_uuid(id),
        state.identity.as_ref(),
        state.work.as_ref(),
    )
    .await?;
    Ok(axum::Json(commitment.into()))
}

/// Changes a commitment. Its owner, the lead of its workstream or an event manager can do it.
/// The status `firm` gives `invalid-transition`: only "make firm" makes a commitment firm.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/commitments/{id}",
    operation_id = "change_commitment",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the commitment."),
    ),
    request_body = ChangeCommitmentRequest,
    responses(
        (status = OK, description = "The changed commitment.", body = Commitment),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_commitment(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeCommitmentRequest>,
) -> Result<axum::Json<Commitment>, ApiError> {
    let change = CommitmentChange {
        text: request.text,
        owner: request.owner_user_id.map(UserId::from_uuid),
        workstream: request
            .workstream_id
            .map(|id| id.map(WorkstreamId::from_uuid)),
        due_date: request.due_date,
        status: request.status.map(DomainStatus::from),
        expected_version: record_version(request.expected_version)?,
    };
    let commitment = app::change_commitment(
        &caller,
        EventId::from_uuid(event_id),
        CommitmentId::from_uuid(id),
        change,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(commitment.into()))
}

/// Makes a conditional commitment firm. The people who change the commitment can do it.
/// The reason is mandatory; the condition stays as history (ADR 0068).
#[utoipa::path(
    post,
    path = "/events/{event_id}/commitments/{id}/firm",
    operation_id = "make_commitment_firm",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the commitment."),
    ),
    request_body = MakeFirmRequest,
    responses(
        (status = OK, description = "The firm commitment.", body = Commitment),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn make_commitment_firm(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<MakeFirmRequest>,
) -> Result<axum::Json<Commitment>, ApiError> {
    let input = FirmInput {
        reason: request.reason,
        expected_version: record_version(request.expected_version)?,
    };
    let commitment = app::make_commitment_firm(
        &caller,
        EventId::from_uuid(event_id),
        CommitmentId::from_uuid(id),
        input,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(commitment.into()))
}

/// An action in "My Work", with the key of its event.
#[derive(Debug, Serialize, ToSchema)]
pub struct MyAction {
    /// The key of the event: with `local_id` it forms the full reference, for example `FLY28/ACT-042`.
    pub event_key: String,
    #[serde(flatten)]
    pub action: Action,
}

/// A commitment in "My Work", with the key of its event.
#[derive(Debug, Serialize, ToSchema)]
pub struct MyCommitment {
    /// The key of the event: with `local_id` it forms the full reference, for example `FLY28/COM-003`.
    pub event_key: String,
    #[serde(flatten)]
    pub commitment: Commitment,
}

/// The open work of the caller.
#[derive(Debug, Serialize, ToSchema)]
pub struct MyWork {
    /// The actions with the status `open`, `in-progress` or `blocked`.
    /// Due date first, a record without a due date last, then event key and readable ID.
    pub actions: Vec<MyAction>,
    /// The commitments with the status `conditional` or `firm`, in the same order.
    pub commitments: Vec<MyCommitment>,
    /// The number of proposals that the caller reviews.
    pub review_count: u32,
}

/// Lists the open actions and commitments that the caller owns.
/// It leaves out the events where the caller has no event role.
#[utoipa::path(
    get,
    path = "/me/work",
    operation_id = "my_work",
    tag = "work",
    responses(
        (status = OK, description = "The open work of the caller.", body = MyWork),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn my_work(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<MyWork>, ApiError> {
    let view = app::my_work(&caller, state.work.as_ref()).await?;
    Ok(axum::Json(MyWork {
        actions: view
            .work
            .actions
            .into_iter()
            .map(|InEvent { event_key, record }| MyAction {
                event_key: event_key.as_str().to_owned(),
                action: record.into(),
            })
            .collect(),
        commitments: view
            .work
            .commitments
            .into_iter()
            .map(|InEvent { event_key, record }| MyCommitment {
                event_key: event_key.as_str().to_owned(),
                commitment: record.into(),
            })
            .collect(),
        review_count: view.review_count,
    }))
}
