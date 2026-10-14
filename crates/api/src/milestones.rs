//! `/api/v1/events/{event_id}/milestones` (ADR 0081): the dated points of the plan of an event.

use axum::extract::State;
use axum::http::StatusCode;
use jiff::civil;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{ActionId, EventId, MilestoneId, UserId, WorkstreamId};
use tada_app::domain::records::MilestoneStatus as DomainStatus;
use tada_app::problem::ProblemCode;
use tada_app::records::Shown;
use tada_app::work::{
    self as app, MilestoneChange, MilestoneView, MilestoneWarning, NewMilestone, WorkError,
    WorkQuery,
};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};
use crate::records::{RecordEvidence, decode_cursor, encode_cursor, present};
use crate::work::{owner_filter, ports};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_milestones, create_milestone))
        .routes(routes!(get_milestone, change_milestone))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let write = || codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkError::CODES]);
    vec![
        (
            "list_milestones",
            codes(&[AUTHENTICATED, PATH, QUERY, WorkError::READ_CODES]),
        ),
        ("create_milestone", write()),
        (
            "get_milestone",
            codes(&[AUTHENTICATED, PATH, WorkError::READ_CODES]),
        ),
        ("change_milestone", write()),
    ]
}

/// The status of a milestone.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MilestoneStatus {
    Open,
    Reached,
    Canceled,
}

impl From<MilestoneStatus> for DomainStatus {
    fn from(status: MilestoneStatus) -> Self {
        match status {
            MilestoneStatus::Open => Self::Open,
            MilestoneStatus::Reached => Self::Reached,
            MilestoneStatus::Canceled => Self::Canceled,
        }
    }
}

impl From<DomainStatus> for MilestoneStatus {
    fn from(status: DomainStatus) -> Self {
        match status {
            DomainStatus::Open => Self::Open,
            DomainStatus::Reached => Self::Reached,
            DomainStatus::Canceled => Self::Canceled,
        }
    }
}

/// A warning about a milestone. tada calculates it at each read and does not store it.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Warning {
    /// The milestone is open and its date is before today in the time zone of the event.
    Overdue,
    /// An action of the milestone is open, in progress or blocked, and it is due after the date of the milestone.
    LateAction { action_id: Uuid },
}

impl From<MilestoneWarning> for Warning {
    fn from(warning: MilestoneWarning) -> Self {
        match warning {
            MilestoneWarning::Overdue => Self::Overdue,
            MilestoneWarning::LateAction(id) => Self::LateAction {
                action_id: ActionId::as_uuid(id),
            },
        }
    }
}

/// A milestone of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Milestone {
    pub id: Uuid,
    /// The readable ID, for example `MIL-001`.
    pub local_id: String,
    pub event_id: Uuid,
    /// Empty after a legal redaction (ADR 0072).
    pub title: String,
    /// Absent after a legal redaction.
    pub description: Option<String>,
    /// True after a legal redaction (ADR 0072): the texts are empty or absent.
    pub redacted: bool,
    pub date: civil::Date,
    /// The member who owns the milestone: a contributor or manager of the event.
    pub owner_user_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub status: MilestoneStatus,
    /// The warnings at the time of the read: an overdue milestone first, then the late actions in the order of
    /// their numbers.
    pub warnings: Vec<Warning>,
    /// The record version. A change needs it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or changed the milestone.
    pub evidence: Vec<RecordEvidence>,
    /// True if the caller can change the milestone now: its owner, the lead of its workstream or an event manager.
    pub can_change: bool,
    /// The statuses that the caller can set with a change now. Empty without `can_change`.
    pub next_statuses: Vec<MilestoneStatus>,
}

impl From<Shown<MilestoneView>> for Milestone {
    fn from(shown: Shown<MilestoneView>) -> Self {
        let next_statuses = shown.next_statuses().into_iter().map(Into::into).collect();
        let Shown {
            record: milestone,
            evidence,
            can_change,
            dispute_notes,
        } = shown;
        let local_id = milestone.local_id();
        let fields = milestone.fields;
        let redacted = fields.title.as_present().is_none();
        Self {
            id: milestone.id.as_uuid(),
            local_id,
            event_id: milestone.event_id.as_uuid(),
            redacted,
            title: fields
                .title
                .as_present()
                .map(|title| title.as_str().to_owned())
                .unwrap_or_default(),
            description: fields.description.map(|text| text.as_str().to_owned()),
            date: fields.date,
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            status: fields.status.into(),
            warnings: milestone.warnings.into_iter().map(Warning::from).collect(),
            version: milestone.version.get(),
            evidence: evidence
                .into_iter()
                .map(|evidence| RecordEvidence::new(evidence, &dispute_notes))
                .collect(),
            // A redacted milestone cannot change (ADR 0072).
            can_change: can_change && !redacted,
            next_statuses: if redacted { Vec::new() } else { next_statuses },
        }
    }
}

/// One page of milestones.
#[derive(Debug, Serialize, ToSchema)]
pub struct MilestonePage {
    pub items: Vec<Milestone>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of a list of milestones.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListMilestonesQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// `me`, or the ID of a member: only the milestones that this member owns.
    pub owner: Option<String>,
    pub status: Option<MilestoneStatus>,
    /// Only the milestones of this workstream.
    pub workstream: Option<Uuid>,
}

/// The input of `CreateMilestone`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMilestoneRequest {
    /// The UUIDv7 of the new milestone. Without it, the server chooses one. An ID that a record holds gives the field code `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// 1 to 200 characters.
    #[schema(example = "Bewilligung liegt vor")]
    pub title: String,
    /// 1 to 4000 characters.
    pub description: Option<String>,
    pub date: civil::Date,
    /// A member with the contributor or manager role in the event.
    pub owner_user_id: Uuid,
    /// An active workstream of the event.
    pub workstream_id: Option<Uuid>,
}

/// The input of `ChangeMilestone`. An absent field stays as it is. `null` clears an optional field.
/// A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeMilestoneRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    pub date: Option<civil::Date>,
    pub owner_user_id: Option<Uuid>,
    /// An active workstream of the event. A record keeps a workstream that closes later.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    pub workstream_id: Option<Option<Uuid>>,
    pub status: Option<MilestoneStatus>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Lists the milestones of an event in the order of their numbers, with their warnings. Each reader of the event can do it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/milestones",
    operation_id = "list_milestones",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListMilestonesQuery),
    responses(
        (status = OK, description = "One page of milestones.", body = MilestonePage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_milestones(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListMilestonesQuery>,
) -> Result<axum::Json<MilestonePage>, ApiError> {
    let query = WorkQuery {
        owner: owner_filter(&caller, query.owner.as_deref())?,
        status: query.status.map(DomainStatus::from),
        workstream: query.workstream.map(WorkstreamId::from_uuid),
        after: query.cursor.as_deref().map(decode_cursor).transpose()?,
        limit: page_limit(query.limit)?,
    };
    let page =
        app::list_milestones(&caller, EventId::from_uuid(event_id), query, ports(&state)).await?;
    Ok(axum::Json(MilestonePage {
        items: page.items.into_iter().map(Milestone::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates a milestone. A contributor or manager of the event can do it.
/// An owner without the contributor or manager role gives `validation-failed` with the field code `unknown-member`.
/// A closed workstream gives the field code `closed`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/milestones",
    operation_id = "create_milestone",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateMilestoneRequest,
    responses(
        (status = CREATED, description = "The new milestone.", body = Milestone),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_milestone(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateMilestoneRequest>,
) -> Result<(StatusCode, axum::Json<Milestone>), ApiError> {
    let input = NewMilestone {
        id: request.id,
        title: request.title,
        description: request.description,
        date: request.date,
        owner: UserId::from_uuid(request.owner_user_id),
        workstream: request.workstream_id.map(WorkstreamId::from_uuid),
    };
    let milestone =
        app::create_milestone(&caller, EventId::from_uuid(event_id), input, ports(&state)).await?;
    Ok((StatusCode::CREATED, axum::Json(milestone.into())))
}

/// Reads one milestone of the event, with its evidence and its warnings.
#[utoipa::path(
    get,
    path = "/events/{event_id}/milestones/{id}",
    operation_id = "get_milestone",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the milestone."),
    ),
    responses(
        (status = OK, description = "The milestone.", body = Milestone),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_milestone(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::Json<Milestone>, ApiError> {
    let milestone = app::get_milestone(
        &caller,
        EventId::from_uuid(event_id),
        MilestoneId::from_uuid(id),
        ports(&state),
    )
    .await?;
    Ok(axum::Json(milestone.into()))
}

/// Changes a milestone. Its owner, the lead of its workstream or an event manager can do it.
/// A status that the current status cannot change to gives `invalid-transition`; so does the current status itself.
/// `canceled` is final.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/milestones/{id}",
    operation_id = "change_milestone",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the milestone."),
    ),
    request_body = ChangeMilestoneRequest,
    responses(
        (status = OK, description = "The changed milestone.", body = Milestone),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_milestone(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeMilestoneRequest>,
) -> Result<axum::Json<Milestone>, ApiError> {
    let change = MilestoneChange {
        title: request.title,
        description: request.description,
        date: request.date,
        owner: request.owner_user_id.map(UserId::from_uuid),
        workstream: request
            .workstream_id
            .map(|id| id.map(WorkstreamId::from_uuid)),
        status: request.status.map(DomainStatus::from),
        expected_version: record_version(request.expected_version)?,
    };
    let milestone = app::change_milestone(
        &caller,
        EventId::from_uuid(event_id),
        MilestoneId::from_uuid(id),
        change,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(milestone.into()))
}
