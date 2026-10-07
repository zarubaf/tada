//! The audit log writer (ADR 0039).

use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::domain::ids::OrganizationId;

/// Adds an audit event inside the transaction of a command. A rollback removes it with the change.
pub(crate) async fn record(conn: &mut PgConnection, event: &AuditEvent) -> Result<(), sqlx::Error> {
    let actor = event.actor();
    sqlx::query!(
        "INSERT INTO audit_event
             (id, organization_id, occurred_at, actor_kind, actor_id, principal_id, channel,
              request_id, action, record_kind, record_id)
         VALUES ($1, $2, now(), $3, $4, $5, $6, $7, $8, $9, $10)",
        Uuid::now_v7(),
        event.organization_id().map(OrganizationId::as_uuid),
        actor.kind().as_str(),
        actor.id(),
        actor.principal(),
        actor.channel().as_str(),
        actor.request_id(),
        event.action(),
        event.record_kind(),
        event.record_id(),
    )
    .execute(conn)
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use tada_app::caller::{MemberCaller, OrganizationRole};
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
        let actor = MemberCaller::new(user, organization, OrganizationRole::Owner)
            .with_request(tada_app::caller::Channel::Telegram, Some(request))
            .actor();
        let event = AuditEvent::new(
            actor,
            "event.create",
            "event",
            Some(record_id),
            Some(organization),
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
    }
}
