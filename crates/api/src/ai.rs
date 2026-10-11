//! `/api/v1/organization/ai`: the AI state and the monthly AI cap of the organization (ADRs 0047 and 0077).

use axum::extract::State;
use serde::{Deserialize, Serialize};
use tada_app::ai_cost::{self as app, AiCapError, AiStatus as AppAiStatus};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, RECENT_SIGN_IN, codes};
use crate::extract::{Caller, Json, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_ai))
        .routes(routes!(set_ai_cap))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        ("get_ai", codes(&[AUTHENTICATED, AiCapError::CODES])),
        (
            "set_ai_cap",
            codes(&[AUTHENTICATED, AiCapError::CODES, JSON_BODY, RECENT_SIGN_IN]),
        ),
    ]
}

/// The AI state of the organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct AiStatus {
    /// True if the organization switched the AI work on in its AI-PM policy.
    pub enabled: bool,
    /// True if the operator set up the model provider. Without it, no AI work runs.
    pub configured: bool,
    /// The monthly cap in Swiss francs, as a decimal number in text, for example `25.00`.
    #[schema(pattern = r"^[0-9]+(\.[0-9]{1,5})?$")]
    pub cap_chf: String,
    /// The AI cost of the current calendar month (UTC) in Swiss francs, as a decimal number in text.
    #[schema(pattern = r"^[0-9]+(\.[0-9]{1,5})?$")]
    pub used_chf: String,
    /// The record version of the cap. `SetAiCap` needs it.
    pub version: i64,
}

impl From<AppAiStatus> for AiStatus {
    fn from(status: AppAiStatus) -> Self {
        Self {
            enabled: status.enabled,
            configured: status.configured,
            cap_chf: status.cap.to_string(),
            used_chf: status.used.to_string(),
            version: status.version.get(),
        }
    }
}

/// The input of `SetAiCap`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetAiCapRequest {
    /// The new monthly cap in Swiss francs, more than zero, with at most 5 decimal places, for example `40.50`.
    #[schema(pattern = r"^[0-9]+(\.[0-9]{1,5})?$")]
    pub cap_chf: String,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Shows the AI state and the monthly AI cap. An owner or an admin can read them.
#[utoipa::path(
    get,
    path = "/organization/ai",
    operation_id = "get_ai",
    tag = "organization",
    responses(
        (status = OK, description = "The AI state.", body = AiStatus),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_ai(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<AiStatus>, ApiError> {
    let status = app::get_ai_status(
        &caller,
        state.ai.as_ref(),
        state.ai_setup,
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(status.into()))
}

/// Sets the monthly AI cap. An owner or an admin with a recent sign-in can do this.
/// At the cap, the AI work pauses until the next month; manual planning continues.
#[utoipa::path(
    post,
    path = "/organization/ai/set-cap",
    operation_id = "set_ai_cap",
    tag = "organization",
    request_body = SetAiCapRequest,
    responses(
        (status = OK, description = "The AI state with the new cap.", body = AiStatus),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_ai_cap(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<SetAiCapRequest>,
) -> Result<axum::Json<AiStatus>, ApiError> {
    let status = app::set_ai_cap(
        &caller,
        &request.cap_chf,
        record_version(request.expected_version)?,
        state.ai.as_ref(),
        state.ai_setup,
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(status.into()))
}
