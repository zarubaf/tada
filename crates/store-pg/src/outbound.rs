//! Outbound intents and the tokens of magic links and invitations (ADRs 0008 and 0042).
//!
//! A magic-link intent belongs to a user, not to an organization.
//! Its queries are infrastructure queries without a scope (ADR 0039).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use secrecy::SecretString;
use serde_json::json;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::identity::Email;
use tada_app::domain::ids::{InvitationId, OrganizationId, UserId};
use tada_app::jobs::NewJob;
use tada_app::outbound::{OutboundStore, Outcome, PendingIntent, Purpose, SEND_JOB};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::jobs::enqueue;
use crate::token::new_token;

/// Stores an outbound intent and its send job inside the transaction of a command.
/// A rollback removes both, so a rolled-back command never sends mail (ADR 0042).
// The sign-in request and the invitation commands call this in Slice 1.
#[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
pub(crate) async fn queue_outbound(
    conn: &mut PgConnection,
    purpose: &Purpose,
    request_id: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    let (kind, user_id, invitation_id) = match *purpose {
        Purpose::MagicLink { user_id } => ("magic-link", Some(user_id.as_uuid()), None),
        Purpose::Invitation { invitation_id, .. } => {
            ("invitation", None, Some(invitation_id.as_uuid()))
        }
    };
    let organization_id = purpose.organization_id();
    sqlx::query!(
        "INSERT INTO outbound_intent
             (id, organization_id, user_id, invitation_id, purpose, message_id, request_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
        id,
        organization_id.map(OrganizationId::as_uuid),
        user_id,
        invitation_id,
        kind,
        id.simple().to_string(),
        request_id,
    )
    .execute(&mut *conn)
    .await?;
    enqueue(
        conn,
        &NewJob {
            kind: SEND_JOB,
            version: 1,
            payload: json!({"intent_id": id}),
            organization_id,
            request_id,
            run_at: None,
        },
    )
    .await?;
    Ok(id)
}

#[async_trait]
impl OutboundStore for Database {
    async fn load_pending(&self, intent_id: Uuid) -> Result<Option<PendingIntent>, StoreError> {
        // An infrastructure query: the purpose gives the organization of an invitation (ADR 0039).
        // An invitation goes out in the locale of the user of its address, if that user exists.
        let row = sqlx::query!(
            r#"SELECT i.organization_id, i.user_id, i.invitation_id, i.message_id,
                      coalesce(e.email, inv.email) AS "email!",
                      coalesce(u.locale, invited.locale, 'de-CH') AS "locale!",
                      o.name AS "organization_name?"
               FROM outbound_intent i
               LEFT JOIN email_identity e ON e.user_id = i.user_id
               LEFT JOIN app_user u ON u.id = i.user_id
               LEFT JOIN invitation inv
                   ON inv.organization_id = i.organization_id AND inv.id = i.invitation_id
               LEFT JOIN organization o ON o.id = inv.organization_id
               LEFT JOIN email_identity invited_email ON invited_email.email = inv.email
               LEFT JOIN app_user invited ON invited.id = invited_email.user_id
               WHERE i.id = $1 AND i.status = 'pending'"#,
            intent_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let purpose = match (row.user_id, row.organization_id, row.invitation_id) {
            (Some(user_id), None, None) => Purpose::MagicLink {
                user_id: UserId::from_uuid(user_id),
            },
            (None, Some(organization_id), Some(invitation_id)) => Purpose::Invitation {
                organization_id: OrganizationId::from_uuid(organization_id),
                invitation_id: InvitationId::from_uuid(invitation_id),
            },
            _ => return Err(InvalidRow("purpose").into()),
        };
        if matches!(purpose, Purpose::Invitation { .. }) != row.organization_name.is_some() {
            return Err(InvalidRow("organization_id").into());
        }
        Ok(Some(PendingIntent {
            purpose,
            to: Email::parse(&row.email).map_err(|_| InvalidRow("email"))?,
            locale: row.locale,
            organization_name: row.organization_name,
            message_id: row.message_id,
        }))
    }

    async fn issue_magic_link(
        &self,
        user_id: UserId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError> {
        let token = new_token("")?;
        sqlx::query!(
            "INSERT INTO magic_link (token_hash, user_id, expires_at, created_at)
             VALUES ($1, $2, $3, now())",
            token.hash,
            user_id.as_uuid(),
            expires_at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(token.secret)
    }

    async fn issue_invitation_token(
        &self,
        scope: OrgScope,
        invitation_id: InvitationId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError> {
        let token = new_token("")?;
        sqlx::query!(
            "INSERT INTO invitation_token (token_hash, organization_id, invitation_id, expires_at)
             VALUES ($1, $2, $3, $4)",
            token.hash,
            scope.organization_id().as_uuid(),
            invitation_id.as_uuid(),
            expires_at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(token.secret)
    }

    async fn finish(&self, intent_id: Uuid, outcome: Outcome) -> Result<(), StoreError> {
        // An infrastructure query by intent ID. Only a pending intent changes.
        sqlx::query!(
            "UPDATE outbound_intent SET status = $2, finished_at = now()
             WHERE id = $1 AND status = 'pending'",
            intent_id,
            outcome.as_str(),
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::{SignedDuration, Timestamp};
    use secrecy::ExposeSecret;
    use sqlx::types::Uuid;
    use tada_app::caller::MemberCaller;
    use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
    use tada_app::domain::ids::{InvitationId, OrganizationId, UserId};
    use tada_app::outbound::{OutboundStore, Outcome, Purpose, SEND_JOB};

    use super::*;
    use crate::testing::TestDatabase;

    fn email(text: &str) -> Email {
        Email::parse(text).unwrap()
    }

    async fn user(test: &TestDatabase) -> UserId {
        test.create_user(
            &DisplayName::parse("Anna Muster").unwrap(),
            &email("anna@example.org"),
        )
        .await
    }

    async fn invitation(test: &TestDatabase, organization: OrganizationId) -> InvitationId {
        let id = Uuid::now_v7();
        sqlx::query!(
            "INSERT INTO invitation (id, organization_id, email, display_name, role, created_at)
             VALUES ($1, $2, 'berta@example.org', 'Berta Beispiel', 'member', now())",
            id,
            organization.as_uuid(),
        )
        .execute(&test.database.pool)
        .await
        .unwrap();
        InvitationId::from_uuid(id)
    }

    async fn queue(test: &TestDatabase, purpose: &Purpose) -> Result<Uuid, sqlx::Error> {
        let mut tx = test.database.pool.begin().await.unwrap();
        let id = queue_outbound(&mut tx, purpose, Some(Uuid::now_v7())).await?;
        tx.commit().await.unwrap();
        Ok(id)
    }

    fn later() -> Timestamp {
        Timestamp::now() + SignedDuration::from_mins(15)
    }

    #[tokio::test]
    async fn loads_a_pending_magic_link_with_the_address_and_the_locale_of_the_user() {
        let test = TestDatabase::start().await;
        let user_id = user(&test).await;
        let purpose = Purpose::MagicLink { user_id };
        let id = queue(&test, &purpose).await.unwrap();

        let pending = test.database.load_pending(id).await.unwrap().unwrap();
        assert_eq!(pending.purpose, purpose);
        assert_eq!(pending.to, email("anna@example.org"));
        assert_eq!(pending.locale, "de-CH");
        assert_eq!(pending.organization_name, None);
        assert!(!pending.message_id.is_empty());
    }

    #[tokio::test]
    async fn loads_a_pending_invitation_with_the_organization_name() {
        let test = TestDatabase::start().await;
        let organization_id = test.create_organization("testwil").await;
        let invitation_id = invitation(&test, organization_id).await;
        let purpose = Purpose::Invitation {
            organization_id,
            invitation_id,
        };
        let id = queue(&test, &purpose).await.unwrap();

        let pending = test.database.load_pending(id).await.unwrap().unwrap();
        assert_eq!(pending.purpose, purpose);
        assert_eq!(pending.to, email("berta@example.org"));
        assert_eq!(pending.organization_name.as_deref(), Some("testwil"));
    }

    #[tokio::test]
    async fn a_finished_intent_is_no_longer_pending() {
        let test = TestDatabase::start().await;
        let user_id = user(&test).await;
        let id = queue(&test, &Purpose::MagicLink { user_id }).await.unwrap();

        test.database.finish(id, Outcome::Unknown).await.unwrap();
        assert_eq!(test.database.load_pending(id).await.unwrap(), None);
        let status: String = sqlx::query_scalar("SELECT status FROM outbound_intent WHERE id = $1")
            .bind(id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(status, "unknown");
    }

    #[tokio::test]
    async fn the_job_payload_names_the_intent_only() {
        let test = TestDatabase::start().await;
        let user_id = user(&test).await;
        let id = queue(&test, &Purpose::MagicLink { user_id }).await.unwrap();

        let (kind, version, payload): (String, i32, serde_json::Value) =
            sqlx::query_as("SELECT kind, version, payload FROM job")
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!((kind.as_str(), version), (SEND_JOB, 1));
        assert_eq!(payload, serde_json::json!({"intent_id": id}));
        assert!(!payload.to_string().contains("example.org"));
    }

    #[tokio::test]
    async fn a_magic_link_stores_only_the_hash_of_its_token() {
        let test = TestDatabase::start().await;
        let user_id = user(&test).await;
        queue(&test, &Purpose::MagicLink { user_id }).await.unwrap();

        let token = test
            .database
            .issue_magic_link(user_id, later())
            .await
            .unwrap();
        let hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM magic_link")
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(hash.len(), 32);
        test.assert_no_plaintext(token.expose_secret()).await;
    }

    #[tokio::test]
    async fn an_invitation_token_stays_in_the_organization_of_the_scope() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let invitation_id = invitation(&test, testwil).await;
        let scope = |organization| {
            MemberCaller::new(
                UserId::from_uuid(Uuid::now_v7()),
                organization,
                OrganizationRole::Owner,
            )
            .scope()
        };

        assert!(
            test.database
                .issue_invitation_token(scope(musterhausen), invitation_id, later())
                .await
                .is_err()
        );
        let token = test
            .database
            .issue_invitation_token(scope(testwil), invitation_id, later())
            .await
            .unwrap();
        test.assert_no_plaintext(token.expose_secret()).await;
    }

    #[tokio::test]
    async fn an_invitation_intent_needs_an_invitation_of_its_organization() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let invitation_id = invitation(&test, testwil).await;

        let other = queue(
            &test,
            &Purpose::Invitation {
                organization_id: musterhausen,
                invitation_id,
            },
        )
        .await;
        assert!(other.is_err(), "the composite foreign key rejects it");

        let without_organization = sqlx::query(
            "INSERT INTO outbound_intent (id, invitation_id, purpose, message_id, created_at)
             VALUES ($1, $2, 'invitation', 'x', now())",
        )
        .bind(Uuid::now_v7())
        .bind(invitation_id.as_uuid())
        .execute(&test.database.pool)
        .await;
        assert!(without_organization.is_err(), "the CHECK rejects it");
    }

    #[tokio::test]
    async fn finds_a_secret_in_any_text_column() {
        let test = TestDatabase::start().await;
        test.create_organization("geheimnis-im-klartext").await;
        let found = tokio::spawn(async move {
            test.assert_no_plaintext("geheimnis-im-klartext").await;
        })
        .await;
        assert!(found.is_err(), "assert_no_plaintext must fail");
    }
}
