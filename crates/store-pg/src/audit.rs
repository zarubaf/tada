//! The audit log writer (ADR 0039).

use serde_json::{Map, Value};
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::{AuditEvent, RoleChange};
use tada_app::domain::ids::{OrganizationId, UserId};

/// The `detail` column of a role change: `{"old_role": ..., "new_role": ...}`, without the key of
/// a missing role (ADR 0061).
fn detail(roles: RoleChange) -> Value {
    let mut detail = Map::new();
    if let Some(old) = roles.old {
        detail.insert("old_role".to_owned(), old.as_str().into());
    }
    if let Some(new) = roles.new {
        detail.insert("new_role".to_owned(), new.as_str().into());
    }
    Value::Object(detail)
}

/// Adds an audit event inside the transaction of a command. A rollback removes it with the change.
pub(crate) async fn record(conn: &mut PgConnection, event: &AuditEvent) -> Result<(), sqlx::Error> {
    let actor = event.actor();
    sqlx::query!(
        "INSERT INTO audit_event
             (id, organization_id, occurred_at, actor_kind, actor_id, principal_id, channel,
              request_id, action, record_kind, record_id, subject_user_id, detail)
         VALUES ($1, $2, now(), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        Uuid::now_v7(),
        event.organization_id().map(OrganizationId::as_uuid),
        actor.kind().as_str(),
        actor.id(),
        actor.principal(),
        actor.channel().as_str(),
        actor.request_id(),
        event.action().as_str(),
        event.record_kind(),
        event.record_id(),
        event.subject().map(UserId::as_uuid),
        event.roles().map(detail),
    )
    .execute(conn)
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use tada_app::audit::{AuditAction, AuditRole};
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::identity::EventRole;
    use tada_app::domain::ids::UserId;

    use super::*;
    use crate::testing::TestDatabase;

    async fn count(test: &TestDatabase) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM audit_event")
            .fetch_one(&test.database.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn records_in_the_transaction_of_the_caller() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let user = UserId::from_uuid(Uuid::now_v7());
        let request = Uuid::now_v7();
        let record_id = Uuid::now_v7();
        let caller = MemberCaller::new(user, organization, OrganizationRole::Owner)
            .with_request(tada_app::caller::Channel::Telegram, Some(request));
        let event = AuditEvent::new(
            caller.actor(),
            AuditAction::EventCreate,
            Some(record_id),
            Some(caller.scope()),
        );

        let mut tx = test.database.pool.begin().await.unwrap();
        record(&mut tx, &event).await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(count(&test).await, 0, "a rollback removes the audit event");

        let mut tx = test.database.pool.begin().await.unwrap();
        record(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(count(&test).await, 1);
        let row: (
            String,
            Uuid,
            Option<Uuid>,
            String,
            Option<Uuid>,
            String,
            Option<Uuid>,
        ) = sqlx::query_as(
            "SELECT actor_kind, actor_id, principal_id, channel, request_id, record_kind, record_id
                 FROM audit_event",
        )
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(
            row,
            (
                "member".to_owned(),
                user.as_uuid(),
                None,
                "telegram".to_owned(),
                Some(request),
                "event".to_owned(),
                Some(record_id)
            )
        );
        let organization_id: Option<Uuid> =
            sqlx::query_scalar("SELECT organization_id FROM audit_event")
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(organization_id, Some(organization.as_uuid()));
    }

    #[tokio::test]
    async fn records_the_subject_and_the_role_change() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let caller = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        );
        let subject = UserId::from_uuid(Uuid::now_v7());
        let event = AuditEvent::new(
            caller.actor(),
            AuditAction::EventMembershipChangeRole,
            Some(Uuid::now_v7()),
            Some(caller.scope()),
        )
        .about(subject)
        .with_roles(
            Some(AuditRole::Event(EventRole::EventManager)),
            Some(AuditRole::Event(EventRole::EventViewer)),
        );
        let added = AuditEvent::new(
            caller.actor(),
            AuditAction::EventMembershipAdd,
            Some(Uuid::now_v7()),
            Some(caller.scope()),
        )
        .about(subject)
        .with_roles(None, Some(AuditRole::Event(EventRole::EventViewer)));
        let plain = AuditEvent::new(
            caller.actor(),
            AuditAction::EventCreate,
            Some(Uuid::now_v7()),
            Some(caller.scope()),
        )
        .with_roles(None, None);

        let mut tx = test.database.pool.begin().await.unwrap();
        for event in [&event, &added, &plain] {
            record(&mut tx, event).await.unwrap();
        }
        tx.commit().await.unwrap();

        let rows: Vec<(String, Option<Uuid>, Option<serde_json::Value>)> =
            sqlx::query_as("SELECT action, subject_user_id, detail FROM audit_event ORDER BY id")
                .fetch_all(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            [
                (
                    "event_membership.change_role".to_owned(),
                    Some(subject.as_uuid()),
                    Some(
                        serde_json::json!({"old_role": "event-manager", "new_role": "event-viewer"})
                    )
                ),
                (
                    "event_membership.add".to_owned(),
                    Some(subject.as_uuid()),
                    Some(serde_json::json!({"new_role": "event-viewer"}))
                ),
                ("event.create".to_owned(), None, None),
            ]
        );
    }

    /// The detail holds role names only, and the actor kind and the channel come from closed sets
    /// (ADR 0061).
    #[tokio::test]
    async fn rejects_free_text_in_an_audit_row() {
        let test = TestDatabase::start().await;
        for (actor_kind, channel, detail) in [
            ("member", "web", r#"{"note": "free text"}"#),
            ("member", "web", r#"{"old_role": "chief"}"#),
            ("member", "web", "{}"),
            ("member", "web", r#"{"old_role": null}"#),
            (
                "member",
                "web",
                r#"{"old_role": "member", "new_role": null}"#,
            ),
            ("robot", "web", "null"),
            ("member", "fax", "null"),
        ] {
            let error = sqlx::query(
                "INSERT INTO audit_event
                     (id, occurred_at, actor_kind, actor_id, channel, action, record_kind, detail)
                 VALUES ($1, now(), $2, $1, $3, 'event.create', 'event', $4::jsonb)",
            )
            .bind(Uuid::now_v7())
            .bind(actor_kind)
            .bind(channel)
            .bind(detail)
            .execute(&test.database.pool)
            .await
            .unwrap_err();
            assert_eq!(crate::testing::sqlstate(&error), "23514", "{detail}");
        }
    }
}
