//! `/api/v1/organization/privacy-notice`: the privacy notice of the organization (ADR 0045).

use axum::extract::State;
use serde::{Deserialize, Deserializer, Serialize};
use tada_app::privacy::{self as app, PrivacyError, PrivacyNotice as AppPrivacyNotice};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, codes};
use crate::extract::{Caller, Json, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_privacy_notice))
        .routes(routes!(set_privacy_notice))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let privacy = |extra: &[&[ProblemCode]]| {
        let mut lists = vec![AUTHENTICATED, PrivacyError::CODES];
        lists.extend_from_slice(extra);
        codes(&lists)
    };
    vec![
        ("get_privacy_notice", privacy(&[])),
        ("set_privacy_notice", privacy(&[JSON_BODY])),
    ]
}

/// The privacy notice of the organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct PrivacyNotice {
    /// The Markdown text of the owner, or null if the template of the web client applies.
    #[schema(required = true, nullable = true)]
    pub markdown: Option<String>,
    /// The record version. `SetPrivacyNotice` needs it.
    pub version: i64,
}

impl From<AppPrivacyNotice> for PrivacyNotice {
    fn from(notice: AppPrivacyNotice) -> Self {
        Self {
            markdown: notice.markdown,
            version: notice.version.get(),
        }
    }
}

/// The input of `SetPrivacyNotice`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPrivacyNoticeRequest {
    /// The new Markdown text, 1 to 20000 characters. Null gives the template back.
    /// The field is required, so that a missing field cannot reset the text.
    #[serde(deserialize_with = "present")]
    #[schema(required = true, nullable = true)]
    pub markdown: Option<String>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// A field that must be in the body, also with the value null.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Option::deserialize(deserializer)
}

/// Shows the privacy notice. Each member can read it.
#[utoipa::path(
    get,
    path = "/organization/privacy-notice",
    operation_id = "get_privacy_notice",
    tag = "organization",
    responses(
        (status = OK, description = "The privacy notice.", body = PrivacyNotice),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_privacy_notice(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<PrivacyNotice>, ApiError> {
    let notice = app::get_privacy_notice(&caller, state.privacy.as_ref()).await?;
    Ok(axum::Json(notice.into()))
}

/// Replaces the privacy notice. Only an owner can do this.
#[utoipa::path(
    post,
    path = "/organization/privacy-notice/set",
    operation_id = "set_privacy_notice",
    tag = "organization",
    request_body = SetPrivacyNoticeRequest,
    responses(
        (status = OK, description = "The new privacy notice.", body = PrivacyNotice),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_privacy_notice(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<SetPrivacyNoticeRequest>,
) -> Result<axum::Json<PrivacyNotice>, ApiError> {
    let notice = app::set_privacy_notice(
        &caller,
        request.markdown,
        record_version(request.expected_version)?,
        state.privacy.as_ref(),
    )
    .await?;
    Ok(axum::Json(notice.into()))
}
