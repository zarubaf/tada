//! `/api/v1/events/{event_id}/workstreams`: the workstreams of an event (ADR 0067).

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{EventId, UserId, WorkstreamId};
use tada_app::domain::work::WorkstreamStatus as DomainStatus;
use tada_app::problem::ProblemCode;
use tada_app::workstreams::{
    self as app, NewWorkstream, Workstream as DomainWorkstream, WorkstreamChange, WorkstreamError,
};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, codes};
use crate::extract::{Caller, Json, Path, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_workstreams, create_workstream))
        .routes(routes!(change_workstream))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "list_workstreams",
            codes(&[AUTHENTICATED, PATH, WorkstreamError::CODES]),
        ),
        (
            "create_workstream",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkstreamError::CODES]),
        ),
        (
            "change_workstream",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkstreamError::CODES]),
        ),
    ]
}

/// The status of a workstream. A closed workstream accepts no new records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WorkstreamStatus {
    Active,
    Closed,
}

impl From<WorkstreamStatus> for DomainStatus {
    fn from(status: WorkstreamStatus) -> Self {
        match status {
            WorkstreamStatus::Active => Self::Active,
            WorkstreamStatus::Closed => Self::Closed,
        }
    }
}

impl From<DomainStatus> for WorkstreamStatus {
    fn from(status: DomainStatus) -> Self {
        match status {
            DomainStatus::Active => Self::Active,
            DomainStatus::Closed => Self::Closed,
        }
    }
}

/// A workstream of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Workstream {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    /// The member who leads the workstream: a contributor or manager of the event.
    pub lead_user_id: Uuid,
    pub status: WorkstreamStatus,
    /// The record version. A command that changes the workstream needs it.
    pub version: i64,
}

impl From<DomainWorkstream> for Workstream {
    fn from(workstream: DomainWorkstream) -> Self {
        Self {
            id: workstream.id.as_uuid(),
            event_id: workstream.event_id.as_uuid(),
            name: workstream.name.as_str().to_owned(),
            lead_user_id: workstream.lead.as_uuid(),
            status: workstream.status.into(),
            version: workstream.version.get(),
        }
    }
}

/// The workstreams of an event. An event has few, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct WorkstreamPage {
    pub items: Vec<Workstream>,
}

/// The input of `CreateWorkstream`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWorkstreamRequest {
    /// The UUIDv7 of the new workstream. A client that sends it can retry the request safely.
    pub id: Option<Uuid>,
    /// 1 to 200 characters, unique in the event whatever its case.
    #[schema(example = "Gelände")]
    pub name: String,
    /// A member with the contributor or manager role in the event.
    pub lead_user_id: Uuid,
}

/// The input of `ChangeWorkstream`. A field that is absent stays as it is.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeWorkstreamRequest {
    pub name: Option<String>,
    pub lead_user_id: Option<Uuid>,
    pub status: Option<WorkstreamStatus>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Lists the workstreams of an event, in the order of their names.
#[utoipa::path(
    get,
    path = "/events/{event_id}/workstreams",
    operation_id = "list_workstreams",
    tag = "workstreams",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    responses(
        (status = OK, description = "All workstreams of the event.", body = WorkstreamPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_workstreams(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
) -> Result<axum::Json<WorkstreamPage>, ApiError> {
    let items = app::list_workstreams(
        &caller,
        EventId::from_uuid(event_id),
        state.identity.as_ref(),
        state.workstreams.as_ref(),
    )
    .await?;
    Ok(axum::Json(WorkstreamPage {
        items: items.into_iter().map(Workstream::from).collect(),
    }))
}

/// Creates a workstream in the event. Only event managers can do this.
/// A lead without the contributor or manager role gives `validation-failed` with the field code `unknown-member`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/workstreams",
    operation_id = "create_workstream",
    tag = "workstreams",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateWorkstreamRequest,
    responses(
        (status = CREATED, description = "The new workstream.", body = Workstream),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_workstream(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateWorkstreamRequest>,
) -> Result<(StatusCode, axum::Json<Workstream>), ApiError> {
    let input = NewWorkstream {
        id: request.id,
        name: request.name,
        lead: UserId::from_uuid(request.lead_user_id),
    };
    let workstream = app::create_workstream(
        &caller,
        EventId::from_uuid(event_id),
        input,
        state.identity.as_ref(),
        state.workstreams.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(workstream.into())))
}

/// Renames a workstream, changes its lead or opens or closes it. Only event managers can do this.
/// A workstream of another event is not found.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/workstreams/{id}",
    operation_id = "change_workstream",
    tag = "workstreams",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the workstream."),
    ),
    request_body = ChangeWorkstreamRequest,
    responses(
        (status = OK, description = "The changed workstream.", body = Workstream),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_workstream(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeWorkstreamRequest>,
) -> Result<axum::Json<Workstream>, ApiError> {
    let change = WorkstreamChange {
        name: request.name,
        lead: request.lead_user_id.map(UserId::from_uuid),
        status: request.status.map(DomainStatus::from),
        expected_version: record_version(request.expected_version)?,
    };
    let workstream = app::change_workstream(
        &caller,
        EventId::from_uuid(event_id),
        WorkstreamId::from_uuid(id),
        change,
        state.identity.as_ref(),
        state.workstreams.as_ref(),
    )
    .await?;
    Ok(axum::Json(workstream.into()))
}
