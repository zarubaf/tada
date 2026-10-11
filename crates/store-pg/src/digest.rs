//! The `DigestStore` and `DigestLog` adapters: the send of a planned digest, its outcome and the notices, and
//! the list of the last digests (ADR 0076).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use serde_json::json;
use tada_app::caller::OrgScope;
use tada_app::domain::ids::{DigestItemId, EventId, UserId};
use tada_app::domain::reminders::{Reason, RecordKind};
use tada_app::jobs::NewJob;
use tada_app::pm::SEND_DIGEST_JOB;
use tada_app::pm::digests::{DigestLog, DigestStatus, DigestSummary};
use tada_app::pm::send::{DigestItemRef, DigestOutcome, DigestStore, ItemNow, PendingDigest};
use tada_app::store::StoreError;
use tada_app::telegram::TelegramUserId;
use uuid::Uuid;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::jobs::enqueue;
use crate::pm::{kind, version};

#[async_trait]
impl DigestStore for Database {
    async fn load(
        &self,
        scope: OrgScope,
        intent: Uuid,
    ) -> Result<Option<PendingDigest>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let Some(row) = sqlx::query!(
            r#"SELECT i.user_id AS "user_id!", u.locale, t.telegram_user_id AS "telegram_user_id?"
               FROM outbound_intent i
               JOIN app_user u ON u.id = i.user_id
               LEFT JOIN telegram_identity t ON t.user_id = i.user_id
               WHERE i.id = $2 AND i.organization_id = $1 AND i.purpose = 'telegram-digest'
                 AND i.status = 'pending'"#,
            organization,
            intent,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?
        else {
            return Ok(None);
        };
        let items = sqlx::query!(
            "SELECT id, record_kind, record_id, record_version, reason FROM digest_item
             WHERE intent_id = $2 AND organization_id = $1
             ORDER BY position",
            organization,
            intent,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?
        .into_iter()
        .map(|item| {
            Ok(DigestItemRef {
                id: DigestItemId::from_uuid(item.id),
                kind: kind(&item.record_kind, "digest_item.record_kind")?,
                record_id: item.record_id,
                version: version(item.record_version, "digest_item.record_version")?,
                reason: Reason::parse(&item.reason).ok_or(InvalidRow("digest_item.reason"))?,
            })
        })
        .collect::<Result<Vec<_>, InvalidRow>>()?;
        Ok(Some(PendingDigest {
            user_id: UserId::from_uuid(row.user_id),
            telegram_user: row.telegram_user_id.map(TelegramUserId),
            locale: row.locale,
            items,
        }))
    }

    async fn current(
        &self,
        scope: OrgScope,
        items: &[DigestItemRef],
    ) -> Result<Vec<ItemNow>, StoreError> {
        let of = |wanted: RecordKind| -> Vec<Uuid> {
            items
                .iter()
                .filter(|item| item.kind == wanted)
                .map(|item| item.record_id)
                .collect()
        };
        // An event can lose its name only by a legal redaction, and only a closed event is redacted.
        let rows = sqlx::query!(
            r#"SELECT 'action' AS "kind!", a.id AS "id!", a.event_id AS "event_id!", a.version AS "version!",
                      a.owner_user_id AS "owner!", a.title AS "title!", coalesce(e.name, '') AS "event_name!",
                      a.due_date AS "due_date: jiff_sqlx::Date", a.status AS "status!",
                      (a.status IN ('open', 'in-progress', 'blocked') AND e.status = 'active') AS "open!"
               FROM action a
               JOIN event e ON e.organization_id = a.organization_id AND e.id = a.event_id
               WHERE a.organization_id = $1 AND a.id = ANY($2)
               UNION ALL
               SELECT 'commitment', c.id, c.event_id, c.version, c.owner_user_id, c.text, coalesce(e.name, ''),
                      c.due_date, c.status,
                      (c.status IN ('conditional', 'firm') AND e.status = 'active')
               FROM commitment c
               JOIN event e ON e.organization_id = c.organization_id AND e.id = c.event_id
               WHERE c.organization_id = $1 AND c.id = ANY($3)"#,
            scope.organization_id().as_uuid(),
            &of(RecordKind::Action),
            &of(RecordKind::Commitment),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(ItemNow {
                    kind: kind(&row.kind, "work.kind")?,
                    record_id: row.id,
                    event_id: EventId::from_uuid(row.event_id),
                    version: version(row.version, "work.version")?,
                    owner: UserId::from_uuid(row.owner),
                    open: row.open,
                    title: row.title,
                    event_name: row.event_name,
                    due_date: row.due_date.map(|date| date.to_jiff()),
                    status: row.status,
                })
            })
            .collect()
    }

    async fn claim(
        &self,
        scope: OrgScope,
        intent: Uuid,
        at: Timestamp,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query!(
            "UPDATE outbound_intent SET status = 'unknown', finished_at = $3
             WHERE id = $2 AND organization_id = $1 AND purpose = 'telegram-digest' AND status = 'pending'",
            scope.organization_id().as_uuid(),
            intent,
            at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn defer(
        &self,
        scope: OrgScope,
        intent: Uuid,
        run_at: Timestamp,
    ) -> Result<(), StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        enqueue(
            &mut conn,
            &NewJob {
                kind: SEND_DIGEST_JOB,
                version: 1,
                payload: json!({"intent_id": intent}),
                organization_id: Some(scope.organization_id()),
                request_id: None,
                run_at: Some(run_at),
            },
        )
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn release(&self, scope: OrgScope, intent: Uuid) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE outbound_intent SET status = 'pending', finished_at = NULL
             WHERE id = $2 AND organization_id = $1 AND purpose = 'telegram-digest' AND status = 'unknown'",
            scope.organization_id().as_uuid(),
            intent,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn finish(
        &self,
        scope: OrgScope,
        intent: Uuid,
        outcome: DigestOutcome,
        kept: &[DigestItemId],
        at: Timestamp,
    ) -> Result<(), StoreError> {
        let organization = scope.organization_id().as_uuid();
        let kept: Vec<Uuid> = kept.iter().map(|id| id.as_uuid()).collect();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // A claimed digest is `unknown` until its outcome. A worker that stops before the outcome leaves it
        // `unknown`, with the time of the claim for the housekeeping.
        let finished = sqlx::query!(
            "UPDATE outbound_intent SET status = $3, finished_at = $4
             WHERE id = $2 AND organization_id = $1 AND purpose = 'telegram-digest' AND status = 'unknown'",
            organization,
            intent,
            outcome.status().as_str(),
            at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if finished.rows_affected() == 0 {
            return Ok(());
        }
        // The items that the message did not show have no button; the rules plan them again.
        sqlx::query!(
            "DELETE FROM digest_item WHERE intent_id = $2 AND organization_id = $1 AND NOT (id = ANY($3))",
            organization,
            intent,
            &kept,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        if outcome.notifies() {
            // The notice of a new version replaces the notice of the old one. The response stays with its
            // version, so the rules see that it answers an older version.
            sqlx::query!(
                "INSERT INTO reminder_state
                     (organization_id, record_kind, record_id, user_id, notified_version, notified_at)
                 SELECT d.organization_id, d.record_kind, d.record_id, i.user_id, d.record_version, $3
                 FROM digest_item d
                 JOIN outbound_intent i ON i.id = d.intent_id AND i.organization_id = d.organization_id
                 WHERE d.intent_id = $2 AND d.organization_id = $1
                 ON CONFLICT (organization_id, record_kind, record_id, user_id) DO UPDATE
                     SET notified_version = EXCLUDED.notified_version, notified_at = EXCLUDED.notified_at",
                organization,
                intent,
                at.to_sqlx() as _,
            )
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        }
        if outcome == DigestOutcome::Blocked {
            // The link belongs to the user, not to the organization: a blocked bot reaches the user in no
            // organization. The digest of the scope names the user.
            sqlx::query!(
                "UPDATE telegram_identity SET blocked_at = $3
                 WHERE blocked_at IS NULL AND user_id = (
                     SELECT user_id FROM outbound_intent
                     WHERE id = $2 AND organization_id = $1 AND purpose = 'telegram-digest')",
                organization,
                intent,
                at.to_sqlx() as _,
            )
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)
    }

    async fn cancel(&self, scope: OrgScope, intent: Uuid, at: Timestamp) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE outbound_intent SET status = 'canceled', finished_at = $3
             WHERE id = $2 AND organization_id = $1 AND purpose = 'telegram-digest' AND status = 'pending'",
            scope.organization_id().as_uuid(),
            intent,
            at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }
}

#[async_trait]
impl DigestLog for Database {
    async fn recent(&self, scope: OrgScope, limit: u32) -> Result<Vec<DigestSummary>, StoreError> {
        let rows = sqlx::query!(
            r#"SELECT i.id, u.display_name, i.status,
                      i.created_at AS "created_at: jiff_sqlx::Timestamp",
                      i.finished_at AS "finished_at: jiff_sqlx::Timestamp",
                      (SELECT count(*) FROM digest_item d WHERE d.intent_id = i.id) AS "item_count!"
               FROM outbound_intent i
               JOIN app_user u ON u.id = i.user_id
               WHERE i.organization_id = $1 AND i.purpose = 'telegram-digest'
               ORDER BY i.created_at DESC, i.id DESC
               LIMIT $2"#,
            scope.organization_id().as_uuid(),
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let status =
                    DigestStatus::parse(&row.status).ok_or(InvalidRow("outbound_intent.status"))?;
                Ok(DigestSummary {
                    id: row.id,
                    user_display_name: row.display_name,
                    created_at: row.created_at.to_jiff(),
                    sent_at: (status == DigestStatus::Sent)
                        .then_some(row.finished_at)
                        .flatten()
                        .map(|at| at.to_jiff()),
                    status,
                    item_count: u32::try_from(row.item_count)
                        .map_err(|_| InvalidRow("digest_item.count"))?,
                })
            })
            .collect()
    }
}
