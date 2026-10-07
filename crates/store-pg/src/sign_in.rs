//! The `SignInStore` adapter (ADR 0008, ADR 0056).
//!
//! Sign-in has no organization yet, so its queries are infrastructure queries without a scope
//! (ADR 0039). A magic link belongs to a user, not to an organization.

use async_trait::async_trait;
use jiff::Timestamp;
use secrecy::SecretString;
use serde_json::json;
use sqlx::PgExecutor;
use sqlx::types::{Json, Uuid};
use tada_app::domain::identity::Email;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::outbound::SEND_JOB;
use tada_app::rate_limit::{RateDecision, RateLimit};
use tada_app::sign_in::{SignInRequestStore, SignInStore, initial_organization};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;
use crate::rate_limit::PgRateLimiter;
use crate::session::insert_session;
use crate::token::hash_token;

/// Queues a magic-link intent for the user of `email` if that user has a membership.
///
/// One statement for both cases, so a member and an unknown address cost one round trip each
/// (ADR 0008). It writes the rows of `queue_outbound` for a magic link: the intent and its send
/// job, whose payload names the intent only (ADR 0042).
async fn queue_magic_link_intent(
    executor: impl PgExecutor<'_>,
    email: &Email,
    request_id: Option<Uuid>,
) -> Result<(), StoreError> {
    let intent_id = Uuid::now_v7();
    sqlx::query!(
        "WITH member AS (
             SELECT e.user_id FROM email_identity e
             WHERE e.email = $1
               AND EXISTS (SELECT 1 FROM organization_membership m WHERE m.user_id = e.user_id)
         ), intent AS (
             INSERT INTO outbound_intent (id, user_id, purpose, message_id, request_id, created_at)
             SELECT $2, user_id, 'magic-link', $3, $4, now() FROM member
             RETURNING id
         )
         INSERT INTO job (id, kind, version, payload, request_id, run_at, created_at)
         SELECT $5, $6, 1, $7, $4, now(), now() FROM intent",
        email.as_str(),
        intent_id,
        intent_id.simple().to_string(),
        request_id,
        Uuid::now_v7(),
        SEND_JOB,
        Json(json!({"intent_id": intent_id})) as _,
    )
    .execute(executor)
    .await
    .map_err(store_error)?;
    Ok(())
}

/// The `SignInRequestStore` adapter: the rate limits and the magic-link intent in one transaction.
#[derive(Debug)]
pub struct PgSignInRequestStore {
    database: Database,
    rate_limiter: PgRateLimiter,
}

impl PgSignInRequestStore {
    pub fn new(database: Database, rate_limiter: PgRateLimiter) -> Self {
        Self {
            database,
            rate_limiter,
        }
    }
}

#[async_trait]
impl SignInRequestStore for PgSignInRequestStore {
    async fn queue_magic_link(
        &self,
        email: &Email,
        limits: &[RateLimit<'_>],
        request_id: Option<Uuid>,
        now: Timestamp,
    ) -> Result<RateDecision, StoreError> {
        let mut tx = self.database.pool.begin().await.map_err(store_error)?;
        let decision = self.rate_limiter.hit(&mut tx, limits, now).await?;
        if decision == RateDecision::Allowed {
            queue_magic_link_intent(&mut *tx, email, request_id).await?;
        }
        // The counters are written in each case, so each request ends with one write commit.
        tx.commit().await.map_err(store_error)?;
        Ok(decision)
    }
}

#[async_trait]
impl SignInStore for Database {
    async fn queue_magic_link(
        &self,
        email: &Email,
        request_id: Option<Uuid>,
    ) -> Result<(), StoreError> {
        queue_magic_link_intent(&self.pool, email, request_id).await
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
        // A person without a membership cannot sign in (ADR 0056). The link is used up all the same.
        if memberships.is_empty() {
            tx.commit().await.map_err(store_error)?;
            return Ok(None);
        }
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
    use tada_app::outbound::{OutboundStore, Purpose};
    use tada_app::rate_limit::{SIGN_IN_PER_EMAIL, sign_in_limits};
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

        let request = Uuid::now_v7();
        test.database
            .queue_magic_link(&email("anna@example.org"), Some(request))
            .await
            .unwrap();
        assert_eq!(count(&test, "outbound_intent").await, 1);

        // The job is the one that `queue_outbound` writes: it names the intent only (ADR 0042).
        let (kind, version, payload, job_request): (String, i32, serde_json::Value, Option<Uuid>) =
            sqlx::query_as("SELECT kind, version, payload, request_id FROM job")
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!((kind.as_str(), version), (SEND_JOB, 1));
        assert_eq!(job_request, Some(request));
        let intent_id: Uuid = payload["intent_id"].as_str().unwrap().parse().unwrap();
        assert_eq!(payload, json!({"intent_id": intent_id}));
        let pending = test
            .database
            .load_pending(intent_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pending.purpose, Purpose::MagicLink { user_id: anna });
        assert_eq!(pending.to, email("anna@example.org"));
    }

    fn request_store(test: &TestDatabase) -> PgSignInRequestStore {
        PgSignInRequestStore::new(
            test.database.clone(),
            PgRateLimiter::new(SecretString::from("test rate limit key")),
        )
    }

    #[tokio::test]
    async fn a_request_counts_and_queues_in_one_transaction() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let store = request_store(&test);
        let ip = "203.0.113.7".parse().unwrap();

        for address in ["nobody@example.org", "anna@example.org"] {
            let email = email(address);
            let decision = store
                .queue_magic_link(&email, &sign_in_limits(&email, ip), None, now())
                .await
                .unwrap();
            assert_eq!(decision, RateDecision::Allowed);
        }
        assert_eq!(count(&test, "outbound_intent").await, 1);
        assert_eq!(count(&test, "rate_limit_counter").await, 3);
    }

    #[tokio::test]
    async fn a_limited_request_queues_nothing() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let store = request_store(&test);
        let anna = email("anna@example.org");
        let limits = sign_in_limits(&anna, "203.0.113.7".parse().unwrap());

        for _ in 0..SIGN_IN_PER_EMAIL {
            store
                .queue_magic_link(&anna, &limits, None, now())
                .await
                .unwrap();
        }
        let decision = store
            .queue_magic_link(&anna, &limits, None, now())
            .await
            .unwrap();
        assert!(matches!(decision, RateDecision::Limited { .. }));
        assert_eq!(
            count(&test, "outbound_intent").await,
            i64::from(SIGN_IN_PER_EMAIL)
        );
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
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
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

    #[tokio::test]
    async fn a_user_without_a_membership_gets_no_session_and_the_link_is_used_up() {
        let test = TestDatabase::start().await;
        // The membership ended after the mail went out.
        let anna = user(&test, "anna@example.org").await;
        let token = magic_link(&test, anna, now() + SignedDuration::from_mins(15)).await;

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
