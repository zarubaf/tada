//! Telegram identity linking (ADR 0011).
//!
//! 1. A member asks for a link code in the web client.
//! 2. The member sends the code to the bot. The gateway claims the code for the sending Telegram
//!    account and names the tada account of the code. The Telegram account accepts the claim with
//!    `/bestaetigen`.
//! 3. The member sees the Telegram account in the web client and confirms the link.
//!
//! A phishing link alone cannot bind an account: the web session of the member confirms the
//! Telegram account, and the Telegram account accepts the tada account. Both sides can end a link.

use std::fmt::{self, Debug};
use std::ops::Range;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use tada_domain::events::{Event, EventKey};
use tada_domain::facts::{FactValue, FieldKey, FieldStatus, ValueError};
use tada_domain::identity::Email;
use tada_domain::ids::{ChangesetId, UserId};
use tada_domain::sources::SourceText;
use uuid::Uuid;

use crate::access::{self, AccessError};
use crate::caller::{MemberCaller, OrgScope, ServiceCaller, TelegramGateway};
use crate::clock::Clock;
use crate::events::EventStore;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::{
    Created, FactStateInput, NewChangeset, NewProposal, OperationInput, PassageInput, ProposeError,
    ProposeStores, ValueInput, create_changeset,
};
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

/// The tada account of a link code, as the bot names it to the Telegram account that claims the
/// code. The names are personal data, so `Debug` leaves them out (ADR 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct LinkTarget {
    /// The display name of the user who asked for the code.
    pub user_name: String,
    /// The name of the organization of the code.
    pub organization_name: String,
    /// The address of the user. The bot shows only its masked form (`sign_in::email_hint`).
    pub email: Email,
}

impl Debug for LinkTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LinkTarget(redacted)")
    }
}

/// The Telegram account that is linked to a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelegramLink {
    pub telegram_user_id: TelegramUserId,
    pub linked_at: Timestamp,
}

/// A claimed code that the Telegram account accepted, and that waits for the confirmation of the member.
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

    /// Marks an unexpired, unclaimed code as claimed by the account and returns the tada account of
    /// the code. Returns `None` for any other code.
    /// Infrastructure query (ADR 0039): the code finds its organization.
    async fn claim(
        &self,
        code: &str,
        account: TelegramUserId,
        name: &TelegramName,
        now: Timestamp,
    ) -> Result<Option<LinkTarget>, StoreError>;

    /// Marks the newest claimed, unexpired code of the account as accepted, if it is not accepted
    /// yet. Returns false if the account has no such code.
    /// Infrastructure query (ADR 0039): a Telegram account has no organization before it names a user.
    async fn accept(&self, account: TelegramUserId, now: Timestamp) -> Result<bool, StoreError>;

    /// The claimed, accepted, unconfirmed and unexpired codes of the user.
    async fn requests(
        &self,
        scope: OrgScope,
        user_id: UserId,
        now: Timestamp,
    ) -> Result<Vec<LinkRequest>, StoreError>;

    /// Binds the account of an accepted request to the user, in one transaction with the confirmation.
    async fn confirm(
        &self,
        scope: OrgScope,
        user_id: UserId,
        request_id: Uuid,
        now: Timestamp,
    ) -> Result<Confirmed, StoreError>;

    /// The user that the Telegram account is linked to.
    /// Infrastructure query (ADR 0039): a Telegram account has no organization before it names a user.
    async fn user_of(&self, account: TelegramUserId) -> Result<Option<UserId>, StoreError>;

    /// The Telegram account that is linked to the user.
    /// Infrastructure query (ADR 0039): a link belongs to a user, not to an organization.
    async fn link_of(&self, user_id: UserId) -> Result<Option<TelegramLink>, StoreError>;

    /// Removes the link of the user, if any.
    /// Infrastructure query (ADR 0039): a link belongs to a user, not to an organization.
    async fn unlink_user(&self, user_id: UserId) -> Result<(), StoreError>;

    /// Removes the link of the account, if any, and its claims that no member confirmed.
    /// Infrastructure query (ADR 0039): a Telegram account has no organization before it names a user.
    async fn unlink_account(&self, account: TelegramUserId) -> Result<(), StoreError>;

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
    /// The session of the caller started too long ago to link an account (`session::RECENT_SIGN_IN`).
    #[error("the action needs a recent sign-in")]
    RecentSignInRequired,
    #[error("no open link request with this ID")]
    NotFound,
    #[error("the Telegram account or the user has another link")]
    AlreadyLinked,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl LinkError {
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::RecentSignInRequired,
        ProblemCode::NotFound,
        ProblemCode::InvalidTransition,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for LinkError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::RecentSignInRequired => ProblemCode::RecentSignInRequired,
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

/// Step 2: the gateway claims a code for the Telegram account that sent it. The result names the
/// tada account of the code, so that the Telegram account sees whom it would link to.
pub async fn claim_link_code(
    _caller: &ServiceCaller<TelegramGateway>,
    code: &str,
    account: TelegramUserId,
    name: &TelegramName,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<Option<LinkTarget>, StoreError> {
    links.claim(code.trim(), account, name, clock.now()).await
}

/// Step 2, continued: the Telegram account accepts its newest claim, after the bot named the tada
/// account. Without it, the member cannot confirm, so a member who sends a victim the own code
/// cannot link the Telegram account of the victim. Returns false if nothing waits for acceptance.
pub async fn accept_link_claim(
    _caller: &ServiceCaller<TelegramGateway>,
    account: TelegramUserId,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<bool, StoreError> {
    links.accept(account, clock.now()).await
}

/// The Telegram account ends its link and rejects its open claims, for example a claim of a code
/// that names a stranger.
pub async fn unlink_account(
    _caller: &ServiceCaller<TelegramGateway>,
    account: TelegramUserId,
    links: &dyn TelegramLinks,
) -> Result<(), StoreError> {
    links.unlink_account(account).await
}

/// The Telegram account that is linked to the calling member, if any.
pub async fn get_link(
    caller: &MemberCaller,
    links: &dyn TelegramLinks,
) -> Result<Option<TelegramLink>, StoreError> {
    links.link_of(caller.user_id()).await
}

/// The calling member ends the link, for example after the loss of the phone. A removal needs no
/// recent sign-in, because it only takes access away.
pub async fn unlink(caller: &MemberCaller, links: &dyn TelegramLinks) -> Result<(), StoreError> {
    links.unlink_user(caller.user_id()).await
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
/// A link lets the account act as the member without a session, so the confirmation needs a
/// session younger than `session::RECENT_SIGN_IN`, as an API token does.
pub async fn confirm_link(
    caller: &MemberCaller,
    request_id: Uuid,
    links: &dyn TelegramLinks,
    clock: &dyn Clock,
) -> Result<TelegramUserId, LinkError> {
    let now = clock.now();
    if !caller.signed_in_recently(now) {
        return Err(LinkError::RecentSignInRequired);
    }
    match links
        .confirm(caller.scope(), caller.user_id(), request_id, now)
        .await?
    {
        Confirmed::Linked(account) => Ok(account),
        Confirmed::NotFound => Err(LinkError::NotFound),
        Confirmed::AlreadyLinked => Err(LinkError::AlreadyLinked),
    }
}

/// The event of a Telegram command, with the linked member who acts in it.
#[derive(Debug)]
pub struct MemberEvent {
    pub caller: MemberCaller,
    pub event: Event,
}

#[derive(Debug, thiserror::Error)]
pub enum TelegramActError {
    #[error("the Telegram account is not linked to a user")]
    NotLinked,
    /// No event with this key, or the member has no event role in it.
    #[error("the event does not exist or the member cannot see it")]
    NotFound,
    /// The member has events with this key in more than one organization.
    #[error("more than one event of the member has this key")]
    Ambiguous,
    #[error("the event has no open field with this key")]
    UnknownField,
    #[error("the value does not fit the field")]
    Value(#[from] ValueError),
    #[error(transparent)]
    Propose(#[from] ProposeError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<AccessError> for TelegramActError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

impl CommandError for TelegramActError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotLinked | Self::NotFound => ProblemCode::NotFound,
            Self::Ambiguous | Self::UnknownField | Self::Value(_) => ProblemCode::ValidationFailed,
            Self::Propose(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            Self::Propose(error) => error.store_error(),
            _ => None,
        }
    }
}

/// The member for whom the gateway acts in the event `event_key` (ADR 0038, ADR 0039):
/// the event of the linked user among the events with this key in the organizations of the user.
/// The user needs an event role in the event, or an owner or admin role in its organization.
/// It reads the memberships at the time of the call, so a removed member finds nothing.
pub async fn member_for(
    gateway: &ServiceCaller<TelegramGateway>,
    account: TelegramUserId,
    event_key: &str,
    links: &dyn TelegramLinks,
    identity: &dyn IdentityStore,
    events: &dyn EventStore,
) -> Result<MemberEvent, TelegramActError> {
    let user = links
        .user_of(account)
        .await?
        .ok_or(TelegramActError::NotLinked)?;
    let key = EventKey::parse(&event_key.to_uppercase()).map_err(|_| TelegramActError::NotFound)?;
    let mut found = Vec::new();
    for membership in identity.memberships_of(user).await? {
        let caller = gateway.member(user, membership.organization_id, membership.role);
        let Some(event) = events.find_by_key(caller.scope(), &key).await? else {
            continue;
        };
        match access::event_access(&caller, event.id, identity).await {
            Ok(_) => found.push(MemberEvent { caller, event }),
            Err(AccessError::NotFound) => {}
            Err(AccessError::Store(error)) => return Err(error.into()),
        }
    }
    match (found.pop(), found.is_empty()) {
        (Some(only), true) => Ok(only),
        (Some(_), false) => Err(TelegramActError::Ambiguous),
        (None, _) => Err(TelegramActError::NotFound),
    }
}

/// A message that proposes a new value for a field.
#[derive(Debug)]
pub struct FactMessage<'a> {
    /// The whole message. The changeset keeps it as its source text.
    pub source: &'a SourceText,
    /// The key of the field, for example `date_window`.
    pub field_key: &'a str,
    /// The value part of the message, in characters of `source`. It is the passage of the evidence.
    pub value: Range<usize>,
}

/// Proposes the value of one field of the event: one changeset with one `SetFact` proposal,
/// against the current version of the fact. `reason` is the short text for the reviewer. An event manager reviews it in the web client (ADR 0050).
pub async fn propose_fact(
    caller: &MemberCaller,
    event: &Event,
    message: FactMessage<'_>,
    reason: &str,
    stores: ProposeStores<'_>,
    clock: &dyn Clock,
) -> Result<ChangesetId, TelegramActError> {
    // Fail fast: a viewer gets the refusal before any hint about the field or the value.
    if !access::event_access(caller, event.id, stores.identity)
        .await?
        .can_propose()
    {
        return Err(ProposeError::Forbidden.into());
    }
    let key = FieldKey::parse(message.field_key).map_err(|_| TelegramActError::UnknownField)?;
    let scope = caller.scope();
    let field = stores
        .facts
        .catalog(scope, event.id)
        .await?
        .into_iter()
        .find(|field| field.key == key && field.status == FieldStatus::Active)
        .ok_or(TelegramActError::UnknownField)?;
    let quote: String = message
        .source
        .as_str()
        .chars()
        .skip(message.value.start)
        .take(message.value.len())
        .collect();
    let valued = FactValue::parse_text(&quote, &field.value_type)?;
    let expected_version = stores
        .facts
        .current_version(scope, event.id, field.id)
        .await?
        .map(|current| current.number.get());
    let offset = |offset: usize| {
        u32::try_from(offset)
            .map_err(|_| ProposeError::Invalid(vec![FieldError::new("source_text", "length")]))
    };
    let input = NewChangeset {
        id: None,
        event_id: Some(event.id.as_uuid()),
        source_text: message.source.as_str().to_owned(),
        proposals: vec![NewProposal {
            id: Uuid::now_v7(),
            operation: OperationInput::SetFact {
                event_id: event.id.as_uuid(),
                field_id: field.id.as_uuid(),
                state: FactStateInput::Accepted {
                    value: ValueInput::from(&valued.value),
                    approximate: valued.approximate,
                },
                expected_version,
            },
            depends_on: Vec::new(),
            evidence: vec![PassageInput {
                source_version_id: None,
                start: offset(message.value.start)?,
                end: offset(message.value.end)?,
                quote,
                page: None,
            }],
            reason: reason.to_owned(),
        }],
    };
    let (Created::New(changeset) | Created::Existing(changeset)) =
        create_changeset(caller, input, stores, clock).await?;
    Ok(changeset.id)
}
