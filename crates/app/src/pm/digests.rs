//! The last digests of an organization, for the AI PM page (ADR 0076). An owner or an admin reads them.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use uuid::Uuid;

use crate::access::Principal;
use crate::caller::{MemberCaller, OrgScope};
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

/// The page shows this many digests, the newest first.
pub const RECENT_DIGESTS: u32 = 50;

/// The state of a digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestStatus {
    /// It waits for its send, for example for the end of the quiet hours.
    Pending,
    Sent,
    /// Telegram refused it, or the recipient blocked the bot. Nothing was sent.
    Failed,
    /// Telegram can have delivered it. Nobody sends it again.
    Unknown,
    /// Nothing was left to send, the recipient was no longer eligible, or the policy was paused.
    Canceled,
}

impl DigestStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Canceled => "canceled",
        }
    }

    /// Returns `None` for a code that is not a status.
    pub fn parse(code: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Sent,
            Self::Failed,
            Self::Unknown,
            Self::Canceled,
        ]
        .into_iter()
        .find(|status| status.as_str() == code)
    }
}

/// One digest of the organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestSummary {
    pub id: Uuid,
    pub user_display_name: String,
    /// The time of the daily check that planned it.
    pub created_at: Timestamp,
    /// The time of the send. Only a sent digest has one.
    pub sent_at: Option<Timestamp>,
    pub status: DigestStatus,
    /// The items of the digest: after the send, the items that it showed.
    pub item_count: u32,
}

/// The repository port of the list.
#[async_trait]
pub trait DigestLog: Debug + Send + Sync {
    /// The last `limit` digests of the organization, the newest first.
    async fn recent(&self, scope: OrgScope, limit: u32) -> Result<Vec<DigestSummary>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum DigestLogError {
    /// Only an owner or an admin reads the digests.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl DigestLogError {
    /// All codes of the list, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for DigestLogError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            Self::Forbidden => None,
        }
    }
}

/// The last digests of the caller's organization. An owner or an admin can read them.
pub async fn recent_digests(
    caller: &MemberCaller,
    store: &dyn DigestLog,
) -> Result<Vec<DigestSummary>, DigestLogError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(DigestLogError::Forbidden);
    }
    Ok(store.recent(caller.scope(), RECENT_DIGESTS).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_status_has_its_code() {
        for status in [
            DigestStatus::Pending,
            DigestStatus::Sent,
            DigestStatus::Failed,
            DigestStatus::Unknown,
            DigestStatus::Canceled,
        ] {
            assert_eq!(DigestStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(DigestStatus::parse("queued"), None);
    }
}
