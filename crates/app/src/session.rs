//! Sessions (ADR 0008) and the organization of a request (ADR 0056).
//!
//! A session names one user. It has an organization after the member chooses one.
//! The authenticator reads the membership in that organization on each request.

use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use tada_domain::identity::DisplayName;
use tada_domain::ids::{OrganizationId, UserId};

use crate::auth::{AuthenticationError, Authenticator, Credential};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::{IdentityStore, Membership};
use crate::problem::ProblemCode;
use crate::store::StoreError;

/// A session that nobody used for this time ends (ADR 0008).
pub const IDLE_TIMEOUT: SignedDuration = SignedDuration::from_hours(14 * 24);
/// A session ends at this age, also with regular use (ADR 0008).
pub const ABSOLUTE_TIMEOUT: SignedDuration = SignedDuration::from_hours(90 * 24);
/// The last-use time of a session changes at most once in this interval, to limit the writes.
pub const TOUCH_INTERVAL: SignedDuration = SignedDuration::from_mins(1);

/// A stored session. The store never gives the token back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub user_id: UserId,
    pub organization_id: Option<OrganizationId>,
    pub created_at: Timestamp,
    pub last_used_at: Timestamp,
}

impl SessionRow {
    /// True if the session is past its idle timeout or its absolute timeout at `now`.
    pub fn is_expired(&self, now: Timestamp) -> bool {
        now.duration_since(self.last_used_at) >= IDLE_TIMEOUT
            || now.duration_since(self.created_at) >= ABSOLUTE_TIMEOUT
    }
}

/// The repository port of sessions. Each method takes the token as the holder sends it.
/// Only the store hashes it; no table holds the token (ADR 0008).
#[async_trait]
pub trait SessionStore: Debug + Send + Sync {
    /// Starts a session and returns its new token.
    async fn create(
        &self,
        user_id: UserId,
        organization_id: Option<OrganizationId>,
        user_agent: Option<&str>,
        now: Timestamp,
    ) -> Result<SecretString, StoreError>;

    /// The session of `token`. An expired session (`SessionRow::is_expired`) gives `None`,
    /// and the store deletes it.
    async fn find(&self, token: &str, now: Timestamp) -> Result<Option<SessionRow>, StoreError>;

    /// Sets the last-use time of the session to `now`.
    async fn touch(&self, token: &str, now: Timestamp) -> Result<(), StoreError>;

    /// Sets or clears the organization of the session. The token stays the same (ADR 0056).
    async fn set_organization(
        &self,
        token: &str,
        organization_id: Option<OrganizationId>,
    ) -> Result<(), StoreError>;

    /// Ends the session, for example at sign-out.
    async fn delete(&self, token: &str) -> Result<(), StoreError>;

    /// Ends all sessions of a user: the revocation of ADR 0008.
    async fn delete_all_of(&self, user_id: UserId) -> Result<(), StoreError>;
}

/// The session of `token` at `now`, after it counts as used.
async fn use_session(
    sessions: &dyn SessionStore,
    token: &str,
    now: Timestamp,
) -> Result<Option<SessionRow>, StoreError> {
    let Some(session) = sessions.find(token, now).await? else {
        return Ok(None);
    };
    if now.duration_since(session.last_used_at) >= TOUCH_INTERVAL {
        sessions.touch(token, now).await?;
    }
    Ok(Some(session))
}

/// The organization boundary of a membership check. No `MemberCaller` exists yet at this point.
fn scope(organization_id: OrganizationId) -> OrgScope {
    OrgScope::for_session(organization_id)
}

/// The signed-in user, the memberships and the organization of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub user_id: UserId,
    pub display_name: DisplayName,
    /// Empty until the member chooses an organization (ADR 0056).
    pub organization_id: Option<OrganizationId>,
    pub memberships: Vec<Membership>,
}

/// An error of `session_info`.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no valid session")]
    Unauthenticated,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SessionError {
    /// All codes that this query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Unauthenticated,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Unauthenticated => ProblemCode::Unauthenticated,
            Self::Store(error) => store_code(error),
        }
    }
}

/// The session of `token`. It works for a session without an organization.
/// The organization is empty if the membership in it no longer exists.
pub async fn session_info(
    token: &str,
    sessions: &dyn SessionStore,
    identity: &dyn IdentityStore,
    clock: &dyn Clock,
) -> Result<SessionInfo, SessionError> {
    let session = use_session(sessions, token, clock.now())
        .await?
        .ok_or(SessionError::Unauthenticated)?;
    let user = identity
        .user(session.user_id)
        .await?
        .ok_or(SessionError::Unauthenticated)?;
    let memberships = identity.memberships_of(session.user_id).await?;
    let organization_id = session.organization_id.filter(|organization| {
        memberships
            .iter()
            .any(|membership| membership.organization_id == *organization)
    });
    Ok(SessionInfo {
        user_id: user.id,
        display_name: user.display_name,
        organization_id,
        memberships,
    })
}

/// An error of `choose_organization`.
#[derive(Debug, thiserror::Error)]
pub enum ChooseOrganizationError {
    #[error("no valid session")]
    Unauthenticated,
    /// The user is no member of the organization. The organization stays invisible.
    #[error("no member of the organization")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ChooseOrganizationError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Unauthenticated,
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Unauthenticated => ProblemCode::Unauthenticated,
            Self::NotFound => ProblemCode::NotFound,
            Self::Store(error) => store_code(error),
        }
    }
}

/// Chooses or changes the organization of the session of `token` (ADR 0056).
/// The user must be a member of the organization.
pub async fn choose_organization(
    token: &str,
    organization_id: OrganizationId,
    sessions: &dyn SessionStore,
    identity: &dyn IdentityStore,
    clock: &dyn Clock,
) -> Result<(), ChooseOrganizationError> {
    let session = use_session(sessions, token, clock.now())
        .await?
        .ok_or(ChooseOrganizationError::Unauthenticated)?;
    identity
        .membership(scope(organization_id), session.user_id)
        .await?
        .ok_or(ChooseOrganizationError::NotFound)?;
    sessions
        .set_organization(token, Some(organization_id))
        .await?;
    Ok(())
}

/// The `Authenticator` of sessions (ADR 0008, ADR 0056).
pub struct SessionAuthenticator {
    sessions: Arc<dyn SessionStore>,
    identity: Arc<dyn IdentityStore>,
    clock: Arc<dyn Clock>,
}

impl SessionAuthenticator {
    pub fn new(
        sessions: Arc<dyn SessionStore>,
        identity: Arc<dyn IdentityStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            sessions,
            identity,
            clock,
        }
    }
}

impl Debug for SessionAuthenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionAuthenticator")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Authenticator for SessionAuthenticator {
    async fn authenticate(
        &self,
        credential: Option<Credential<'_>>,
    ) -> Result<MemberCaller, AuthenticationError> {
        // API tokens have their own authenticator.
        let Some(Credential::Session(token)) = credential else {
            return Err(AuthenticationError::Unauthenticated);
        };
        let session = use_session(&*self.sessions, token, self.clock.now())
            .await?
            .ok_or(AuthenticationError::Unauthenticated)?;
        let organization_id = session
            .organization_id
            .ok_or(AuthenticationError::OrganizationRequired)?;
        // The membership can change between two requests, so each request reads it.
        let Some(role) = self
            .identity
            .membership(scope(organization_id), session.user_id)
            .await?
        else {
            self.sessions.set_organization(token, None).await?;
            return Err(AuthenticationError::OrganizationRequired);
        };
        Ok(MemberCaller::new(session.user_id, organization_id, role))
    }
}

fn store_code(error: &StoreError) -> ProblemCode {
    match error {
        StoreError::Unavailable(_) => ProblemCode::Unavailable,
        StoreError::Internal(_) => ProblemCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use secrecy::ExposeSecret;
    use tada_domain::identity::{Email, EventRole, OrganizationRole};
    use tada_domain::ids::EventId;
    use uuid::Uuid;

    use super::*;
    use crate::identity::UserRef;

    const START: &str = "2030-05-18T08:00:00Z";
    const SECOND: SignedDuration = SignedDuration::from_secs(1);
    const DAY: SignedDuration = SignedDuration::from_hours(24);

    /// A clock that the test moves.
    #[derive(Debug)]
    struct TestClock(Mutex<Timestamp>);

    impl TestClock {
        fn new() -> Self {
            Self(Mutex::new(START.parse().unwrap()))
        }

        /// Sets the time to `offset` after the start.
        fn set(&self, offset: SignedDuration) {
            *self.0.lock().unwrap() = START.parse::<Timestamp>().unwrap() + offset;
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Timestamp {
            *self.0.lock().unwrap()
        }
    }

    #[derive(Debug, Default)]
    struct MemorySessions {
        rows: Mutex<HashMap<String, SessionRow>>,
        touches: Mutex<usize>,
    }

    #[async_trait]
    impl SessionStore for MemorySessions {
        async fn create(
            &self,
            user_id: UserId,
            organization_id: Option<OrganizationId>,
            _user_agent: Option<&str>,
            now: Timestamp,
        ) -> Result<SecretString, StoreError> {
            let token = Uuid::now_v7().to_string();
            self.rows.lock().unwrap().insert(
                token.clone(),
                SessionRow {
                    user_id,
                    organization_id,
                    created_at: now,
                    last_used_at: now,
                },
            );
            Ok(SecretString::from(token))
        }

        async fn find(
            &self,
            token: &str,
            now: Timestamp,
        ) -> Result<Option<SessionRow>, StoreError> {
            let mut rows = self.rows.lock().unwrap();
            match rows.get(token) {
                Some(row) if row.is_expired(now) => {
                    rows.remove(token);
                    Ok(None)
                }
                row => Ok(row.cloned()),
            }
        }

        async fn touch(&self, token: &str, now: Timestamp) -> Result<(), StoreError> {
            *self.touches.lock().unwrap() += 1;
            if let Some(row) = self.rows.lock().unwrap().get_mut(token) {
                row.last_used_at = now;
            }
            Ok(())
        }

        async fn set_organization(
            &self,
            token: &str,
            organization_id: Option<OrganizationId>,
        ) -> Result<(), StoreError> {
            if let Some(row) = self.rows.lock().unwrap().get_mut(token) {
                row.organization_id = organization_id;
            }
            Ok(())
        }

        async fn delete(&self, token: &str) -> Result<(), StoreError> {
            self.rows.lock().unwrap().remove(token);
            Ok(())
        }

        async fn delete_all_of(&self, user_id: UserId) -> Result<(), StoreError> {
            self.rows
                .lock()
                .unwrap()
                .retain(|_, row| row.user_id != user_id);
            Ok(())
        }
    }

    /// One user, Anna, and her memberships.
    #[derive(Debug, Default)]
    struct MemoryIdentity(Mutex<Vec<Membership>>);

    impl MemoryIdentity {
        fn add(&self, organization: OrganizationId, name: &str, role: OrganizationRole) {
            self.0.lock().unwrap().push(Membership {
                organization_id: organization,
                organization_name: name.to_owned(),
                role,
            });
        }

        fn remove(&self, organization: OrganizationId) {
            self.0
                .lock()
                .unwrap()
                .retain(|membership| membership.organization_id != organization);
        }
    }

    #[async_trait]
    impl IdentityStore for MemoryIdentity {
        async fn user(&self, id: UserId) -> Result<Option<UserRef>, StoreError> {
            Ok((id == anna()).then(|| UserRef {
                id,
                display_name: DisplayName::parse("Anna Muster").unwrap(),
                locale: "de-CH".to_owned(),
            }))
        }

        async fn user_by_email(&self, _: &Email) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn memberships_of(&self, user: UserId) -> Result<Vec<Membership>, StoreError> {
            assert_eq!(user, anna());
            Ok(self.0.lock().unwrap().clone())
        }

        async fn membership(
            &self,
            scope: OrgScope,
            user: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .find(|membership| {
                    user == anna() && membership.organization_id == scope.organization_id()
                })
                .map(|membership| membership.role))
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
    }

    fn anna() -> UserId {
        UserId::from_uuid(Uuid::from_u128(1))
    }

    fn testwil() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(10))
    }

    fn musterhausen() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(11))
    }

    struct Fixture {
        sessions: Arc<MemorySessions>,
        identity: Arc<MemoryIdentity>,
        clock: Arc<TestClock>,
        authenticator: SessionAuthenticator,
    }

    impl Fixture {
        /// Anna is an admin of Testwil.
        fn new() -> Self {
            let sessions = Arc::new(MemorySessions::default());
            let identity = Arc::new(MemoryIdentity::default());
            identity.add(testwil(), "Testwil", OrganizationRole::Admin);
            let clock = Arc::new(TestClock::new());
            let authenticator =
                SessionAuthenticator::new(sessions.clone(), identity.clone(), clock.clone());
            Self {
                sessions,
                identity,
                clock,
                authenticator,
            }
        }

        async fn sign_in(&self, organization: Option<OrganizationId>) -> String {
            self.sessions
                .create(anna(), organization, None, self.clock.now())
                .await
                .unwrap()
                .expose_secret()
                .to_owned()
        }

        async fn authenticate(&self, token: &str) -> Result<MemberCaller, AuthenticationError> {
            self.authenticator
                .authenticate(Some(Credential::Session(token)))
                .await
        }

        async fn choose(
            &self,
            token: &str,
            organization: OrganizationId,
        ) -> Result<(), ChooseOrganizationError> {
            choose_organization(
                token,
                organization,
                &*self.sessions,
                &*self.identity,
                &*self.clock,
            )
            .await
        }
    }

    #[tokio::test]
    async fn a_session_gives_the_member_of_its_organization() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;

        let caller = fixture.authenticate(&token).await.unwrap();
        assert_eq!(
            caller,
            MemberCaller::new(anna(), testwil(), OrganizationRole::Admin)
        );
    }

    #[tokio::test]
    async fn a_missing_or_unknown_credential_is_unauthenticated() {
        let fixture = Fixture::new();
        for credential in [
            None,
            Some(Credential::Session("unknown")),
            Some(Credential::ApiToken("tada_pat_unknown")),
        ] {
            let result = fixture.authenticator.authenticate(credential).await;
            assert!(
                matches!(result, Err(AuthenticationError::Unauthenticated)),
                "{credential:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_session_ends_after_14_idle_days() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;

        fixture.clock.set(IDLE_TIMEOUT - SECOND);
        assert!(fixture.authenticate(&token).await.is_ok());

        // 14 days and 1 second after the last use.
        fixture
            .clock
            .set((IDLE_TIMEOUT - SECOND) + IDLE_TIMEOUT + SECOND);
        assert!(matches!(
            fixture.authenticate(&token).await,
            Err(AuthenticationError::Unauthenticated)
        ));
        assert!(
            fixture.sessions.rows.lock().unwrap().is_empty(),
            "the store deletes the expired session"
        );
    }

    #[tokio::test]
    async fn a_session_ends_after_90_days_also_with_regular_use() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;

        for day in 1..90 {
            fixture.clock.set(DAY * day);
            assert!(fixture.authenticate(&token).await.is_ok(), "day {day}");
        }
        fixture.clock.set(ABSOLUTE_TIMEOUT - SECOND);
        assert!(fixture.authenticate(&token).await.is_ok());

        fixture.clock.set(ABSOLUTE_TIMEOUT + SECOND);
        assert!(matches!(
            fixture.authenticate(&token).await,
            Err(AuthenticationError::Unauthenticated)
        ));
    }

    #[tokio::test]
    async fn the_last_use_changes_at_most_once_a_minute() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;

        for second in [1, 30, 59] {
            fixture.clock.set(SECOND * second);
            fixture.authenticate(&token).await.unwrap();
        }
        assert_eq!(*fixture.sessions.touches.lock().unwrap(), 0);

        fixture.clock.set(TOUCH_INTERVAL);
        fixture.authenticate(&token).await.unwrap();
        fixture.clock.set(TOUCH_INTERVAL + SECOND);
        fixture.authenticate(&token).await.unwrap();
        assert_eq!(*fixture.sessions.touches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_session_without_an_organization_requires_one() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(None).await;

        assert!(matches!(
            fixture.authenticate(&token).await,
            Err(AuthenticationError::OrganizationRequired)
        ));
    }

    #[tokio::test]
    async fn a_removed_membership_requires_an_organization_and_clears_it() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;
        fixture.identity.remove(testwil());

        assert!(matches!(
            fixture.authenticate(&token).await,
            Err(AuthenticationError::OrganizationRequired)
        ));
        let row = fixture.sessions.rows.lock().unwrap()[&token].clone();
        assert_eq!(row.organization_id, None);
    }

    #[tokio::test]
    async fn session_info_works_without_an_organization() {
        let fixture = Fixture::new();
        fixture
            .identity
            .add(musterhausen(), "Musterhausen", OrganizationRole::Member);
        let token = fixture.sign_in(None).await;

        let info = session_info(
            &token,
            &*fixture.sessions,
            &*fixture.identity,
            &*fixture.clock,
        )
        .await
        .unwrap();
        assert_eq!(info.user_id, anna());
        assert_eq!(info.display_name.as_str(), "Anna Muster");
        assert_eq!(info.organization_id, None);
        assert_eq!(info.memberships.len(), 2);
    }

    #[tokio::test]
    async fn session_info_of_an_unknown_token_is_unauthenticated() {
        let fixture = Fixture::new();
        let result = session_info(
            "unknown",
            &*fixture.sessions,
            &*fixture.identity,
            &*fixture.clock,
        )
        .await;
        assert!(matches!(result, Err(SessionError::Unauthenticated)));
    }

    #[tokio::test]
    async fn a_member_chooses_and_changes_the_organization() {
        let fixture = Fixture::new();
        fixture
            .identity
            .add(musterhausen(), "Musterhausen", OrganizationRole::Member);
        let token = fixture.sign_in(None).await;

        for organization in [testwil(), musterhausen()] {
            fixture.choose(&token, organization).await.unwrap();
            let caller = fixture.authenticate(&token).await.unwrap();
            assert_eq!(caller.scope().organization_id(), organization);
        }
    }

    #[tokio::test]
    async fn a_non_member_cannot_choose_the_organization() {
        let fixture = Fixture::new();
        let token = fixture.sign_in(Some(testwil())).await;

        let result = fixture.choose(&token, musterhausen()).await;
        assert!(matches!(result, Err(ChooseOrganizationError::NotFound)));
        let caller = fixture.authenticate(&token).await.unwrap();
        assert_eq!(caller.scope().organization_id(), testwil());
    }

    #[test]
    fn each_error_has_a_code_in_its_list() {
        let store = || StoreError::Internal("test".into());
        for error in [SessionError::Unauthenticated, SessionError::Store(store())] {
            assert!(SessionError::CODES.contains(&error.code()), "{error:?}");
        }
        for error in [
            ChooseOrganizationError::Unauthenticated,
            ChooseOrganizationError::NotFound,
            ChooseOrganizationError::Store(store()),
        ] {
            assert!(
                ChooseOrganizationError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
    }
}
