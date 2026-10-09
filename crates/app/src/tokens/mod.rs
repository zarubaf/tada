//! Personal API tokens (ADR 0039, ADR 0045): a member gives an external client, for example an MCP client, access in one organization.
//!
//! A token caller is an `AiCaller`, because an AI can be behind any token.
//! It can read and, with the scope `propose`, propose. It never applies a change.

#[cfg(test)]
mod tests;

use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use tada_domain::RecordVersion;
use tada_domain::identity::TokenName;
use tada_domain::ids::{ApiTokenId, OrganizationId, UserId};
use uuid::Uuid;

use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::auth::{Authenticated, AuthenticationError, Authenticator, Credential};
use crate::caller::OrganizationRole;
use crate::caller::{AiCaller, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::session::TOUCH_INTERVAL;
use crate::store::StoreError;

/// The start of each token, so that secret scanners find it (ADR 0039).
pub const TOKEN_PREFIX: &str = "tada_pat_";
/// The version of the token notice of ADR 0045. A changed notice text gets a new version.
pub const NOTICE_VERSION: u32 = 1;
/// Each version of the token notice with the SHA-256 of its text (the `token-notice-*` messages of
/// `locales/de-CH/web.ftl`), oldest first. A changed text adds a line with the next version and changes
/// `NOTICE_VERSION`. Never change a line. A test fails when the text and the newest line disagree.
#[cfg(test)]
const NOTICE_TEXTS: &[(u32, &str)] = &[(
    1,
    "35f604e0d2ce1c7e5c0ce5c4404605d7f2b2961b5a7a41479697457128aa09d6",
)];
/// The longest time from the creation of a token to its expiry.
pub const MAX_LIFETIME: SignedDuration = SignedDuration::from_hours(365 * 24);

/// What a token allows (ADR 0039). There is no `write` scope: an AI behind a token never changes accepted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenScope {
    /// Queries only.
    Read,
    /// Queries and proposals.
    Propose,
}

impl TokenScope {
    /// The name that the API and the database use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Propose => "propose",
        }
    }

    /// The scope of a name of `as_str`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Read, Self::Propose]
            .into_iter()
            .find(|scope| scope.as_str() == name)
    }
}

/// A switch of one organization (ADR 0036). An owner changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// Personal API tokens for MCP clients (ADR 0045). It is on until an owner switches it off.
    McpTokens,
}

impl Feature {
    pub const ALL: [Self; 1] = [Self::McpTokens];

    /// The name that the API and the database use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::McpTokens => "mcp-tokens",
        }
    }

    /// The feature of a name of `as_str`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.as_str() == name)
    }

    /// The value of a feature that no owner changed.
    fn default_enabled(self) -> bool {
        match self {
            Self::McpTokens => true,
        }
    }
}

/// The value of a feature in one organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureState {
    pub feature: Feature,
    pub enabled: bool,
    /// A feature that no owner changed has version 1.
    pub version: RecordVersion,
}

/// A token as its member sees it. It never holds the secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiToken {
    pub id: ApiTokenId,
    pub name: TokenName,
    pub scope: TokenScope,
    pub expires_at: Timestamp,
    /// The version of the token notice that the member confirmed at `created_at` (ADR 0045).
    pub notice_version: u32,
    pub created_at: Timestamp,
    pub last_used_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
}

/// A stored token with its member, as the lookup by secret finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredToken {
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub token: ApiToken,
}

/// The input of `create_token`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRequest {
    pub name: String,
    pub scope: TokenScope,
    pub expires_at: Timestamp,
    /// The version of the token notice that the member confirmed. It must be `NOTICE_VERSION`.
    pub notice_version_confirmed: u32,
}

/// A new token with its secret. The member sees the secret once; no table holds it.
#[derive(Debug)]
pub struct CreatedToken {
    pub token: ApiToken,
    pub secret: SecretString,
}

/// The repository port of API tokens and organization features.
#[async_trait]
pub trait TokenStore: Debug + Send + Sync {
    /// Stores `token` of the member `user` with a new secret that starts with `TOKEN_PREFIX`,
    /// and records `audit`, in one transaction. Returns the secret; the store keeps only its hash.
    async fn insert(
        &self,
        scope: OrgScope,
        user: UserId,
        token: &ApiToken,
        audit: &AuditEvent,
    ) -> Result<SecretString, StoreError>;

    /// The tokens of the member `user`, the newest first.
    async fn list(&self, scope: OrgScope, user: UserId) -> Result<Vec<ApiToken>, StoreError>;

    /// Sets the revocation time of the token `id` of the member `user` to `now` and records `audit`,
    /// if the token is not revoked. Returns false if the member has no token with this ID.
    async fn revoke(
        &self,
        scope: OrgScope,
        user: UserId,
        id: ApiTokenId,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<bool, StoreError>;

    /// The token of `secret`, found by the hash of the secret, also if it is revoked or expired.
    /// Infrastructure query (ADR 0039): the secret names the token, and no caller exists before the lookup.
    async fn find(&self, secret: &str) -> Result<Option<StoredToken>, StoreError>;

    /// Sets the last-use time of the token `id` to `now`.
    async fn touch(
        &self,
        scope: OrgScope,
        id: ApiTokenId,
        now: Timestamp,
    ) -> Result<(), StoreError>;

    /// The features that an owner changed. A missing feature has its default value and version 1.
    async fn features(&self, scope: OrgScope) -> Result<Vec<FeatureState>, StoreError>;

    /// Sets `feature` to `enabled` and records `audit` in one transaction, if the feature has the
    /// version `expected_version`. Returns `None` on a version conflict.
    async fn set_feature(
        &self,
        scope: OrgScope,
        feature: Feature,
        enabled: bool,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<FeatureState>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    /// The caller cannot do this: for example, a member without the right to propose asks for a
    /// `propose` token (ADR 0052), or a member who is not an owner changes a feature.
    #[error("the caller cannot do this")]
    Forbidden,
    /// The organization switched off MCP tokens (ADR 0045).
    #[error("the organization switched off API tokens")]
    Disabled,
    /// The member has no token with this ID.
    #[error("the token does not exist")]
    NotFound,
    #[error("the feature changed after the caller read it")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl TokenError {
    /// All codes of the token commands and queries, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for TokenError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden | Self::Disabled => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The value of each feature in the organization of `scope`.
async fn features_of(
    scope: OrgScope,
    tokens: &dyn TokenStore,
) -> Result<Vec<FeatureState>, StoreError> {
    let changed = tokens.features(scope).await?;
    Ok(Feature::ALL
        .into_iter()
        .map(|feature| {
            changed
                .iter()
                .find(|state| state.feature == feature)
                .copied()
                .unwrap_or(FeatureState {
                    feature,
                    enabled: feature.default_enabled(),
                    version: RecordVersion::FIRST,
                })
        })
        .collect())
}

/// True if MCP tokens are on in the organization of `scope`.
async fn tokens_enabled(scope: OrgScope, tokens: &dyn TokenStore) -> Result<bool, StoreError> {
    Ok(features_of(scope, tokens)
        .await?
        .iter()
        .any(|state| state.feature == Feature::McpTokens && state.enabled))
}

/// The checked values of a request, or the errors.
fn check(request: &TokenRequest, now: Timestamp) -> Result<TokenName, TokenError> {
    let mut errors = Vec::new();
    if request.notice_version_confirmed != NOTICE_VERSION {
        errors.push(FieldError::new("notice_version_confirmed", "not-confirmed"));
    }
    let name = TokenName::parse(&request.name)
        .map_err(|error| {
            errors.push(FieldError::new(
                "name",
                crate::events::name_error_code(error),
            ))
        })
        .ok();
    if request.expires_at <= now {
        errors.push(FieldError::new("expires_at", "not-in-future"));
    } else if request.expires_at > now + MAX_LIFETIME {
        errors.push(FieldError::new("expires_at", "too-late"));
    }
    match name {
        Some(name) if errors.is_empty() => Ok(name),
        _ => Err(TokenError::Invalid(errors)),
    }
}

/// Creates a personal API token of the caller in the caller's organization (ADR 0039).
///
/// The member must confirm the current token notice (ADR 0045), and the organization must not
/// switch off MCP tokens. A `propose` token needs the right to propose in at least one event (ADR 0052).
/// The result holds the secret; tada shows it once.
pub async fn create_token(
    caller: &MemberCaller,
    request: TokenRequest,
    tokens: &dyn TokenStore,
    identity: &dyn IdentityStore,
    clock: &dyn Clock,
) -> Result<CreatedToken, TokenError> {
    let now = clock.now();
    let name = check(&request, now)?;
    let scope = caller.scope();
    if !tokens_enabled(scope, tokens).await? {
        return Err(TokenError::Disabled);
    }
    if request.scope == TokenScope::Propose
        && !access::proposes_in_some_event(caller, identity).await?
    {
        return Err(TokenError::Forbidden);
    }
    let token = ApiToken {
        id: ApiTokenId::from_uuid(Uuid::now_v7()),
        name,
        scope: request.scope,
        expires_at: request.expires_at,
        notice_version: NOTICE_VERSION,
        created_at: now,
        last_used_at: None,
        revoked_at: None,
    };
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::ApiTokenCreate,
        Some(token.id.as_uuid()),
        Some(scope),
    );
    let secret = tokens
        .insert(scope, caller.user_id(), &token, &audit)
        .await?;
    Ok(CreatedToken { token, secret })
}

/// The tokens of the caller in the caller's organization, the newest first, also the revoked and expired ones.
pub async fn list_tokens(
    caller: &MemberCaller,
    tokens: &dyn TokenStore,
) -> Result<Vec<ApiToken>, TokenError> {
    Ok(tokens.list(caller.scope(), caller.user_id()).await?)
}

/// Revokes a token of the caller. The token stops working with the next request.
/// A revoked token stays revoked; a second revocation changes nothing.
pub async fn revoke_token(
    caller: &MemberCaller,
    id: ApiTokenId,
    tokens: &dyn TokenStore,
    clock: &dyn Clock,
) -> Result<(), TokenError> {
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::ApiTokenRevoke,
        Some(id.as_uuid()),
        Some(caller.scope()),
    );
    if tokens
        .revoke(caller.scope(), caller.user_id(), id, clock.now(), &audit)
        .await?
    {
        Ok(())
    } else {
        Err(TokenError::NotFound)
    }
}

/// The value of each feature of the caller's organization. Each member can read them.
pub async fn get_features(
    caller: &MemberCaller,
    tokens: &dyn TokenStore,
) -> Result<Vec<FeatureState>, TokenError> {
    Ok(features_of(caller.scope(), tokens).await?)
}

/// Switches a feature of the caller's organization on or off (ADR 0036). Only an owner can do this.
pub async fn set_feature(
    caller: &MemberCaller,
    feature: Feature,
    enabled: bool,
    expected_version: RecordVersion,
    tokens: &dyn TokenStore,
) -> Result<FeatureState, TokenError> {
    if caller.organization_role() != OrganizationRole::Owner {
        return Err(TokenError::Forbidden);
    }
    let scope = caller.scope();
    let action = if enabled {
        AuditAction::OrganizationFeatureEnable
    } else {
        AuditAction::OrganizationFeatureDisable
    };
    // A feature has no ID of its own: the record ID is its organization.
    let audit = AuditEvent::new(
        caller.actor(),
        action,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    );
    tokens
        .set_feature(scope, feature, enabled, expected_version, &audit)
        .await?
        .ok_or(TokenError::VersionConflict)
}

/// The `Authenticator` of personal API tokens (ADR 0039). It lives in `app` because it only
/// composes ports and needs the private caller constructors (ADR 0062).
pub struct TokenAuthenticator {
    tokens: Arc<dyn TokenStore>,
    identity: Arc<dyn IdentityStore>,
    clock: Arc<dyn Clock>,
}

impl TokenAuthenticator {
    pub fn new(
        tokens: Arc<dyn TokenStore>,
        identity: Arc<dyn IdentityStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            tokens,
            identity,
            clock,
        }
    }
}

impl Debug for TokenAuthenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenAuthenticator").finish_non_exhaustive()
    }
}

#[async_trait]
impl Authenticator for TokenAuthenticator {
    /// Returns the AI caller of a valid token. Each other case is `Unauthenticated`: an unknown,
    /// revoked or expired token, a removed member, or an organization without MCP tokens.
    async fn authenticate(
        &self,
        credential: Option<Credential<'_>>,
    ) -> Result<Authenticated, AuthenticationError> {
        let Some(Credential::ApiToken(secret)) = credential else {
            return Err(AuthenticationError::Unauthenticated);
        };
        let Some(stored) = self.tokens.find(secret).await? else {
            return Err(AuthenticationError::Unauthenticated);
        };
        let token = &stored.token;
        let now = self.clock.now();
        if token.revoked_at.is_some() || now >= token.expires_at {
            return Err(AuthenticationError::Unauthenticated);
        }
        // The membership and the switch can change between two requests, so each request reads them.
        let scope = OrgScope::for_credential(stored.organization_id);
        let Some(role) = self.identity.membership(scope, stored.user_id).await? else {
            return Err(AuthenticationError::Unauthenticated);
        };
        if !tokens_enabled(scope, &*self.tokens).await? {
            return Err(AuthenticationError::Unauthenticated);
        }
        if token
            .last_used_at
            .is_none_or(|last| now.duration_since(last) >= TOUCH_INTERVAL)
        {
            self.tokens.touch(scope, token.id, now).await?;
        }
        Ok(Authenticated::Ai(AiCaller::create(
            stored.user_id,
            stored.organization_id,
            role,
            token.id,
            token.scope,
        )))
    }
}
