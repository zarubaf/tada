//! `/api/v1/events/{event_id}/memberships`: event roles (ADR 0052).

use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::EventRole as DomainEventRole;
use tada_app::domain::ids::{EventId, UserId};
use tada_app::event_members::{
    self as app, AddEventMemberError, ChangeEventMemberError, EventMember, ListEventMembersError,
};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, codes};
use crate::extract::{Caller, Json, Path};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_event_memberships, add_event_membership))
        .routes(routes!(change_event_role))
        .routes(routes!(remove_event_membership))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "list_event_memberships",
            codes(&[AUTHENTICATED, PATH, ListEventMembersError::CODES]),
        ),
        (
            "add_event_membership",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, AddEventMemberError::CODES]),
        ),
        (
            "change_event_role",
            codes(&[
                AUTHENTICATED,
                PATH,
                JSON_BODY,
                ChangeEventMemberError::CODES,
            ]),
        ),
        (
            "remove_event_membership",
            codes(&[
                AUTHENTICATED,
                PATH,
                JSON_BODY,
                ChangeEventMemberError::CODES,
            ]),
        ),
    ]
}

/// The role of a member in one event.
// The names are the glossary terms and the wire values, for example `event-manager`.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EventRole {
    EventManager,
    EventContributor,
    EventViewer,
}

impl From<EventRole> for DomainEventRole {
    fn from(role: EventRole) -> Self {
        match role {
            EventRole::EventManager => Self::EventManager,
            EventRole::EventContributor => Self::EventContributor,
            EventRole::EventViewer => Self::EventViewer,
        }
    }
}

impl From<DomainEventRole> for EventRole {
    fn from(role: DomainEventRole) -> Self {
        match role {
            DomainEventRole::EventManager => Self::EventManager,
            DomainEventRole::EventContributor => Self::EventContributor,
            DomainEventRole::EventViewer => Self::EventViewer,
        }
    }
}

/// The event role of one member in one event.
#[derive(Debug, Serialize, ToSchema)]
pub struct EventMembership {
    pub user_id: Uuid,
    pub display_name: String,
    pub event_role: EventRole,
    /// The record version. A command that changes the event membership needs it.
    pub version: i64,
    pub created_at: Timestamp,
}

impl From<EventMember> for EventMembership {
    fn from(member: EventMember) -> Self {
        Self {
            user_id: member.user_id.as_uuid(),
            display_name: member.display_name.as_str().to_owned(),
            event_role: member.event_role.into(),
            version: member.version.get(),
            created_at: member.created_at,
        }
    }
}

/// The event memberships of an event. An event has few, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct EventMembershipPage {
    pub items: Vec<EventMembership>,
}

/// The input of `AddEventMembership`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddEventMembershipRequest {
    /// A member of the caller's organization.
    pub user_id: Uuid,
    pub event_role: EventRole,
}

/// The input of `ChangeEventRole`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeEventRoleRequest {
    pub event_role: EventRole,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The input of `RemoveEventMembership`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RemoveEventMembershipRequest {
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

fn record_version(value: i64) -> Result<RecordVersion, ApiError> {
    RecordVersion::new(value).ok_or_else(|| {
        ApiError::new(ProblemCode::MalformedRequest)
            .with_detail("The expected version must be 1 or more.")
    })
}

fn change_error(error: ChangeEventMemberError) -> ApiError {
    match error {
        ChangeEventMemberError::Store(error) => ApiError::store(&error),
        error => ApiError::new(error.code()),
    }
}

/// Lists the event memberships of an event. Only its event managers see them.
#[utoipa::path(
    get,
    path = "/events/{event_id}/memberships",
    operation_id = "list_event_memberships",
    tag = "events",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    responses(
        (status = OK, description = "All event memberships of the event.", body = EventMembershipPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_event_memberships(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
) -> Result<axum::Json<EventMembershipPage>, ApiError> {
    let members = app::list_event_members(
        &caller,
        EventId::from_uuid(event_id),
        state.identity.as_ref(),
        state.event_members.as_ref(),
    )
    .await
    .map_err(|error| match error {
        ListEventMembersError::Store(error) => ApiError::store(&error),
        error => ApiError::new(error.code()),
    })?;
    Ok(axum::Json(EventMembershipPage {
        items: members.into_iter().map(EventMembership::from).collect(),
    }))
}

/// Gives a member of the organization an event role. Only event managers can do this.
#[utoipa::path(
    post,
    path = "/events/{event_id}/memberships",
    operation_id = "add_event_membership",
    tag = "events",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = AddEventMembershipRequest,
    responses(
        (status = CREATED, description = "The new event membership.", body = EventMembership),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn add_event_membership(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<AddEventMembershipRequest>,
) -> Result<(StatusCode, axum::Json<EventMembership>), ApiError> {
    let member = app::add_event_member(
        &caller,
        EventId::from_uuid(event_id),
        UserId::from_uuid(request.user_id),
        request.event_role.into(),
        state.identity.as_ref(),
        state.event_members.as_ref(),
        state.clock.as_ref(),
    )
    .await
    .map_err(|error| match error {
        AddEventMemberError::Invalid(errors) => ApiError::invalid(errors),
        AddEventMemberError::Store(error) => ApiError::store(&error),
        error => ApiError::new(error.code()),
    })?;
    Ok((StatusCode::CREATED, axum::Json(member.into())))
}

/// Changes the event role of a member. Only event managers can do this.
#[utoipa::path(
    post,
    path = "/events/{event_id}/memberships/{user_id}/change-role",
    operation_id = "change_event_role",
    tag = "events",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("user_id" = Uuid, Path, description = "The ID of the member."),
    ),
    request_body = ChangeEventRoleRequest,
    responses(
        (status = OK, description = "The changed event membership.", body = EventMembership),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_event_role(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, user_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeEventRoleRequest>,
) -> Result<axum::Json<EventMembership>, ApiError> {
    let member = app::change_event_role(
        &caller,
        EventId::from_uuid(event_id),
        UserId::from_uuid(user_id),
        request.event_role.into(),
        record_version(request.expected_version)?,
        state.identity.as_ref(),
        state.event_members.as_ref(),
    )
    .await
    .map_err(change_error)?;
    Ok(axum::Json(member.into()))
}

/// Removes the event role of a member. The member loses access to the event with the next request.
#[utoipa::path(
    post,
    path = "/events/{event_id}/memberships/{user_id}/remove",
    operation_id = "remove_event_membership",
    tag = "events",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("user_id" = Uuid, Path, description = "The ID of the member."),
    ),
    request_body = RemoveEventMembershipRequest,
    responses(
        (status = NO_CONTENT, description = "The event membership is removed."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn remove_event_membership(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, user_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<RemoveEventMembershipRequest>,
) -> Result<StatusCode, ApiError> {
    app::remove_event_member(
        &caller,
        EventId::from_uuid(event_id),
        UserId::from_uuid(user_id),
        record_version(request.expected_version)?,
        state.identity.as_ref(),
        state.event_members.as_ref(),
    )
    .await
    .map_err(change_error)?;
    Ok(StatusCode::NO_CONTENT)
}
