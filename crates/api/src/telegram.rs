//! `/api/v1/telegram`: identity linking (ADR 0011).

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jiff::Timestamp;
use serde::Serialize;
use tada_app::problem::ProblemCode;
use tada_app::telegram::{self as app, LinkError, LinkRequest};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, codes};
use crate::extract::Caller;
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(create_link_code))
        .routes(routes!(list_link_requests))
        .routes(routes!(confirm_link))
}

pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        ("create_telegram_link_code", codes(&[AUTHENTICATED])),
        ("list_telegram_link_requests", codes(&[AUTHENTICATED])),
        (
            "confirm_telegram_link",
            codes(&[AUTHENTICATED, LinkError::CODES]),
        ),
    ]
}

/// A new link code. The member sends it to the bot within ten minutes.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramLinkCode {
    /// The code. It works once. The server shows it only in this response.
    pub code: String,
    pub expires_at: Timestamp,
}

/// Creates a link code for the calling member.
#[utoipa::path(
    post,
    path = "/telegram/link-codes",
    operation_id = "create_telegram_link_code",
    tag = "telegram",
    responses(
        (status = CREATED, description = "The new code.", body = TelegramLinkCode),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_link_code(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<Response, ApiError> {
    let code = app::create_link_code(&caller, state.telegram.as_ref(), state.clock.as_ref())
        .await
        .map_err(|error| ApiError::store(&error))?;
    let body = TelegramLinkCode {
        code: code.code,
        expires_at: code.expires_at,
    };
    let mut response = (StatusCode::CREATED, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// A Telegram account that sent a code of the member, and that waits for the confirmation.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramLinkRequest {
    pub id: Uuid,
    pub telegram_user_id: i64,
    /// The name of the Telegram account, as Telegram sends it.
    pub telegram_name: String,
    pub claimed_at: Timestamp,
}

impl From<LinkRequest> for TelegramLinkRequest {
    fn from(request: LinkRequest) -> Self {
        Self {
            id: request.id,
            telegram_user_id: request.telegram_user_id.0,
            telegram_name: request.telegram_name.0,
            claimed_at: request.claimed_at,
        }
    }
}

/// The open link requests of the member. A member has few, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramLinkRequestPage {
    pub items: Vec<TelegramLinkRequest>,
}

/// Lists the open link requests of the calling member.
#[utoipa::path(
    get,
    path = "/telegram/link-requests",
    operation_id = "list_telegram_link_requests",
    tag = "telegram",
    responses(
        (status = OK, description = "The open requests.", body = TelegramLinkRequestPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_link_requests(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<TelegramLinkRequestPage>, ApiError> {
    let requests = app::list_link_requests(&caller, state.telegram.as_ref(), state.clock.as_ref())
        .await
        .map_err(|error| ApiError::store(&error))?;
    Ok(axum::Json(TelegramLinkRequestPage {
        items: requests
            .into_iter()
            .map(TelegramLinkRequest::from)
            .collect(),
    }))
}

/// The linked Telegram account.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramLink {
    pub telegram_user_id: i64,
}

/// Confirms a link request. Only then does tada bind the Telegram account to the member.
#[utoipa::path(
    post,
    path = "/telegram/link-requests/{request_id}/confirm",
    operation_id = "confirm_telegram_link",
    tag = "telegram",
    params(("request_id" = Uuid, Path, description = "The ID of the link request.")),
    responses(
        (status = OK, description = "The account is linked.", body = TelegramLink),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn confirm_link(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(request_id): Path<Uuid>,
) -> Result<axum::Json<TelegramLink>, ApiError> {
    match app::confirm_link(
        &caller,
        request_id,
        state.telegram.as_ref(),
        state.clock.as_ref(),
    )
    .await
    {
        Ok(account) => Ok(axum::Json(TelegramLink {
            telegram_user_id: account.0,
        })),
        Err(LinkError::NotFound) => Err(ApiError::new(ProblemCode::NotFound)),
        Err(LinkError::AlreadyLinked) => Err(ApiError::new(ProblemCode::InvalidTransition)),
        Err(LinkError::Store(error)) => Err(ApiError::store(&error)),
    }
}
