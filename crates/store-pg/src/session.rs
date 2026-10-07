//! The `SessionStore` adapter (ADR 0008, ADR 0056).
//!
//! A session belongs to a user, not to an organization.
//! Its queries are infrastructure queries without a scope (ADR 0039).
//! Each query finds the row by the hash of the token; no column holds the token.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::SecretString;
use sqlx::PgConnection;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::session::{SessionRow, SessionStore};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;
use crate::token::{hash_token, new_token};

/// Starts a session and returns its new token. This is the only code that writes a session row,
/// so that the sign-in transactions and the tests make the same row.
pub(crate) async fn insert_session(
    conn: &mut PgConnection,
    user_id: UserId,
    organization_id: Option<OrganizationId>,
    user_agent: Option<&str>,
    now: Timestamp,
) -> Result<SecretString, StoreError> {
    let token = new_token("")?;
    sqlx::query!(
        "INSERT INTO session (token_hash, user_id, organization_id, created_at, last_used_at, user_agent)
         VALUES ($1, $2, $3, $4, $4, $5)",
        token.hash,
        user_id.as_uuid(),
        organization_id.map(OrganizationId::as_uuid),
        now.to_sqlx() as _,
        user_agent,
    )
    .execute(conn)
    .await
    .map_err(store_error)?;
    Ok(token.secret)
}

#[async_trait]
impl SessionStore for Database {
    async fn find(&self, token: &str, now: Timestamp) -> Result<Option<SessionRow>, StoreError> {
        let hash = hash_token(token);
        let row = sqlx::query!(
            r#"SELECT user_id, organization_id,
                      created_at AS "created_at: jiff_sqlx::Timestamp",
                      last_used_at AS "last_used_at: jiff_sqlx::Timestamp"
               FROM session WHERE token_hash = $1"#,
            hash,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let session = SessionRow {
            user_id: UserId::from_uuid(row.user_id),
            organization_id: row.organization_id.map(OrganizationId::from_uuid),
            created_at: row.created_at.to_jiff(),
            last_used_at: row.last_used_at.to_jiff(),
        };
        if session.is_expired(now) {
            self.delete(token).await?;
            return Ok(None);
        }
        Ok(Some(session))
    }

    async fn touch(&self, token: &str, now: Timestamp) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE session SET last_used_at = greatest(last_used_at, $2) WHERE token_hash = $1",
            hash_token(token),
            now.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn set_organization(
        &self,
        token: &str,
        organization_id: Option<OrganizationId>,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE session SET organization_id = $2 WHERE token_hash = $1",
            hash_token(token),
            organization_id.map(OrganizationId::as_uuid),
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn delete(&self, token: &str) -> Result<(), StoreError> {
        sqlx::query!(
            "DELETE FROM session WHERE token_hash = $1",
            hash_token(token)
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use secrecy::ExposeSecret;
    use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
    use tada_app::session::IDLE_TIMEOUT;

    use super::*;
    use crate::testing::TestDatabase;

    fn now() -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }

    async fn anna(test: &TestDatabase) -> UserId {
        test.create_user(
            &DisplayName::parse("Anna Muster").unwrap(),
            &Email::parse("anna@example.org").unwrap(),
        )
        .await
    }

    async fn create(
        test: &TestDatabase,
        user: UserId,
        organization: Option<OrganizationId>,
        user_agent: Option<&str>,
        now: Timestamp,
    ) -> SecretString {
        let mut conn = test.database.pool.acquire().await.unwrap();
        insert_session(&mut conn, user, organization, user_agent, now)
            .await
            .unwrap()
    }

    async fn count(test: &TestDatabase) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM session")
            .fetch_one(&test.database.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn stores_only_the_hash_of_a_session_token() {
        let test = TestDatabase::start().await;
        let user = anna(&test).await;
        let token = create(&test, user, None, Some("Firefox"), now()).await;

        test.assert_no_plaintext(token.expose_secret()).await;
        let hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM session")
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(hash, hash_token(token.expose_secret()));
    }

    #[tokio::test]
    async fn finds_a_session_by_its_token() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let user = anna(&test).await;
        let token = create(&test, user, Some(testwil), None, now()).await;

        let session = test
            .database
            .find(token.expose_secret(), now())
            .await
            .unwrap();
        assert_eq!(
            session,
            Some(SessionRow {
                user_id: user,
                organization_id: Some(testwil),
                created_at: now(),
                last_used_at: now(),
            })
        );
        assert_eq!(test.database.find("unknown", now()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn find_deletes_an_expired_session() {
        let test = TestDatabase::start().await;
        let user = anna(&test).await;
        let token = create(&test, user, None, None, now()).await;

        let later = now() + IDLE_TIMEOUT;
        assert_eq!(
            test.database
                .find(token.expose_secret(), later)
                .await
                .unwrap(),
            None
        );
        assert_eq!(count(&test).await, 0);
    }

    #[tokio::test]
    async fn touch_and_set_organization_change_the_session() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let user = anna(&test).await;
        let token = create(&test, user, None, None, now()).await;
        let token = token.expose_secret();

        let later = now() + SignedDuration::from_hours(1);
        test.database.touch(token, later).await.unwrap();
        test.database
            .set_organization(token, Some(testwil))
            .await
            .unwrap();
        let session = test.database.find(token, later).await.unwrap().unwrap();
        assert_eq!(session.last_used_at, later);
        assert_eq!(session.organization_id, Some(testwil));

        test.database.set_organization(token, None).await.unwrap();
        let session = test.database.find(token, later).await.unwrap().unwrap();
        assert_eq!(session.organization_id, None);
    }

    #[tokio::test]
    async fn delete_ends_the_session() {
        let test = TestDatabase::start().await;
        let user = anna(&test).await;
        let token = create(&test, user, None, None, now()).await;

        test.database.delete(token.expose_secret()).await.unwrap();
        assert_eq!(
            test.database
                .find(token.expose_secret(), now())
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn two_sessions_of_one_user_are_independent() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let user = anna(&test).await;
        let first = create(&test, user, None, None, now()).await;
        let second = create(&test, user, None, None, now()).await;
        assert_ne!(first.expose_secret(), second.expose_secret());

        test.database
            .set_organization(first.expose_secret(), Some(testwil))
            .await
            .unwrap();
        let session = test
            .database
            .find(second.expose_secret(), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.organization_id, None);

        test.database.delete(first.expose_secret()).await.unwrap();
        assert!(
            test.database
                .find(second.expose_secret(), now())
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn the_cookie_value_of_a_test_member_is_a_valid_session() {
        let test = TestDatabase::start().await;
        let (testwil, user, cookie) = test.member("testwil", OrganizationRole::Admin).await;
        let (same, other, _) = test.member("testwil", OrganizationRole::Member).await;
        assert_eq!(same, testwil);
        assert_ne!(other, user);

        let session = test
            .database
            .find(&cookie, Timestamp::now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.user_id, user);
        assert_eq!(session.organization_id, Some(testwil));

        let cookie = test.sign_in(user, None, Timestamp::now()).await;
        let session = test
            .database
            .find(&cookie, Timestamp::now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.organization_id, None);
    }

    #[tokio::test]
    async fn deleting_a_user_deletes_the_sessions() {
        let test = TestDatabase::start().await;
        let user = anna(&test).await;
        create(&test, user, None, None, now()).await;
        sqlx::query("DELETE FROM email_identity WHERE user_id = $1")
            .bind(user.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM app_user WHERE id = $1")
            .bind(user.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(count(&test).await, 0);
    }
}
