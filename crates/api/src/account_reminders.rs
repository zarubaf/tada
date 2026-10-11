//! `/api/v1/account/reminders`: the reminder settings of the own account (ADR 0076).

use axum::extract::State;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::account_reminders::{
    self as app, ReminderSettings as AppSettings, ReminderSettingsError,
};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, codes};
use crate::extract::{Caller, Json};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_reminder_settings))
        .routes(routes!(set_reminder_settings))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "get_reminder_settings",
            codes(&[AUTHENTICATED, ReminderSettingsError::CODES]),
        ),
        (
            "set_reminder_settings",
            codes(&[AUTHENTICATED, ReminderSettingsError::CODES, JSON_BODY]),
        ),
    ]
}

/// The reminder settings of the own account. They apply in each organization of the user.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReminderSettings {
    /// The time of the opt-in, or null if the user gets no reminders.
    #[schema(required = true, nullable = true)]
    pub opted_in_at: Option<Timestamp>,
    /// The IANA time zone of the user. The quiet hours use it.
    pub time_zone: String,
    /// True if the user has a confirmed Telegram link. An opt-in needs it.
    pub telegram_linked: bool,
}

impl From<AppSettings> for ReminderSettings {
    fn from(settings: AppSettings) -> Self {
        Self {
            opted_in_at: settings.opted_in_at,
            time_zone: settings.time_zone.as_str().to_owned(),
            telegram_linked: settings.telegram_linked,
        }
    }
}

/// The input of `SetReminderSettings`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetReminderSettingsRequest {
    /// True opts in, false opts out. An opt-in without a confirmed Telegram link gets the field code
    /// `telegram-not-linked`.
    pub opt_in: bool,
    /// An IANA time zone, for example `Europe/Zurich`.
    pub time_zone: String,
}

/// Shows the reminder settings of the own account.
#[utoipa::path(
    get,
    path = "/account/reminders",
    operation_id = "get_reminder_settings",
    tag = "account",
    responses(
        (status = OK, description = "The reminder settings.", body = ReminderSettings),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_reminder_settings(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<ReminderSettings>, ApiError> {
    let settings = app::get_reminder_settings(&caller, state.reminder_settings.as_ref()).await?;
    Ok(axum::Json(settings.into()))
}

/// Opts the own account in or out of reminders and sets its time zone.
#[utoipa::path(
    post,
    path = "/account/reminders/set",
    operation_id = "set_reminder_settings",
    tag = "account",
    request_body = SetReminderSettingsRequest,
    responses(
        (status = OK, description = "The new reminder settings.", body = ReminderSettings),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_reminder_settings(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<SetReminderSettingsRequest>,
) -> Result<axum::Json<ReminderSettings>, ApiError> {
    let settings = app::set_reminder_settings(
        &caller,
        request.opt_in,
        request.time_zone,
        state.reminder_settings.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(settings.into()))
}
