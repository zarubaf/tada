//! `/api/v1/organization/pm-policy`: the reminder policy of the organization (ADR 0076).

use axum::extract::State;
use jiff::civil::Time;
use serde::{Deserialize, Serialize};
use tada_app::domain::RecordVersion;
use tada_app::domain::reminders::PmPolicy as AppPmPolicy;
use tada_app::pm::policy::{self as app, PmPolicyError, PmPolicyInput};
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
        .routes(routes!(get_pm_policy))
        .routes(routes!(set_pm_policy))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "get_pm_policy",
            codes(&[AUTHENTICATED, PmPolicyError::CODES]),
        ),
        (
            "set_pm_policy",
            codes(&[
                AUTHENTICATED,
                PmPolicyError::CODES,
                JSON_BODY,
                RECENT_SIGN_IN,
            ]),
        ),
    ]
}

/// The reminder policy of the organization. A time is a local time `HH:MM:SS`; a request can leave out the seconds.
#[derive(Debug, Serialize, ToSchema)]
pub struct PmPolicy {
    /// The local time of the daily check, in `time_zone`.
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "08:30:00")]
    pub local_time: Time,
    /// The IANA time zone of the daily check, for example `Europe/Zurich`.
    pub time_zone: String,
    /// The days of the daily check as bits: bit 0 is Monday, bit 6 is Sunday.
    #[schema(minimum = 1, maximum = 127)]
    pub weekdays: u8,
    /// The start of the quiet hours, in the local time of each recipient.
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "20:00:00")]
    pub quiet_start: Time,
    /// The end of the quiet hours, in the local time of each recipient.
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "08:00:00")]
    pub quiet_end: Time,
    /// The time after a digest in which a person gets no other digest.
    #[schema(minimum = 1, maximum = 168)]
    pub cooldown_hours: u8,
    /// The working days without a response after which an item comes again.
    #[schema(minimum = 1, maximum = 10)]
    pub follow_up_working_days: u8,
    /// The working days ahead in which a due date makes an item due soon.
    #[schema(minimum = 0, maximum = 10)]
    pub look_ahead_working_days: u8,
    /// True stops all reminders of the organization.
    pub paused: bool,
    /// True lets the AI word the digest.
    pub ai_enabled: bool,
    /// The record version. `SetPmPolicy` needs it. The defaults have version 1.
    pub version: i64,
}

impl PmPolicy {
    fn new(policy: &AppPmPolicy, version: RecordVersion) -> Self {
        Self {
            local_time: policy.schedule.time,
            time_zone: policy.schedule.zone.as_str().to_owned(),
            weekdays: policy.schedule.weekdays.bits(),
            quiet_start: policy.quiet.start,
            quiet_end: policy.quiet.end,
            cooldown_hours: policy.cooldown_hours,
            follow_up_working_days: policy.follow_up_working_days,
            look_ahead_working_days: policy.look_ahead_working_days,
            paused: policy.paused,
            ai_enabled: policy.ai_enabled,
            version: version.get(),
        }
    }
}

/// The input of `SetPmPolicy`: all values of the policy.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPmPolicyRequest {
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "08:30:00")]
    pub local_time: Time,
    pub time_zone: String,
    #[schema(minimum = 1, maximum = 127)]
    pub weekdays: i64,
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "20:00:00")]
    pub quiet_start: Time,
    #[schema(value_type = String, pattern = r"^([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9])?$", example = "08:00:00")]
    pub quiet_end: Time,
    #[schema(minimum = 1, maximum = 168)]
    pub cooldown_hours: i64,
    #[schema(minimum = 1, maximum = 10)]
    pub follow_up_working_days: i64,
    #[schema(minimum = 0, maximum = 10)]
    pub look_ahead_working_days: i64,
    pub paused: bool,
    pub ai_enabled: bool,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Shows the reminder policy. An owner or an admin can read it.
#[utoipa::path(
    get,
    path = "/organization/pm-policy",
    operation_id = "get_pm_policy",
    tag = "organization",
    responses(
        (status = OK, description = "The reminder policy.", body = PmPolicy),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_pm_policy(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<PmPolicy>, ApiError> {
    let (policy, version) = app::get_pm_policy(&caller, state.pm_policy.as_ref()).await?;
    Ok(axum::Json(PmPolicy::new(&policy, version)))
}

/// Replaces the reminder policy. An owner or an admin can do this.
/// A change of `ai_enabled` also needs a recent sign-in.
/// The daily check moves to the new time at once.
#[utoipa::path(
    post,
    path = "/organization/pm-policy/set",
    operation_id = "set_pm_policy",
    tag = "organization",
    request_body = SetPmPolicyRequest,
    responses(
        (status = OK, description = "The new reminder policy.", body = PmPolicy),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_pm_policy(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<SetPmPolicyRequest>,
) -> Result<axum::Json<PmPolicy>, ApiError> {
    let expected = record_version(request.expected_version)?;
    let input = PmPolicyInput {
        local_time: request.local_time,
        time_zone: request.time_zone,
        weekdays: request.weekdays,
        quiet_start: request.quiet_start,
        quiet_end: request.quiet_end,
        cooldown_hours: request.cooldown_hours,
        follow_up_working_days: request.follow_up_working_days,
        look_ahead_working_days: request.look_ahead_working_days,
        paused: request.paused,
        ai_enabled: request.ai_enabled,
    };
    let (policy, version) = app::set_pm_policy(
        &caller,
        input,
        expected,
        state.pm_policy.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(PmPolicy::new(&policy, version)))
}
