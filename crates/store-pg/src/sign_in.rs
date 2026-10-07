//! The `SignInStore` adapter (ADR 0008, ADR 0056).
//!
//! Sign-in has no organization yet, so its queries are infrastructure queries without a scope
//! (ADR 0039). A magic link belongs to a user, not to an organization.

use async_trait::async_trait;
use jiff::Timestamp;
use secrecy::SecretString;
use sqlx::types::Uuid;
use tada_app::domain::identity::Email;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::outbound::Purpose;
use tada_app::sign_in::{SignInStore, initial_organization};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;
use crate::outbound::queue_outbound;
use crate::session::insert_session;
use crate::token::hash_token;

#[async_trait]
impl SignInStore for Database {
    async fn queue_magic_link(
        &self,
        email: &Email,
        request_id: Option<Uuid>,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let user_id = sqlx::query_scalar!(
            "SELECT e.user_id FROM email_identity e
             WHERE e.email = $1
               AND EXISTS (SELECT 1 FROM organization_membership m WHERE m.user_id = e.user_id)",
            email.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        if let Some(user_id) = user_id {
            let purpose = Purpose::MagicLink {
                user_id: UserId::from_uuid(user_id),
            };
            queue_outbound(&mut tx, &purpose, request_id)
                .await
                .map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)
    }

    async fn redeem_magic_link(
        &self,
        token: &str,
        user_agent: Option<&str>,
        now: Timestamp,
    ) -> Result<Option<SecretString>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The delete locks the row, so a second redeem of the same token finds nothing.
        let link = sqlx::query!(
            r#"DELETE FROM magic_link WHERE token_hash = $1
               RETURNING user_id, expires_at AS "expires_at: jiff_sqlx::Timestamp""#,
            hash_token(token),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(link) = link else {
            return Ok(None);
        };
        // The clock of the caller decides, not the database time (ADR 0038).
        if now >= link.expires_at.to_jiff() {
            tx.commit().await.map_err(store_error)?;
            return Ok(None);
        }
        let memberships: Vec<OrganizationId> = sqlx::query_scalar!(
            "SELECT organization_id FROM organization_membership WHERE user_id = $1",
            link.user_id,
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(store_error)?
        .into_iter()
        .map(OrganizationId::from_uuid)
        .collect();
        let session = insert_session(
            &mut tx,
            UserId::from_uuid(link.user_id),
            initial_organization(&memberships),
            user_agent,
            now,
        )
        .await?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(session))
    }
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use secrecy::ExposeSecret;
    use tada_app::domain::identity::{DisplayName, OrganizationRole};
    use tada_app::outbound::OutboundStore;
    use tada_app::session::SessionStore;

    use super::*;
    use crate::testing::TestDatabase;

    fn now() -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }

    fn email(text: &str) -> Email {
        Email::parse(text).unwrap()
    }

    async fn user(test: &TestDatabase, address: &str) -> UserId {
        test.create_user(&DisplayName::parse("Anna Muster").unwrap(), &email(address))
            .await
    }

    async fn count(test: &TestDatabase, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&test.database.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn queues_a_magic_link_for_a_member_only() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        user(&test, "ben@example.org").await;

        for address in ["nobody@example.org", "ben@example.org"] {
            test.database
                .queue_magic_link(&email(address), None)
                .await
                .unwrap();
        }
        assert_eq!(count(&test, "outbound_intent").await, 0);
        assert_eq!(count(&test, "job").await, 0);

        test.database
            .queue_magic_link(&email("anna@example.org"), Some(Uuid::now_v7()))
            .await
            .unwrap();
        assert_eq!(count(&test, "outbound_intent").await, 1);
        assert_eq!(count(&test, "job").await, 1);
    }

    async fn magic_link(test: &TestDatabase, user: UserId, expires_at: Timestamp) -> String {
        test.database
            .issue_magic_link(user, expires_at)
            .await
            .unwrap()
            .expose_secret()
            .to_owned()
    }

    #[tokio::test]
    async fn a_magic_link_starts_one_session_and_is_deleted() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let token = magic_link(&test, anna, now() + SignedDuration::from_mins(15)).await;

        let session = test
            .database
            .redeem_magic_link(&token, Some("Firefox"), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(count(&test, "magic_link").await, 0);
        let row = test
            .database
            .find(session.expose_secret(), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.user_id, anna);
        assert_eq!(row.organization_id, Some(testwil));
        assert_eq!(row.created_at, now());

        let again = test
            .database
            .redeem_magic_link(&token, None, now())
            .await
            .unwrap();
        assert!(again.is_none());
        assert_eq!(count(&test, "session").await, 1);
    }

    #[tokio::test]
    async fn a_user_with_two_memberships_gets_a_session_without_an_organization() {
        let test = TestDatabase::start().await;
        let anna = user(&test, "anna@example.org").await;
        for slug in ["testwil", "musterhausen"] {
            let organization = test.create_organization(slug).await;
            test.add_membership(organization, anna, OrganizationRole::Member)
                .await;
        }
        let token = magic_link(&test, anna, now() + SignedDuration::from_mins(15)).await;

        let session = test
            .database
            .redeem_magic_link(&token, None, now())
            .await
            .unwrap()
            .unwrap();
        let row = test
            .database
            .find(session.expose_secret(), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.organization_id, None);
    }

    #[tokio::test]
    async fn an_expired_magic_link_starts_no_session_and_is_deleted() {
        let test = TestDatabase::start().await;
        let anna = user(&test, "anna@example.org").await;
        let token = magic_link(&test, anna, now()).await;

        let session = test
            .database
            .redeem_magic_link(&token, None, now())
            .await
            .unwrap();
        assert!(session.is_none());
        assert_eq!(count(&test, "magic_link").await, 0);
        assert_eq!(count(&test, "session").await, 0);
    }
}
