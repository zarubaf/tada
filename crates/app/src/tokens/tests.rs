use std::collections::HashMap;
use std::sync::Mutex;

use secrecy::ExposeSecret;
use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{EventId, OrganizationId};

use super::*;
use crate::access::Principal;
use crate::identity::{Membership, UserRef};

use OrganizationRole::{Admin, Member, Owner};

const START: Timestamp = Timestamp::constant(1_900_000_000, 0);
const DAY: SignedDuration = SignedDuration::from_hours(24);
const SECOND: SignedDuration = SignedDuration::from_secs(1);

fn testwil() -> OrganizationId {
    OrganizationId::from_uuid(Uuid::from_u128(10))
}

fn anna() -> UserId {
    UserId::from_uuid(Uuid::from_u128(1))
}

/// A clock that the test moves.
#[derive(Debug)]
struct TestClock(Mutex<Timestamp>);

impl TestClock {
    fn new() -> Self {
        Self(Mutex::new(START))
    }

    fn set(&self, offset: SignedDuration) {
        *self.0.lock().unwrap() = START + offset;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

/// Anna's membership in Testwil, her event roles, the tokens and the switches of Testwil.
#[derive(Debug, Default)]
struct Memory {
    role: Mutex<Option<OrganizationRole>>,
    event_roles: Mutex<Vec<EventRole>>,
    /// The tokens by their secret, with the hash a store would keep.
    tokens: Mutex<HashMap<String, StoredToken>>,
    features: Mutex<Vec<FeatureState>>,
    audit: Mutex<Vec<AuditEvent>>,
    touches: Mutex<usize>,
}

impl Memory {
    fn with_role(role: OrganizationRole) -> Self {
        let memory = Self::default();
        *memory.role.lock().unwrap() = Some(role);
        memory
    }

    fn token(&self, secret: &str) -> StoredToken {
        self.tokens.lock().unwrap()[secret].clone()
    }
}

#[async_trait]
impl IdentityStore for Memory {
    async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
        unreachable!()
    }

    async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
        unreachable!()
    }

    async fn membership(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError> {
        let found = scope.organization_id() == testwil() && user == anna();
        Ok(found.then(|| *self.role.lock().unwrap()).flatten())
    }

    async fn event_exists(&self, _: OrgScope, _: EventId) -> Result<bool, StoreError> {
        unreachable!()
    }

    async fn event_role(
        &self,
        _: OrgScope,
        _: EventId,
        _: UserId,
    ) -> Result<Option<EventRole>, StoreError> {
        unreachable!()
    }

    async fn event_roles_of(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
        let found = scope.organization_id() == testwil() && user == anna();
        Ok(if found {
            let event = EventId::from_uuid(Uuid::from_u128(20));
            let roles = self.event_roles.lock().unwrap();
            roles.iter().map(|role| (event, *role)).collect()
        } else {
            Vec::new()
        })
    }
}

#[async_trait]
impl TokenStore for Memory {
    async fn insert(
        &self,
        scope: OrgScope,
        user: UserId,
        token: &ApiToken,
        audit: &AuditEvent,
    ) -> Result<SecretString, StoreError> {
        let secret = format!("{TOKEN_PREFIX}{}", Uuid::now_v7().simple());
        self.tokens.lock().unwrap().insert(
            secret.clone(),
            StoredToken {
                organization_id: scope.organization_id(),
                user_id: user,
                token: token.clone(),
            },
        );
        self.audit.lock().unwrap().push(audit.clone());
        Ok(SecretString::from(secret))
    }

    async fn list(&self, scope: OrgScope, user: UserId) -> Result<Vec<ApiToken>, StoreError> {
        let mut tokens: Vec<ApiToken> = self
            .tokens
            .lock()
            .unwrap()
            .values()
            .filter(|stored| {
                stored.organization_id == scope.organization_id() && stored.user_id == user
            })
            .map(|stored| stored.token.clone())
            .collect();
        tokens.sort_by_key(|token| std::cmp::Reverse(token.id));
        Ok(tokens)
    }

    async fn revoke(
        &self,
        scope: OrgScope,
        user: UserId,
        id: ApiTokenId,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<bool, StoreError> {
        let mut tokens = self.tokens.lock().unwrap();
        let Some(stored) = tokens.values_mut().find(|stored| {
            stored.organization_id == scope.organization_id()
                && stored.user_id == user
                && stored.token.id == id
        }) else {
            return Ok(false);
        };
        if stored.token.revoked_at.is_none() {
            stored.token.revoked_at = Some(now);
            self.audit.lock().unwrap().push(audit.clone());
        }
        Ok(true)
    }

    async fn find(&self, secret: &str) -> Result<Option<StoredToken>, StoreError> {
        Ok(self.tokens.lock().unwrap().get(secret).cloned())
    }

    async fn touch(
        &self,
        scope: OrgScope,
        id: ApiTokenId,
        now: Timestamp,
    ) -> Result<(), StoreError> {
        *self.touches.lock().unwrap() += 1;
        for stored in self.tokens.lock().unwrap().values_mut() {
            if stored.organization_id == scope.organization_id() && stored.token.id == id {
                stored.token.last_used_at = Some(now);
            }
        }
        Ok(())
    }

    async fn features(&self, scope: OrgScope) -> Result<Vec<FeatureState>, StoreError> {
        assert_eq!(scope.organization_id(), testwil());
        Ok(self.features.lock().unwrap().clone())
    }

    async fn set_feature(
        &self,
        scope: OrgScope,
        feature: Feature,
        enabled: bool,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<FeatureState>, StoreError> {
        assert_eq!(scope.organization_id(), testwil());
        let mut features = self.features.lock().unwrap();
        let current = features
            .iter()
            .find(|state| state.feature == feature)
            .map_or(RecordVersion::FIRST, |state| state.version);
        if current != expected_version {
            return Ok(None);
        }
        let state = FeatureState {
            feature,
            enabled,
            version: RecordVersion::new(current.get() + 1).unwrap(),
        };
        features.retain(|other| other.feature != feature);
        features.push(state);
        self.audit.lock().unwrap().push(audit.clone());
        Ok(Some(state))
    }
}

fn anna_as(role: OrganizationRole) -> MemberCaller {
    MemberCaller::new(anna(), testwil(), role)
}

fn request(scope: TokenScope) -> TokenRequest {
    TokenRequest {
        name: "  Claude Desktop ".to_owned(),
        scope,
        expires_at: START + DAY * 30,
        notice_version_confirmed: NOTICE_VERSION,
    }
}

async fn create(
    memory: &Memory,
    role: OrganizationRole,
    request: TokenRequest,
) -> Result<CreatedToken, TokenError> {
    create_token(&anna_as(role), request, memory, memory, &TestClock::new()).await
}

fn invalid_fields(result: Result<CreatedToken, TokenError>) -> Vec<(String, &'static str)> {
    let Err(TokenError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    errors
        .into_iter()
        .map(|error| (error.field.into_owned(), error.code))
        .collect()
}

#[tokio::test]
async fn a_member_creates_a_read_token_with_an_audit_event() {
    let memory = Memory::with_role(Member);
    let created = create(&memory, Member, request(TokenScope::Read))
        .await
        .unwrap();

    assert!(created.secret.expose_secret().starts_with("tada_pat_"));
    assert_eq!(created.token.name.as_str(), "Claude Desktop");
    assert_eq!(created.token.scope, TokenScope::Read);
    assert_eq!(created.token.expires_at, START + DAY * 30);
    assert_eq!(created.token.notice_version, NOTICE_VERSION);
    assert_eq!(created.token.created_at, START);
    assert_eq!(created.token.last_used_at, None);
    assert_eq!(created.token.revoked_at, None);
    assert!(tada_domain::ids::is_record_id(created.token.id.as_uuid()));
    let stored = memory.token(created.secret.expose_secret());
    assert_eq!(
        (stored.organization_id, stored.user_id),
        (testwil(), anna())
    );

    let audit = memory.audit.lock().unwrap();
    assert_eq!(audit[0].action(), AuditAction::ApiTokenCreate);
    assert_eq!(audit[0].record_kind(), "api_token");
    assert_eq!(audit[0].record_id(), Some(created.token.id.as_uuid()));
    assert_eq!(audit[0].organization_id(), Some(testwil()));
    assert_eq!(audit[0].actor(), &anna_as(Member).actor());
}

#[tokio::test]
async fn creation_without_the_notice_confirmation_is_invalid() {
    let memory = Memory::with_role(Owner);
    for confirmed in [0, NOTICE_VERSION + 1] {
        let request = TokenRequest {
            notice_version_confirmed: confirmed,
            ..request(TokenScope::Read)
        };
        let result = create(&memory, Owner, request).await;
        assert_eq!(
            result.as_ref().map_err(TokenError::code).err(),
            Some(ProblemCode::ValidationFailed)
        );
        assert_eq!(
            invalid_fields(result),
            [("notice_version_confirmed".to_owned(), "not-confirmed")]
        );
    }
    // The notice error comes with the other field errors, so a form needs one round trip.
    let request = TokenRequest {
        name: " ".to_owned(),
        notice_version_confirmed: 0,
        ..request(TokenScope::Read)
    };
    assert_eq!(
        invalid_fields(create(&memory, Owner, request).await),
        [
            ("notice_version_confirmed".to_owned(), "not-confirmed"),
            ("name".to_owned(), "empty"),
        ]
    );
    assert!(memory.tokens.lock().unwrap().is_empty());
}

#[tokio::test]
async fn rejects_an_invalid_name_and_expiry() {
    let memory = Memory::with_role(Owner);
    for (name, expires_at, field, code) in [
        (" ", START + DAY, "name", "empty"),
        ("Claude", START, "expires_at", "not-in-future"),
        (
            "Claude",
            START + MAX_LIFETIME + SECOND,
            "expires_at",
            "too-late",
        ),
    ] {
        let request = TokenRequest {
            name: name.to_owned(),
            expires_at,
            ..request(TokenScope::Read)
        };
        let result = create(&memory, Owner, request).await;
        assert_eq!(invalid_fields(result), [(field.to_owned(), code)]);
    }
    let longest = TokenRequest {
        expires_at: START + MAX_LIFETIME,
        ..request(TokenScope::Read)
    };
    assert!(create(&memory, Owner, longest).await.is_ok());
}

#[tokio::test]
async fn a_pure_event_viewer_gets_a_read_token_but_no_propose_token() {
    let memory = Memory::with_role(Member);
    memory
        .event_roles
        .lock()
        .unwrap()
        .push(EventRole::EventViewer);

    let result = create(&memory, Member, request(TokenScope::Propose)).await;
    assert!(matches!(result, Err(TokenError::Forbidden)), "{result:?}");
    assert_eq!(TokenError::Forbidden.code(), ProblemCode::Forbidden);
    assert!(
        create(&memory, Member, request(TokenScope::Read))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn contributors_managers_owners_and_admins_get_a_propose_token() {
    for event_role in [EventRole::EventContributor, EventRole::EventManager] {
        let memory = Memory::with_role(Member);
        memory
            .event_roles
            .lock()
            .unwrap()
            .extend([EventRole::EventViewer, event_role]);
        assert!(
            create(&memory, Member, request(TokenScope::Propose))
                .await
                .is_ok()
        );
    }
    for role in [Owner, Admin] {
        let memory = Memory::with_role(role);
        assert!(
            create(&memory, role, request(TokenScope::Propose))
                .await
                .is_ok()
        );
    }
    let memory = Memory::with_role(Member);
    let result = create(&memory, Member, request(TokenScope::Propose)).await;
    assert!(
        matches!(result, Err(TokenError::Forbidden)),
        "no event role: {result:?}"
    );
}

#[tokio::test]
async fn no_token_while_the_organization_switches_mcp_tokens_off() {
    let memory = Memory::with_role(Owner);
    let owner = anna_as(Owner);
    set_feature(
        &owner,
        Feature::McpTokens,
        false,
        RecordVersion::FIRST,
        &memory,
    )
    .await
    .unwrap();

    let result = create(&memory, Owner, request(TokenScope::Read)).await;
    assert!(matches!(result, Err(TokenError::Disabled)), "{result:?}");
    assert_eq!(TokenError::Disabled.code(), ProblemCode::Forbidden);
}

#[tokio::test]
async fn only_an_owner_switches_a_feature_with_the_expected_version() {
    let memory = Memory::with_role(Owner);
    let features = get_features(&anna_as(Member), &memory).await.unwrap();
    assert_eq!(
        features,
        [FeatureState {
            feature: Feature::McpTokens,
            enabled: true,
            version: RecordVersion::FIRST,
        }],
        "a feature without a row is on"
    );

    for role in [Admin, Member] {
        let result = set_feature(
            &anna_as(role),
            Feature::McpTokens,
            false,
            RecordVersion::FIRST,
            &memory,
        )
        .await;
        assert!(matches!(result, Err(TokenError::Forbidden)), "{role:?}");
    }
    let owner = anna_as(Owner);
    let off = set_feature(
        &owner,
        Feature::McpTokens,
        false,
        RecordVersion::FIRST,
        &memory,
    )
    .await
    .unwrap();
    assert!(!off.enabled);
    assert_eq!(off.version.get(), 2);
    let stale = set_feature(
        &owner,
        Feature::McpTokens,
        true,
        RecordVersion::FIRST,
        &memory,
    )
    .await;
    assert!(
        matches!(stale, Err(TokenError::VersionConflict)),
        "{stale:?}"
    );
    assert_eq!(
        TokenError::VersionConflict.code(),
        ProblemCode::RecordVersionConflict
    );
    assert_eq!(get_features(&owner, &memory).await.unwrap(), [off]);

    let audit = memory.audit.lock().unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action(), AuditAction::OrganizationFeatureDisable);
    assert_eq!(audit[0].record_kind(), "organization_feature");
    assert_eq!(audit[0].record_id(), Some(testwil().as_uuid()));
}

#[tokio::test]
async fn a_member_lists_and_revokes_the_own_tokens() {
    let memory = Memory::with_role(Member);
    let clock = TestClock::new();
    let first = create(&memory, Member, request(TokenScope::Read))
        .await
        .unwrap();
    let second = create(&memory, Member, request(TokenScope::Read))
        .await
        .unwrap();

    let member = anna_as(Member);
    let listed = list_tokens(&member, &memory).await.unwrap();
    assert_eq!(listed, [second.token.clone(), first.token.clone()]);

    clock.set(DAY);
    revoke_token(&member, first.token.id, &memory, &clock)
        .await
        .unwrap();
    let listed = list_tokens(&member, &memory).await.unwrap();
    assert_eq!(listed[1].revoked_at, Some(START + DAY));
    // A second revocation changes nothing.
    revoke_token(&member, first.token.id, &memory, &clock)
        .await
        .unwrap();

    let unknown = ApiTokenId::from_uuid(Uuid::now_v7());
    let result = revoke_token(&member, unknown, &memory, &clock).await;
    assert!(matches!(result, Err(TokenError::NotFound)), "{result:?}");

    let audit = memory.audit.lock().unwrap();
    let actions: Vec<_> = audit.iter().map(AuditEvent::action).collect();
    assert_eq!(
        actions,
        [
            AuditAction::ApiTokenCreate,
            AuditAction::ApiTokenCreate,
            AuditAction::ApiTokenRevoke
        ]
    );
}

struct Fixture {
    memory: Arc<Memory>,
    clock: Arc<TestClock>,
    authenticator: TokenAuthenticator,
}

impl Fixture {
    /// Anna is a member of Testwil and has a token with the scope `scope`.
    async fn new(scope: TokenScope) -> (Self, CreatedToken) {
        let memory = Arc::new(Memory::with_role(Member));
        memory
            .event_roles
            .lock()
            .unwrap()
            .push(EventRole::EventContributor);
        let clock = Arc::new(TestClock::new());
        let created = create(&memory, Member, request(scope)).await.unwrap();
        let authenticator = TokenAuthenticator::new(memory.clone(), memory.clone(), clock.clone());
        (
            Self {
                memory,
                clock,
                authenticator,
            },
            created,
        )
    }

    async fn authenticate(&self, secret: &str) -> Result<Authenticated, AuthenticationError> {
        self.authenticator
            .authenticate(Some(Credential::ApiToken(secret)))
            .await
    }
}

fn unauthenticated(result: Result<Authenticated, AuthenticationError>) -> bool {
    matches!(result, Err(AuthenticationError::Unauthenticated))
}

#[tokio::test]
async fn a_token_gives_an_ai_caller_for_its_member() {
    let (fixture, created) = Fixture::new(TokenScope::Propose).await;
    let Authenticated::Ai(caller) = fixture
        .authenticate(created.secret.expose_secret())
        .await
        .unwrap()
    else {
        panic!("a token gives an AI caller");
    };
    assert_eq!(caller.token_id(), created.token.id);
    assert_eq!(caller.token_scope(), TokenScope::Propose);
    assert_eq!(crate::access::Principal::user_id(&caller), anna());
    assert_eq!(caller.organization_role(), Member);
    let actor = caller.actor();
    assert_eq!(actor.kind(), crate::caller::ActorKind::Ai);
    assert_eq!(actor.principal(), Some(anna().as_uuid()));
}

#[tokio::test]
async fn a_missing_unknown_or_session_credential_is_unauthenticated() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    for credential in [
        None,
        Some(Credential::ApiToken("tada_pat_unknown")),
        Some(Credential::ApiToken("unknown")),
        Some(Credential::Session(created.secret.expose_secret())),
    ] {
        let result = fixture.authenticator.authenticate(credential).await;
        assert!(unauthenticated(result), "{credential:?}");
    }
}

#[tokio::test]
async fn a_revoked_token_is_unauthenticated() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    revoke_token(
        &anna_as(Member),
        created.token.id,
        &*fixture.memory,
        &*fixture.clock,
    )
    .await
    .unwrap();
    assert!(unauthenticated(
        fixture.authenticate(created.secret.expose_secret()).await
    ));
}

#[tokio::test]
async fn an_expired_token_is_unauthenticated() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    let secret = created.secret.expose_secret();
    fixture.clock.set(DAY * 30 - SECOND);
    assert!(fixture.authenticate(secret).await.is_ok());
    fixture.clock.set(DAY * 30);
    assert!(unauthenticated(fixture.authenticate(secret).await));
}

#[tokio::test]
async fn the_token_of_a_removed_member_is_unauthenticated() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    *fixture.memory.role.lock().unwrap() = None;
    assert!(unauthenticated(
        fixture.authenticate(created.secret.expose_secret()).await
    ));
}

#[tokio::test]
async fn a_token_is_unauthenticated_while_mcp_tokens_are_off() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    let secret = created.secret.expose_secret();
    let owner = MemberCaller::new(anna(), testwil(), Owner);
    let off = set_feature(
        &owner,
        Feature::McpTokens,
        false,
        RecordVersion::FIRST,
        &*fixture.memory,
    )
    .await
    .unwrap();
    assert!(unauthenticated(fixture.authenticate(secret).await));

    set_feature(
        &owner,
        Feature::McpTokens,
        true,
        off.version,
        &*fixture.memory,
    )
    .await
    .unwrap();
    assert!(fixture.authenticate(secret).await.is_ok());
}

#[tokio::test]
async fn the_last_use_changes_at_most_once_a_minute() {
    let (fixture, created) = Fixture::new(TokenScope::Read).await;
    let secret = created.secret.expose_secret();
    fixture.authenticate(secret).await.unwrap();
    assert_eq!(*fixture.memory.touches.lock().unwrap(), 1, "the first use");
    for second in [1, 30, 59] {
        fixture.clock.set(SECOND * second);
        fixture.authenticate(secret).await.unwrap();
    }
    assert_eq!(*fixture.memory.touches.lock().unwrap(), 1);

    fixture.clock.set(TOUCH_INTERVAL);
    fixture.authenticate(secret).await.unwrap();
    assert_eq!(*fixture.memory.touches.lock().unwrap(), 2);
    assert_eq!(
        fixture.memory.token(secret).token.last_used_at,
        Some(START + TOUCH_INTERVAL)
    );
}

#[test]
fn scopes_have_stable_names() {
    let names = [TokenScope::Read, TokenScope::Propose].map(TokenScope::as_str);
    assert_eq!(names, ["read", "propose"]);
    for name in names {
        assert_eq!(TokenScope::parse(name).map(TokenScope::as_str), Some(name));
    }
    assert_eq!(TokenScope::parse("write"), None);
}

#[test]
fn constants_have_the_values_of_the_adrs() {
    assert_eq!(TOKEN_PREFIX, "tada_pat_");
    assert_eq!(NOTICE_VERSION, 1);
    assert_eq!(MAX_LIFETIME, SignedDuration::from_hours(365 * 24));
    assert_eq!(Feature::McpTokens.as_str(), "mcp-tokens");
    assert_eq!(Feature::parse("mcp-tokens"), Some(Feature::McpTokens));
    assert_eq!(Feature::parse("telegram"), None);
}

#[test]
fn each_error_has_a_code_in_its_list() {
    for error in [
        TokenError::Forbidden,
        TokenError::Disabled,
        TokenError::NotFound,
        TokenError::VersionConflict,
        TokenError::Invalid(Vec::new()),
        TokenError::Store(StoreError::Internal("test".into())),
        TokenError::Store(StoreError::Unavailable("test".into())),
    ] {
        assert!(TokenError::CODES.contains(&error.code()), "{error:?}");
    }
}
