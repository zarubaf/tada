//! `/api/v1/organization/retention`: the retention periods of the organization (ADR 0071).

use axum::extract::State;
use serde::{Deserialize, Deserializer, Serialize};
use tada_app::problem::ProblemCode;
use tada_app::retention::{self as app, Retention as AppRetention, RetentionError};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, RECENT_SIGN_IN, codes};
use crate::extract::{Caller, Json, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_retention))
        .routes(routes!(set_retention))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let retention = |extra: &[&[ProblemCode]]| {
        let mut lists = vec![AUTHENTICATED, RetentionError::CODES];
        lists.extend_from_slice(extra);
        codes(&lists)
    };
    vec![
        ("get_retention", retention(&[])),
        ("set_retention", retention(&[JSON_BODY, RECENT_SIGN_IN])),
    ]
}

/// The retention periods of the organization, in months.
#[derive(Debug, Serialize, ToSchema)]
pub struct Retention {
    /// A closed event is redacted this many months after it closed. Null means that retention is off.
    #[schema(required = true, nullable = true, minimum = 1, maximum = 120)]
    pub closed_event_months: Option<i64>,
    /// A source that belongs to no event is redacted this many months after its creation.
    /// Null means that retention is off.
    #[schema(required = true, nullable = true, minimum = 1, maximum = 120)]
    pub source_months: Option<i64>,
    /// The record version. `SetRetention` needs it.
    pub version: i64,
}

impl From<AppRetention> for Retention {
    fn from(retention: AppRetention) -> Self {
        let months = |months: Option<app::Months>| months.map(|months| i64::from(months.get()));
        Self {
            closed_event_months: months(retention.periods.closed_event),
            source_months: months(retention.periods.source),
            version: retention.version.get(),
        }
    }
}

/// The input of `SetRetention`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetRetentionRequest {
    /// The months from 1 to 120, or null to switch retention off.
    /// The field is required, so that a missing field cannot switch a period off.
    #[serde(deserialize_with = "present")]
    #[schema(required = true, nullable = true, minimum = 1, maximum = 120)]
    pub closed_event_months: Option<i64>,
    /// The months from 1 to 120, or null to switch retention off. The field is required.
    #[serde(deserialize_with = "present")]
    #[schema(required = true, nullable = true, minimum = 1, maximum = 120)]
    pub source_months: Option<i64>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// A field that must be in the body, also with the value null.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Option::deserialize(deserializer)
}

/// Shows the retention periods. An owner or an admin can read them.
#[utoipa::path(
    get,
    path = "/organization/retention",
    operation_id = "get_retention",
    tag = "organization",
    responses(
        (status = OK, description = "The retention periods.", body = Retention),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_retention(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<Retention>, ApiError> {
    let retention = app::get_retention(&caller, state.retention.as_ref()).await?;
    Ok(axum::Json(retention.into()))
}

/// Replaces both retention periods. Only an owner can do this.
/// Retention is off for a period until an owner sets it.
#[utoipa::path(
    post,
    path = "/organization/retention/set",
    operation_id = "set_retention",
    tag = "organization",
    request_body = SetRetentionRequest,
    responses(
        (status = OK, description = "The new retention periods.", body = Retention),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_retention(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<SetRetentionRequest>,
) -> Result<axum::Json<Retention>, ApiError> {
    let retention = app::set_retention(
        &caller,
        request.closed_event_months,
        request.source_months,
        record_version(request.expected_version)?,
        state.retention.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(retention.into()))
}
