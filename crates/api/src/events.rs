//! `/api/v1/events`: the `CreateEvent` command and the `ListEvents` and `GetEvent` queries.

use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::access::AccessError;
use tada_app::domain::events::{self as domain, EventKey};
use tada_app::domain::ids::EventId;
use tada_app::events::{
    self as app, CreateEventError, Created, EventCursor, ListEventsError, NewEvent,
};
use tada_app::problem::ProblemCode;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_events, create_event))
        .routes(routes!(get_event))
}

/// The problem codes of each operation (ADR 0037). They come from the error types of the `app` crate
/// and from the extractors, so they cannot drift from the handlers.
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "create_event",
            codes(&[AUTHENTICATED, JSON_BODY, CreateEventError::CODES]),
        ),
        (
            "list_events",
            codes(&[AUTHENTICATED, QUERY, ListEventsError::CODES]),
        ),
        (
            "get_event",
            codes(&[AUTHENTICATED, PATH, AccessError::CODES]),
        ),
    ]
}

/// An event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Event {
    pub id: Uuid,
    /// The short key of the event, unique in its organization, for example `FLY28`.
    pub key: String,
    pub name: String,
    /// The IANA time zone of the event, for example `Europe/Zurich`.
    pub time_zone: String,
    /// The record version. A command that changes the event needs it.
    pub version: i64,
    pub created_at: Timestamp,
}

impl From<domain::Event> for Event {
    fn from(event: domain::Event) -> Self {
        Self {
            id: event.id.as_uuid(),
            key: event.key.as_str().to_owned(),
            name: event.name.as_str().to_owned(),
            time_zone: event.time_zone.as_str().to_owned(),
            version: event.version.get(),
            created_at: event.created_at,
        }
    }
}

/// The input of `CreateEvent`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEventRequest {
    /// The UUIDv7 of the new event. A client that sends it can retry the request safely.
    pub id: Option<Uuid>,
    /// Two to eight capital letters and digits, unique in the organization.
    #[schema(example = "TEST30")]
    pub key: String,
    /// 1 to 200 characters.
    #[schema(example = "Open Day Testwil")]
    pub name: String,
    /// An IANA time zone. The default is `Europe/Zurich`.
    pub time_zone: Option<String>,
}

/// Creates an event in the caller's organization.
///
/// If an event with the same `id` and the same content exists, the response is this event with the
/// status 200, and nothing changes.
#[utoipa::path(
    post,
    path = "/events",
    operation_id = "create_event",
    tag = "events",
    request_body = CreateEventRequest,
    responses(
        (status = CREATED, description = "The new event.", body = Event),
        (status = OK, description = "The event exists with this ID and the same content.", body = Event),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_event(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreateEventRequest>,
) -> Result<(StatusCode, axum::Json<Event>), ApiError> {
    let input = NewEvent {
        id: request.id,
        key: request.key,
        name: request.name,
        time_zone: request.time_zone,
    };
    match app::create_event(&caller, input, state.events.as_ref(), state.clock.as_ref()).await? {
        Created::New(event) => Ok((StatusCode::CREATED, axum::Json(event.into()))),
        Created::Existing(event) => Ok((StatusCode::OK, axum::Json(event.into()))),
    }
}

/// The parameters of `ListEvents`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListEventsQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
}

/// One page of events.
#[derive(Debug, Serialize, ToSchema)]
pub struct EventPage {
    pub items: Vec<Event>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Lists the events that the caller can see, in the order of their keys.
#[utoipa::path(
    get,
    path = "/events",
    operation_id = "list_events",
    tag = "events",
    params(ListEventsQuery),
    responses(
        (status = OK, description = "One page of events.", body = EventPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_events(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Query(query): Query<ListEventsQuery>,
) -> Result<axum::Json<EventPage>, ApiError> {
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_events(&caller, after, limit, state.events.as_ref()).await?;
    Ok(axum::Json(EventPage {
        items: page.items.into_iter().map(Event::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Reads one event. The caller needs an event role in it, or the organization role owner or admin.
#[utoipa::path(
    get,
    path = "/events/{event_id}",
    operation_id = "get_event",
    tag = "events",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    responses(
        (status = OK, description = "The event.", body = Event),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_event(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
) -> Result<axum::Json<Event>, ApiError> {
    let event = app::get_event(
        &caller,
        EventId::from_uuid(event_id),
        state.events.as_ref(),
        state.identity.as_ref(),
    )
    .await?;
    Ok(axum::Json(event.into()))
}

/// The cursor is opaque for clients (ADR 0044): the key and the ID, in Base64.
fn encode_cursor(cursor: &EventCursor) -> String {
    cursor::encode(format!("{} {}", cursor.key.as_str(), cursor.id))
}

fn decode_cursor(text: &str) -> Result<EventCursor, ApiError> {
    let text = cursor::decode_text(text)?;
    let (key, id) = text.split_once(' ').ok_or_else(cursor::invalid)?;
    Ok(EventCursor {
        key: EventKey::parse(key).map_err(|_| cursor::invalid())?,
        id: EventId::from_uuid(id.parse().map_err(|_| cursor::invalid())?),
    })
}
