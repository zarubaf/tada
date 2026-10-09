//! The `SignInStore` adapter (ADR 0008, ADR 0056).
//!
//! Sign-in has no organization yet, so its queries are infrastructure queries without a scope
//! (ADR 0039). A magic link belongs to a user, not to an organization.
//! An invitation token names its organization, so the token gives the scope of an acceptance.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::SecretString;
use serde_json::json;
use sqlx::types::{Json, Uuid};
use sqlx::{PgConnection, PgExecutor};
use tada_app::audit::AuditEvent;
use tada_app::domain::identity::{Email, OrganizationRole};
use tada_app::domain::ids::{InvitationId, OrganizationId, UserId};
use tada_app::outbound::SEND_JOB;
use tada_app::rate_limit::{RateDecision, SignInLimits};
use tada_app::sign_in::{
    InvitationPreview, SignInRequestStore, SignInStore, accepted_role, initial_organization,
};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit::record;
use crate::error::store_error;
use crate::identity::organization_role;
use crate::rate_limit::{PgRateLimiter, delete_ended_counters};
use crate::session::insert_session;
use crate::token::hash_token;

/// Queues a magic-link intent for the user of `email` if that user has a membership.
///
/// One statement for both cases, so a member and an unknown address cost the same (ADR 0008).
/// It writes the rows of `queue_outbound` for a magic link: the intent and its send
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
/// A request that the client limit refuses does not count against the mail limit, so it cannot
/// take the one mail of a cooldown without a mail.
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
        limits: &SignInLimits<'_>,
        request_id: Option<Uuid>,
        now: Timestamp,
    ) -> Result<RateDecision, StoreError> {
        let mut tx = self.database.pool.begin().await.map_err(store_error)?;
        delete_ended_counters(&mut tx, now).await?;
        let decision = self.rate_limiter.hit(&mut tx, &limits.client, now).await?;
        if decision == RateDecision::Allowed
            && self.rate_limiter.hit(&mut tx, &limits.mail, now).await? == RateDecision::Allowed
        {
            queue_magic_link_intent(&mut *tx, email, request_id).await?;
        }
        // The counters are written in each case, so each request ends with one write commit.
        tx.commit().await.map_err(store_error)?;
        Ok(decision)
    }
}

#[async_trait]
impl SignInStore for Database {
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

    async fn preview_invitation(
        &self,
        token: &str,
        now: Timestamp,
    ) -> Result<Option<InvitationPreview>, StoreError> {
        let row = sqlx::query!(
            "SELECT o.name AS organization_name, o.privacy_notice, i.role
             FROM invitation_token t
             JOIN invitation i ON i.organization_id = t.organization_id AND i.id = t.invitation_id
             JOIN organization o ON o.id = i.organization_id
             WHERE t.token_hash = $1 AND t.expires_at > $2 AND i.status = 'pending'",
            hash_token(token),
            now.to_sqlx() as _,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        row.map(|row| {
            Ok(InvitationPreview {
                organization_name: row.organization_name,
                privacy_notice: row.privacy_notice,
                role: organization_role(&row.role)?,
            })
        })
        .transpose()
    }

    async fn accept_invitation(
        &self,
        token: &str,
        user_agent: Option<&str>,
        request_id: Option<Uuid>,
        now: Timestamp,
    ) -> Result<Option<SecretString>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The lock on the invitation serializes two acceptances, also with two different tokens of
        // one invitation. The second one then sees the status `accepted` and finds nothing.
        // The clock of the caller decides, not the database time (ADR 0038).
        let invitation = sqlx::query!(
            "SELECT i.id, i.organization_id, i.email, i.display_name, i.role
             FROM invitation_token t
             JOIN invitation i ON i.organization_id = t.organization_id AND i.id = t.invitation_id
             WHERE t.token_hash = $1 AND t.expires_at > $2 AND i.status = 'pending'
             FOR UPDATE OF i",
            hash_token(token),
            now.to_sqlx() as _,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(invitation) = invitation else {
            return Ok(None);
        };
        let organization_id = OrganizationId::from_uuid(invitation.organization_id);
        let user_id =
            find_or_create_user(&mut tx, &invitation.email, &invitation.display_name, now).await?;
        let (existing, accepted) = add_membership(
            &mut tx,
            organization_id,
            user_id,
            organization_role(&invitation.role)?,
            now,
        )
        .await?;

        sqlx::query!(
            "UPDATE invitation SET status = 'accepted', accepted_at = $3
             WHERE organization_id = $1 AND id = $2",
            organization_id.as_uuid(),
            invitation.id,
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        // All tokens of the invitation stop working, also a printed one (ADR 0036).
        sqlx::query!(
            "DELETE FROM invitation_token WHERE organization_id = $1 AND invitation_id = $2",
            organization_id.as_uuid(),
            invitation.id,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;

        let event = AuditEvent::by_invitee(
            user_id,
            organization_id,
            InvitationId::from_uuid(invitation.id),
            request_id,
            existing,
            accepted,
        );
        record(&mut tx, &event).await.map_err(store_error)?;

        // The session starts in the organization of the invitation (ADR 0056).
        let session =
            insert_session(&mut tx, user_id, Some(organization_id), user_agent, now).await?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(session))
    }
}

/// The user of `email`, or a new user with `display_name` if no user has this address.
/// One address belongs to one user (ADR 0056).
///
/// Known limit: two acceptances for one new address at the same moment both find no user.
/// The UNIQUE email constraint then fails the second transaction, which rolls back with a 500.
/// It never makes a second user for the address.
async fn find_or_create_user(
    conn: &mut PgConnection,
    email: &str,
    display_name: &str,
    now: Timestamp,
) -> Result<UserId, StoreError> {
    let existing =
        sqlx::query_scalar!("SELECT user_id FROM email_identity WHERE email = $1", email,)
            .fetch_optional(&mut *conn)
            .await
            .map_err(store_error)?;
    if let Some(id) = existing {
        return Ok(UserId::from_uuid(id));
    }
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO app_user (id, display_name, created_at) VALUES ($1, $2, $3)",
        id,
        display_name,
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await
    .map_err(store_error)?;
    sqlx::query!(
        "INSERT INTO email_identity (user_id, email, created_at) VALUES ($1, $2, $3)",
        id,
        email,
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await
    .map_err(store_error)?;
    Ok(UserId::from_uuid(id))
}

/// Adds the membership of an invitee, or raises the role of an existing one to `accepted_role`.
/// Returns the role before, if the membership existed, and the role after.
///
/// Known limit: two acceptances that add the first membership of one user in one organization at
/// the same moment both find none. The primary key then fails the second transaction, which rolls
/// back with a 500. It never makes a second membership.
async fn add_membership(
    conn: &mut PgConnection,
    organization_id: OrganizationId,
    user_id: UserId,
    invited: OrganizationRole,
    now: Timestamp,
) -> Result<(Option<OrganizationRole>, OrganizationRole), StoreError> {
    let existing = sqlx::query_scalar!(
        "SELECT role FROM organization_membership
         WHERE organization_id = $1 AND user_id = $2
         FOR UPDATE",
        organization_id.as_uuid(),
        user_id.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_error)?
    .as_deref()
    .map(organization_role)
    .transpose()?;
    let accepted = accepted_role(existing, invited);
    match existing {
        None => {
            sqlx::query!(
                "INSERT INTO organization_membership (organization_id, user_id, role, created_at)
                 VALUES ($1, $2, $3, $4)",
                organization_id.as_uuid(),
                user_id.as_uuid(),
                accepted.as_str(),
                now.to_sqlx() as _,
            )
            .execute(&mut *conn)
            .await
            .map_err(store_error)?;
        }
        Some(existing) if existing != accepted => {
            sqlx::query!(
                "UPDATE organization_membership SET role = $3, version = version + 1
                 WHERE organization_id = $1 AND user_id = $2",
                organization_id.as_uuid(),
                user_id.as_uuid(),
                accepted.as_str(),
            )
            .execute(&mut *conn)
            .await
            .map_err(store_error)?;
        }
        Some(_) => {}
    }
    Ok((existing, accepted))
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use secrecy::ExposeSecret;
    use tada_app::caller::MemberCaller;
    use tada_app::domain::identity::{DisplayName, OrganizationRole};
    use tada_app::outbound::{OutboundStore, Purpose};
    use tada_app::rate_limit::{MAIL_COOLDOWN, SIGN_IN_PER_IP, sign_in_limits};
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

    fn request_store(test: &TestDatabase) -> PgSignInRequestStore {
        PgSignInRequestStore::new(
            test.database.clone(),
            PgRateLimiter::new(SecretString::from("test rate limit key")),
        )
    }

    /// A sign-in request for `address` that the limits allow.
    async fn queue(test: &TestDatabase, address: &str, request_id: Option<Uuid>) {
        let email = email(address);
        let limits = sign_in_limits(&email, "203.0.113.7".parse().unwrap());
        let decision = request_store(test)
            .queue_magic_link(&email, &limits, request_id, now())
            .await
            .unwrap();
        assert_eq!(decision, RateDecision::Allowed);
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
            queue(&test, address, None).await;
        }
        assert_eq!(count(&test, "outbound_intent").await, 0);
        assert_eq!(count(&test, "job").await, 0);

        let request = Uuid::now_v7();
        queue(&test, "anna@example.org", Some(request)).await;
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

    #[tokio::test]
    async fn a_request_counts_and_queues_in_one_transaction() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        for address in ["nobody@example.org", "anna@example.org"] {
            queue(&test, address, None).await;
        }
        assert_eq!(count(&test, "outbound_intent").await, 1);
        assert_eq!(count(&test, "rate_limit_counter").await, 3);
    }

    #[tokio::test]
    async fn the_mail_limit_stops_the_mail_but_not_the_request() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let store = request_store(&test);
        let anna = email("anna@example.org");

        for client in ["203.0.113.7", "198.51.100.1", "198.51.100.2"] {
            let limits = sign_in_limits(&anna, client.parse().unwrap());
            let decision = store
                .queue_magic_link(&anna, &limits, None, now())
                .await
                .unwrap();
            assert_eq!(decision, RateDecision::Allowed, "{client}");
        }
        assert_eq!(count(&test, "outbound_intent").await, 1);

        let limits = sign_in_limits(&anna, "203.0.113.7".parse().unwrap());
        store
            .queue_magic_link(&anna, &limits, None, now() + MAIL_COOLDOWN)
            .await
            .unwrap();
        assert_eq!(
            count(&test, "outbound_intent").await,
            2,
            "the next cooldown"
        );
    }

    /// A refused request does not take the one mail of the cooldown, so a flood from one client
    /// cannot stop the mail of a request from another client.
    #[tokio::test]
    async fn a_request_that_the_client_limit_refuses_queues_nothing_and_leaves_the_mail() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = user(&test, "anna@example.org").await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let store = request_store(&test);
        let anna = email("anna@example.org");
        let flooding = "198.51.100.66".parse().unwrap();
        for n in 0..SIGN_IN_PER_IP {
            let other = email(&format!("person{n}@example.org"));
            store
                .queue_magic_link(&other, &sign_in_limits(&other, flooding), None, now())
                .await
                .unwrap();
        }
        let decision = store
            .queue_magic_link(&anna, &sign_in_limits(&anna, flooding), None, now())
            .await
            .unwrap();
        assert!(matches!(decision, RateDecision::Limited { .. }));
        assert_eq!(count(&test, "outbound_intent").await, 0);

        let limits = sign_in_limits(&anna, "203.0.113.7".parse().unwrap());
        let decision = store
            .queue_magic_link(&anna, &limits, None, now())
            .await
            .unwrap();
        assert_eq!(decision, RateDecision::Allowed);
        assert_eq!(count(&test, "outbound_intent").await, 1);
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

    async fn invitation(test: &TestDatabase, organization: OrganizationId) -> InvitationId {
        test.queue_invitation(
            organization,
            &email("berta@example.org"),
            &DisplayName::parse("Berta Beispiel").unwrap(),
            OrganizationRole::Member,
        )
        .await
    }

    async fn invitation_token(
        test: &TestDatabase,
        organization: OrganizationId,
        invitation: InvitationId,
        expires_at: Timestamp,
    ) -> String {
        let scope = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        )
        .scope();
        test.database
            .issue_invitation_token(scope, invitation, expires_at)
            .await
            .unwrap()
            .unwrap()
            .expose_secret()
            .to_owned()
    }

    #[tokio::test]
    async fn an_invitation_that_is_not_pending_cannot_be_previewed_or_accepted() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let id = invitation(&test, testwil).await;
        let token =
            invitation_token(&test, testwil, id, now() + SignedDuration::from_mins(1)).await;
        sqlx::query("UPDATE invitation SET status = 'revoked', revoked_at = now() WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();

        let preview = test
            .database
            .preview_invitation(&token, now())
            .await
            .unwrap();
        assert!(preview.is_none());
        let session = test
            .database
            .accept_invitation(&token, None, None, now())
            .await
            .unwrap();
        assert!(session.is_none());
        assert_eq!(count(&test, "app_user").await, 0);
        assert_eq!(count(&test, "session").await, 0);
    }

    #[tokio::test]
    async fn an_invitation_token_is_valid_until_its_expiry_by_the_clock_of_the_caller() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let id = invitation(&test, testwil).await;
        let token = invitation_token(&test, testwil, id, now()).await;

        let before = now() - SignedDuration::from_secs(1);
        let preview = test
            .database
            .preview_invitation(&token, before)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(preview.organization_name, "testwil");
        assert_eq!(preview.role, OrganizationRole::Member);
        assert!(
            test.database
                .preview_invitation(&token, now())
                .await
                .unwrap()
                .is_none()
        );
        let session = test
            .database
            .accept_invitation(&token, None, None, now())
            .await
            .unwrap();
        assert!(session.is_none());
        assert_eq!(count(&test, "organization_membership").await, 0);
    }

    #[tokio::test]
    async fn an_acceptance_deletes_all_tokens_of_the_invitation_and_starts_a_session() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let id = invitation(&test, testwil).await;
        let expires_at = now() + SignedDuration::from_mins(1);
        let mailed = invitation_token(&test, testwil, id, expires_at).await;
        let printed = invitation_token(&test, testwil, id, expires_at).await;

        let request = Uuid::now_v7();
        let session = test
            .database
            .accept_invitation(&mailed, Some("Firefox"), Some(request), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(count(&test, "invitation_token").await, 0);
        let row = test
            .database
            .find(session.expose_secret(), now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.organization_id, Some(testwil));
        assert_eq!(row.created_at, now());
        let (status, accepted_at): (String, jiff_sqlx::Timestamp) =
            sqlx::query_as("SELECT status, accepted_at FROM invitation WHERE id = $1")
                .bind(id.as_uuid())
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(
            (status.as_str(), accepted_at.to_jiff()),
            ("accepted", now())
        );
        let audit: (String, Uuid, Option<Uuid>) = sqlx::query_as(
            "SELECT action, actor_id, request_id FROM audit_event WHERE record_id = $1",
        )
        .bind(id.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(
            audit,
            (
                "invitation.accept".into(),
                row.user_id.as_uuid(),
                Some(request)
            )
        );

        let again = test
            .database
            .accept_invitation(&printed, None, None, now())
            .await
            .unwrap();
        assert!(again.is_none(), "the printed token stopped working");
        assert_eq!(count(&test, "session").await, 1);
    }
}
