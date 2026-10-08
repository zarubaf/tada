//! Telegram identity linking (ADR 0011).
//!
//! 1. A member asks for a link code in the web client.
//! 2. The member sends the code to the bot. The gateway claims the code for the sending Telegram account.
//! 3. The member sees the Telegram account in the web client and confirms the link.
//!
//! A phishing link alone cannot bind an account, because only the member's own session can confirm.

use std::fmt::{self, Debug};

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use tada_domain::ids::UserId;
use uuid::Uuid;

use crate::caller::{MemberCaller, OrgScope, ServiceCaller, TelegramGateway};
use crate::clock::Clock;
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

/// A link code expires after 10 minutes (ADR 0011).
pub const CODE_LIFETIME: SignedDuration = SignedDuration::from_mins(10);

/// The ID of a Telegram account: a direct identifier. `Debug` never shows it (ADR 0035).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TelegramUserId(pub i64);

impl Debug for TelegramUserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TelegramUserId(redacted)")
    }
}

/// The name of a Telegram account, as Telegram sends it. `Debug` never shows it (ADR 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct TelegramName(pub String);

impl Debug for TelegramName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TelegramName(redacted)")
    }
}

/// A new link code. Only the member who asked for it sees the code, once.
#[derive(Clone, PartialEq, Eq)]
pub struct LinkCode {
    pub code: String,
    pub expires_at: Timestamp,
}

impl Debug for LinkCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkCode")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// A claimed code that waits for the confirmation of the member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRequest {
    pub id: Uuid,
    pub telegram_user_id: TelegramUserId,
    pub telegram_name: TelegramName,
    pub claimed_at: Timestamp,
}

/// The repository port of the linking.
#[async_trait]
pub trait TelegramLinks: Debug + Send + Sync {
    /// Stores the hash of a new random code and returns the code.
    async fn create_code(
        &self,
        scope: OrgScope,
        user_id: UserId,
        expires_at: Timestamp,
    ) -> Result<String, StoreError>;

    /// Marks an unexpired, unclaimed code as claimed by the account. Returns false for any other code.
    /// Infrastructure query (ADR 0039): the code finds its organization.
    async fn claim(
        &self,
        code: &str,
        account: TelegramUserId,
        name: &TelegramName,
        now: Timestamp,
    ) -> Result<bool, StoreError>;

    /// The claimed, unconfirmed and unexpired codes of the user.
    async fn requests(
        &self,
        scope: OrgScope,
        user_id: UserId,
        now: Timestamp,
    ) -> Result<Vec<LinkRequest>, StoreError>;

    /// Binds the account of the request to the user, in one transaction with the confirmation.
    async fn confirm(
        &self,
        scope: OrgScope,
        user_id: UserId,
        request_id: Uuid,
        now: Timestamp,
    ) -> Result<Confirmed, StoreError>;

    /// Records an update ID. Returns false if the gateway saw it before.
    /// Infrastructure query (ADR 0039): a Telegram update ID has no organization.
    async fn record_update(&self, update_id: i64) -> Result<bool, StoreError>;
}

/// The result of `TelegramLinks::confirm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmed {
    Linked(TelegramUserId),
    /// No open request with this ID for the user.
    NotFound,
    /// The Telegram account or the user has another link already.
    AlreadyLinked,
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("no open link request with this ID")]
    NotFound,
    #[error("the Telegram account or the user has another link")]
    AlreadyLinked,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl LinkError {
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::InvalidTransition,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for LinkError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::AlreadyLinked => ProblemCode::InvalidTransition,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

/// Step 1: a new link code for the calling member.
pub async fn create_link_code(
    caller: &MemberCaller,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<LinkCode, StoreError> {
    let expires_at = clock.now() + CODE_LIFETIME;
    let code = links
        .create_code(caller.scope(), caller.user_id(), expires_at)
        .await?;
    Ok(LinkCode { code, expires_at })
}

/// Step 2: the gateway claims a code for the Telegram account that sent it.
pub async fn claim_link_code(
    _caller: &ServiceCaller<TelegramGateway>,
    code: &str,
    account: TelegramUserId,
    name: &TelegramName,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<bool, StoreError> {
    links.claim(code.trim(), account, name, clock.now()).await
}

/// The open requests of the calling member, for the confirmation in step 3.
pub async fn list_link_requests(
    caller: &MemberCaller,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<Vec<LinkRequest>, StoreError> {
    links
        .requests(caller.scope(), caller.user_id(), clock.now())
        .await
}

/// Step 3: the member confirms a request. Only then does tada bind the account to the user.
pub async fn confirm_link(
    caller: &MemberCaller,
    request_id: Uuid,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<TelegramUserId, LinkError> {
    match links
        .confirm(caller.scope(), caller.user_id(), request_id, clock.now())
        .await?
    {
        Confirmed::Linked(account) => Ok(account),
        Confirmed::NotFound => Err(LinkError::NotFound),
        Confirmed::AlreadyLinked => Err(LinkError::AlreadyLinked),
    }
}
