//! `/api/v1/organization/digests`: the last digests of the AI PM in the organization (ADR 0076).

use axum::extract::State;
use jiff::Timestamp;
use serde::Serialize;
use tada_app::pm::digests::{
    self as app, DigestLogError, DigestStatus as AppDigestStatus, DigestSummary,
};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, codes};
use crate::extract::Caller;
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_digests))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![(
        "list_digests",
        codes(&[AUTHENTICATED, DigestLogError::CODES]),
    )]
}

/// The state of a digest.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DigestStatus {
    /// It waits for its send, for example for the end of the quiet hours.
    Pending,
    Sent,
    /// Telegram refused it, or the recipient blocked the bot. Nothing was sent.
    Failed,
    /// Telegram can have delivered it. tada does not send it again.
    Unknown,
    /// Nothing was left to send, the recipient could no longer get it, or the reminders were paused.
    Canceled,
}

impl From<AppDigestStatus> for DigestStatus {
    fn from(status: AppDigestStatus) -> Self {
        match status {
            AppDigestStatus::Pending => Self::Pending,
            AppDigestStatus::Sent => Self::Sent,
            AppDigestStatus::Failed => Self::Failed,
            AppDigestStatus::Unknown => Self::Unknown,
            AppDigestStatus::Canceled => Self::Canceled,
        }
    }
}

/// One digest to one member.
#[derive(Debug, Serialize, ToSchema)]
pub struct Digest {
    pub id: Uuid,
    /// The display name of the recipient.
    pub user_display_name: String,
    /// The time of the daily check that planned the digest.
    pub created_at: Timestamp,
    /// The time of the send. It is absent for a digest that was not sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<Timestamp>,
    pub status: DigestStatus,
    /// The number of items. After the send, the items that the message showed.
    pub item_count: u32,
}

impl From<DigestSummary> for Digest {
    fn from(digest: DigestSummary) -> Self {
        Self {
            id: digest.id,
            user_display_name: digest.user_display_name,
            created_at: digest.created_at,
            sent_at: digest.sent_at,
            status: digest.status.into(),
            item_count: digest.item_count,
        }
    }
}

/// The last 50 digests, the newest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct DigestList {
    pub items: Vec<Digest>,
}

/// Lists the last digests of the organization. An owner or an admin can read them.
#[utoipa::path(
    get,
    path = "/organization/digests",
    operation_id = "list_digests",
    tag = "organization",
    responses(
        (status = OK, description = "The last digests.", body = DigestList),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_digests(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<DigestList>, ApiError> {
    let digests = app::recent_digests(&caller, state.digests.as_ref()).await?;
    Ok(axum::Json(DigestList {
        items: digests.into_iter().map(Digest::from).collect(),
    }))
}
