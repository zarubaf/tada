//! The `TokenStore` adapter: personal API tokens and organization features (ADR 0039, ADR 0036).
//!
//! The `api_token` table holds the hash of each token only (ADR 0008).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::SecretString;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::TokenName;
use tada_app::domain::ids::{ApiTokenId, OrganizationId, UserId};
use tada_app::store::StoreError;
use tada_app::tokens::{
    ApiToken, Feature, FeatureState, StoredToken, TOKEN_PREFIX, TokenScope, TokenStore,
};

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};
use crate::token::{hash_token, new_token};

struct TokenRow {
    id: Uuid,
    organization_id: Uuid,
    user_id: Uuid,
    name: String,
    scope: String,
    expires_at: jiff_sqlx::Timestamp,
    notice_version: i32,
    created_at: jiff_sqlx::Timestamp,
    last_used_at: Option<jiff_sqlx::Timestamp>,
    revoked_at: Option<jiff_sqlx::Timestamp>,
}

impl TryFrom<TokenRow> for StoredToken {
    type Error = InvalidRow;

    fn try_from(row: TokenRow) -> Result<Self, InvalidRow> {
        Ok(StoredToken {
            organization_id: OrganizationId::from_uuid(row.organization_id),
            user_id: UserId::from_uuid(row.user_id),
            token: ApiToken {
                id: ApiTokenId::from_uuid(row.id),
                name: TokenName::parse(&row.name).map_err(|_| InvalidRow("api_token.name"))?,
                scope: TokenScope::parse(&row.scope).ok_or(InvalidRow("api_token.scope"))?,
                expires_at: row.expires_at.to_jiff(),
                notice_version: u32::try_from(row.notice_version)
                    .map_err(|_| InvalidRow("api_token.notice_version"))?,
                created_at: row.created_at.to_jiff(),
                last_used_at: row.last_used_at.map(|time| time.to_jiff()),
                revoked_at: row.revoked_at.map(|time| time.to_jiff()),
            },
        })
    }
}

#[async_trait]
impl TokenStore for Database {
    async fn insert(
        &self,
        scope: OrgScope,
        user: UserId,
        token: &ApiToken,
        audit: &AuditEvent,
    ) -> Result<SecretString, StoreError> {
        let secret = new_token(TOKEN_PREFIX)?;
        let notice_version = i32::try_from(token.notice_version)
            .map_err(|error| StoreError::Internal(Box::new(error)))?;
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        sqlx::query!(
            "INSERT INTO api_token
                 (id, organization_id, user_id, token_hash, name, scope, expires_at,
                  notice_version, notice_confirmed_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)",
            token.id.as_uuid(),
            scope.organization_id().as_uuid(),
            user.as_uuid(),
            secret.hash,
            token.name.as_str(),
            token.scope.as_str(),
            token.expires_at.to_sqlx() as _,
            notice_version,
            token.created_at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(secret.secret)
    }

    async fn list(&self, scope: OrgScope, user: UserId) -> Result<Vec<ApiToken>, StoreError> {
        let rows = sqlx::query_as!(
            TokenRow,
            r#"SELECT id, organization_id, user_id, name, scope, notice_version,
                      expires_at AS "expires_at: jiff_sqlx::Timestamp",
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      last_used_at AS "last_used_at: jiff_sqlx::Timestamp",
                      revoked_at AS "revoked_at: jiff_sqlx::Timestamp"
               FROM api_token
               WHERE organization_id = $1 AND user_id = $2
               ORDER BY created_at DESC, id DESC"#,
            scope.organization_id().as_uuid(),
            user.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| Ok(StoredToken::try_from(row)?.token))
            .collect()
    }

    async fn revoke(
        &self,
        scope: OrgScope,
        user: UserId,
        id: ApiTokenId,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The row lock orders two revocations, so only one of them records an audit event.
        let found = sqlx::query_scalar!(
            "SELECT revoked_at IS NULL AS \"active!\" FROM api_token
             WHERE organization_id = $1 AND user_id = $2 AND id = $3
             FOR UPDATE",
            scope.organization_id().as_uuid(),
            user.as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        match found {
            None => return Ok(false),
            Some(false) => return Ok(true),
            Some(true) => {}
        }
        sqlx::query!(
            "UPDATE api_token SET revoked_at = $3 WHERE organization_id = $1 AND id = $2",
            scope.organization_id().as_uuid(),
            id.as_uuid(),
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(true)
    }

    async fn find(&self, secret: &str) -> Result<Option<StoredToken>, StoreError> {
        let row = sqlx::query_as!(
            TokenRow,
            r#"SELECT id, organization_id, user_id, name, scope, notice_version,
                      expires_at AS "expires_at: jiff_sqlx::Timestamp",
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      last_used_at AS "last_used_at: jiff_sqlx::Timestamp",
                      revoked_at AS "revoked_at: jiff_sqlx::Timestamp"
               FROM api_token WHERE token_hash = $1"#,
            hash_token(secret),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(StoredToken::try_from).transpose()?)
    }

    async fn touch(
        &self,
        scope: OrgScope,
        id: ApiTokenId,
        now: Timestamp,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE api_token SET last_used_at = greatest(last_used_at, $3)
             WHERE organization_id = $1 AND id = $2",
            scope.organization_id().as_uuid(),
            id.as_uuid(),
            now.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn features(&self, scope: OrgScope) -> Result<Vec<FeatureState>, StoreError> {
        let rows = sqlx::query!(
            "SELECT feature, enabled, version FROM organization_feature
             WHERE organization_id = $1 ORDER BY feature",
            scope.organization_id().as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(FeatureState {
                    feature: Feature::parse(&row.feature)
                        .ok_or(InvalidRow("organization_feature.feature"))?,
                    enabled: row.enabled,
                    version: RecordVersion::new(row.version)
                        .ok_or(InvalidRow("organization_feature.version"))?,
                })
            })
            .collect()
    }

    async fn set_feature(
        &self,
        scope: OrgScope,
        feature: Feature,
        enabled: bool,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<FeatureState>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let mut version = sqlx::query_scalar!(
            "UPDATE organization_feature SET enabled = $3, version = version + 1
             WHERE organization_id = $1 AND feature = $2 AND version = $4
             RETURNING version",
            organization,
            feature.as_str(),
            enabled,
            expected_version.get(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        // A feature without a row has version 1, so its first change inserts version 2.
        // A concurrent first change inserts the row first; then this one is a version conflict.
        if version.is_none() && expected_version == RecordVersion::FIRST {
            version = sqlx::query_scalar!(
                "INSERT INTO organization_feature (organization_id, feature, enabled, version)
                 VALUES ($1, $2, $3, 2)
                 ON CONFLICT (organization_id, feature) DO NOTHING
                 RETURNING version",
                organization,
                feature.as_str(),
                enabled,
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(store_error)?;
        }
        let Some(version) = version else {
            return Ok(None);
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(FeatureState {
            feature,
            enabled,
            version: RecordVersion::new(version)
                .ok_or(InvalidRow("organization_feature.version"))?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use jiff::SignedDuration;
    use secrecy::ExposeSecret;
    use tada_app::auth::{Authenticated, AuthenticationError, Authenticator, Credential};
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::clock::Clock;
    use tada_app::domain::identity::EventRole;
    use tada_app::members::remove_member;
    use tada_app::tokens::{
        CreatedToken, NOTICE_VERSION, TokenAuthenticator, TokenError, TokenRequest, create_token,
        get_features, list_tokens, revoke_token, set_feature,
    };

    use super::*;
    use crate::testing::{TestDatabase, sqlstate};

    const DAY: SignedDuration = SignedDuration::from_hours(24);

    fn start() -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }

    /// A clock that the test moves.
    #[derive(Debug)]
    struct TestClock(Mutex<Timestamp>);

    impl TestClock {
        fn new() -> Self {
            Self(Mutex::new(start()))
        }

        fn set(&self, offset: SignedDuration) {
            *self.0.lock().unwrap() = start() + offset;
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Timestamp {
            *self.0.lock().unwrap()
        }
    }

    fn request(scope: TokenScope) -> TokenRequest {
        TokenRequest {
            name: "Claude Desktop".to_owned(),
            scope,
            expires_at: start() + DAY * 30,
            notice_version_confirmed: NOTICE_VERSION,
        }
    }

    /// A new member of Testwil with the role `role`.
    async fn member(test: &TestDatabase, role: OrganizationRole) -> MemberCaller {
        let (organization, user, _) = test.member("testwil", role).await;
        MemberCaller::new(user, organization, role)
    }

    async fn create(
        test: &TestDatabase,
        caller: &MemberCaller,
        scope: TokenScope,
        clock: &TestClock,
    ) -> Result<CreatedToken, TokenError> {
        // The member signed in just now (`session::RECENT_SIGN_IN`).
        create_token(
            &caller.clone().with_sign_in(clock.now()),
            request(scope),
            &test.database,
            &test.database,
            clock,
        )
        .await
    }

    async fn count(test: &TestDatabase, sql: &str) -> i64 {
        test.scalar(sql).await
    }

    struct Fixture {
        test: TestDatabase,
        clock: Arc<TestClock>,
        authenticator: TokenAuthenticator,
    }

    impl Fixture {
        async fn new() -> Self {
            let test = TestDatabase::start().await;
            let clock = Arc::new(TestClock::new());
            let database = Arc::new(test.database.clone());
            let authenticator = TokenAuthenticator::new(database.clone(), database, clock.clone());
            Self {
                test,
                clock,
                authenticator,
            }
        }

        async fn authenticate(
            &self,
            token: &CreatedToken,
        ) -> Result<Authenticated, AuthenticationError> {
            self.authenticator
                .authenticate(Some(Credential::ApiToken(token.secret.expose_secret())))
                .await
        }

        async fn assert_unauthenticated(&self, token: &CreatedToken) {
            let result = self.authenticate(token).await;
            assert!(
                matches!(result, Err(AuthenticationError::Unauthenticated)),
                "{result:?}"
            );
        }
    }

    #[tokio::test]
    async fn tokens_start_with_the_prefix_and_the_database_holds_only_the_hash() {
        let test = TestDatabase::start().await;
        let anna = member(&test, OrganizationRole::Member).await;
        let created = create(&test, &anna, TokenScope::Read, &TestClock::new())
            .await
            .unwrap();
        let secret = created.secret.expose_secret();

        assert!(secret.starts_with("tada_pat_"));
        test.assert_no_plaintext(secret).await;
        let hash: Vec<u8> = test.scalar("SELECT token_hash FROM api_token").await;
        assert_eq!(hash, hash_token(secret));
        let found = test.database.find(secret).await.unwrap().unwrap();
        assert_eq!(found.token, created.token);
        assert_eq!(found.user_id, anna.user_id());
        assert_eq!(found.organization_id, anna.scope().organization_id());
        let confirmed: i64 = test
            .scalar("SELECT count(*) FROM api_token WHERE notice_version = 1 AND notice_confirmed_at = created_at")
            .await;
        assert_eq!(confirmed, 1);
        assert_eq!(
            count(
                &test,
                "SELECT count(*) FROM audit_event WHERE action = 'api_token.create'"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn tokens_list_and_revoke_only_the_own_tokens() {
        let test = TestDatabase::start().await;
        let clock = TestClock::new();
        let anna = member(&test, OrganizationRole::Member).await;
        let bruno = member(&test, OrganizationRole::Member).await;
        let first = create(&test, &anna, TokenScope::Read, &clock)
            .await
            .unwrap();
        clock.set(DAY);
        let second = create(&test, &anna, TokenScope::Read, &clock)
            .await
            .unwrap();
        let other = create(&test, &bruno, TokenScope::Read, &clock)
            .await
            .unwrap();

        let listed = list_tokens(&anna, &test.database).await.unwrap();
        assert_eq!(listed, [second.token.clone(), first.token.clone()]);

        let result = revoke_token(&anna, other.token.id, &test.database, &clock).await;
        assert!(matches!(result, Err(TokenError::NotFound)), "{result:?}");
        clock.set(DAY * 2);
        revoke_token(&anna, first.token.id, &test.database, &clock)
            .await
            .unwrap();
        revoke_token(&anna, first.token.id, &test.database, &clock)
            .await
            .unwrap();
        let listed = list_tokens(&anna, &test.database).await.unwrap();
        assert_eq!(listed[1].revoked_at, Some(start() + DAY * 2));
        assert_eq!(listed[0].revoked_at, None);
        assert_eq!(
            count(
                &test,
                "SELECT count(*) FROM audit_event WHERE action = 'api_token.revoke'"
            )
            .await,
            1,
            "a second revocation records nothing"
        );
    }

    #[tokio::test]
    async fn tokens_stay_in_their_organization() {
        let test = TestDatabase::start().await;
        let clock = TestClock::new();
        let anna = member(&test, OrganizationRole::Member).await;
        let token = create(&test, &anna, TokenScope::Read, &clock)
            .await
            .unwrap();
        // The same user in another organization sees and revokes nothing of Testwil.
        let musterhausen = test.create_organization("musterhausen").await;
        test.add_membership(musterhausen, anna.user_id(), OrganizationRole::Member)
            .await;
        let elsewhere = MemberCaller::new(anna.user_id(), musterhausen, OrganizationRole::Member);

        assert!(
            list_tokens(&elsewhere, &test.database)
                .await
                .unwrap()
                .is_empty()
        );
        let result = revoke_token(&elsewhere, token.token.id, &test.database, &clock).await;
        assert!(matches!(result, Err(TokenError::NotFound)), "{result:?}");
        assert_eq!(
            list_tokens(&anna, &test.database).await.unwrap()[0].revoked_at,
            None
        );
    }

    #[tokio::test]
    async fn tokens_names_follow_the_name_rules_in_the_schema() {
        let test = TestDatabase::start().await;
        let anna = member(&test, OrganizationRole::Member).await;
        for name in ["", &"ä".repeat(201)] {
            let error = sqlx::query(
                "INSERT INTO api_token (id, organization_id, user_id, token_hash, name, scope,
                     expires_at, notice_version, notice_confirmed_at, created_at)
                 VALUES ($1, $2, $3, $4, $5, 'read', now() + interval '1 day', 1, now(), now())",
            )
            .bind(Uuid::now_v7())
            .bind(anna.scope().organization_id().as_uuid())
            .bind(anna.user_id().as_uuid())
            .bind(hash_token(name))
            .bind(name)
            .execute(&test.database.pool)
            .await
            .unwrap_err();
            assert_eq!(sqlstate(&error), "23514", "a CHECK violation");
        }
    }

    #[tokio::test]
    async fn tokens_of_a_viewer_are_read_only() {
        let test = TestDatabase::start().await;
        let clock = TestClock::new();
        let viewer = member(&test, OrganizationRole::Member).await;
        let contributor = member(&test, OrganizationRole::Member).await;
        let organization = viewer.scope().organization_id();
        let event = test.create_event(organization, "OPEN30").await;
        for (caller, role) in [
            (&viewer, EventRole::EventViewer),
            (&contributor, EventRole::EventContributor),
        ] {
            sqlx::query(
                "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
                 VALUES ($1, $2, $3, $4, 1, now())",
            )
            .bind(organization.as_uuid())
            .bind(event.as_uuid())
            .bind(caller.user_id().as_uuid())
            .bind(role.as_str())
            .execute(&test.database.pool)
            .await
            .unwrap();
        }

        let result = create(&test, &viewer, TokenScope::Propose, &clock).await;
        assert!(matches!(result, Err(TokenError::Forbidden)), "{result:?}");
        assert!(
            create(&test, &viewer, TokenScope::Read, &clock)
                .await
                .is_ok()
        );
        let token = create(&test, &contributor, TokenScope::Propose, &clock)
            .await
            .unwrap();
        assert_eq!(token.token.scope, TokenScope::Propose);
    }

    #[tokio::test]
    async fn tokens_give_an_ai_caller_and_record_the_last_use() {
        let fixture = Fixture::new().await;
        let anna = member(&fixture.test, OrganizationRole::Member).await;
        let created = create(&fixture.test, &anna, TokenScope::Read, &fixture.clock)
            .await
            .unwrap();

        fixture.clock.set(DAY);
        let Authenticated::Ai(caller) = fixture.authenticate(&created).await.unwrap() else {
            panic!("a token gives an AI caller");
        };
        assert_eq!(caller.token_id(), created.token.id);
        assert_eq!(caller.actor().principal(), Some(anna.user_id().as_uuid()));
        let listed = list_tokens(&anna, &fixture.test.database).await.unwrap();
        assert_eq!(listed[0].last_used_at, Some(start() + DAY));
    }

    #[tokio::test]
    async fn tokens_that_are_revoked_or_expired_are_unauthenticated() {
        let fixture = Fixture::new().await;
        let anna = member(&fixture.test, OrganizationRole::Member).await;
        let clock = &*fixture.clock;
        let revoked = create(&fixture.test, &anna, TokenScope::Read, clock)
            .await
            .unwrap();
        let expired = create(&fixture.test, &anna, TokenScope::Read, clock)
            .await
            .unwrap();
        revoke_token(&anna, revoked.token.id, &fixture.test.database, clock)
            .await
            .unwrap();
        fixture.assert_unauthenticated(&revoked).await;

        fixture.clock.set(DAY * 30);
        fixture.assert_unauthenticated(&expired).await;
    }

    #[tokio::test]
    async fn tokens_of_a_removed_member_are_unauthenticated_and_deleted() {
        let fixture = Fixture::new().await;
        let owner = member(&fixture.test, OrganizationRole::Owner).await;
        let anna = member(&fixture.test, OrganizationRole::Member).await;
        let created = create(&fixture.test, &anna, TokenScope::Read, &fixture.clock)
            .await
            .unwrap();
        assert!(fixture.authenticate(&created).await.is_ok());

        remove_member(
            &owner,
            anna.user_id(),
            RecordVersion::FIRST,
            &fixture.test.database,
        )
        .await
        .unwrap();
        fixture.assert_unauthenticated(&created).await;
        assert_eq!(
            count(&fixture.test, "SELECT count(*) FROM api_token").await,
            0
        );
    }

    #[tokio::test]
    async fn tokens_are_unauthenticated_while_the_owner_switches_them_off() {
        let fixture = Fixture::new().await;
        let owner = member(&fixture.test, OrganizationRole::Owner).await;
        let anna = member(&fixture.test, OrganizationRole::Member).await;
        let database = &fixture.test.database;
        let created = create(&fixture.test, &anna, TokenScope::Read, &fixture.clock)
            .await
            .unwrap();

        let off = set_feature(
            &owner,
            Feature::McpTokens,
            false,
            RecordVersion::FIRST,
            database,
        )
        .await
        .unwrap();
        assert_eq!((off.enabled, off.version.get()), (false, 2));
        fixture.assert_unauthenticated(&created).await;
        let result = create(&fixture.test, &anna, TokenScope::Read, &fixture.clock).await;
        assert!(matches!(result, Err(TokenError::Disabled)), "{result:?}");

        let stale = set_feature(
            &owner,
            Feature::McpTokens,
            true,
            RecordVersion::FIRST,
            database,
        )
        .await;
        assert!(
            matches!(stale, Err(TokenError::VersionConflict)),
            "{stale:?}"
        );
        let on = set_feature(&owner, Feature::McpTokens, true, off.version, database)
            .await
            .unwrap();
        assert_eq!((on.enabled, on.version.get()), (true, 3));
        assert_eq!(get_features(&anna, database).await.unwrap(), [on]);
        assert!(fixture.authenticate(&created).await.is_ok());
        assert_eq!(
            count(
                &fixture.test,
                "SELECT count(*) FROM audit_event WHERE record_kind = 'organization_feature'"
            )
            .await,
            2
        );
    }

    #[tokio::test]
    async fn tokens_features_stay_in_their_organization() {
        let test = TestDatabase::start().await;
        let (testwil, owner, _) = test.member("testwil", OrganizationRole::Owner).await;
        let (musterhausen, other, _) = test.member("musterhausen", OrganizationRole::Owner).await;
        let owner = MemberCaller::new(owner, testwil, OrganizationRole::Owner);
        set_feature(
            &owner,
            Feature::McpTokens,
            false,
            RecordVersion::FIRST,
            &test.database,
        )
        .await
        .unwrap();

        let other = MemberCaller::new(other, musterhausen, OrganizationRole::Owner);
        let features = get_features(&other, &test.database).await.unwrap();
        assert!(features[0].enabled);
        assert_eq!(features[0].version, RecordVersion::FIRST);
    }
}
