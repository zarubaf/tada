//! The `PmStore` adapter: the recipients, the work and the notices of the daily check, and the planned
//! digest (ADR 0076).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff_sqlx::ToSqlx;
use serde_json::json;
use tada_app::access::EventReach;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::events::EventTimeZone;
use tada_app::domain::ids::UserId;
use tada_app::domain::reminders::{Answer, Candidate, Memory, Reason, RecordKind};
use tada_app::jobs::NewJob;
use tada_app::pm::SEND_DIGEST_JOB;
use tada_app::pm::check::{PmStore, Recipient};
use tada_app::store::StoreError;
use uuid::Uuid;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::identity::organization_role;
use crate::jobs::enqueue;

fn version(value: i64, column: &'static str) -> Result<RecordVersion, InvalidRow> {
    RecordVersion::new(value).ok_or(InvalidRow(column))
}

fn kind(code: &str, column: &'static str) -> Result<RecordKind, InvalidRow> {
    RecordKind::parse(code).ok_or(InvalidRow(column))
}

/// 16 random bytes from the OS: the opaque reference of a digest item. A button carries it, so it must not
/// tell the time or the order of the items, as a UUIDv7 would.
fn random_reference() -> Result<Uuid, StoreError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        StoreError::Internal(Box::new(std::io::Error::other(error.to_string())))
    })?;
    Ok(Uuid::from_bytes(bytes))
}

#[async_trait]
impl PmStore for Database {
    async fn recipients(&self, scope: OrgScope) -> Result<Vec<Recipient>, StoreError> {
        // A confirmed Telegram link is a `telegram_identity` row, as for the opt-in (`account_reminders`).
        // A canceled or failed digest does not count for the cooldown; a pending one does, so a retry of the
        // check plans no second digest. The cooldown starts at the planning time: a send that waits for the end
        // of the quiet hours must not push the digest of the next day out.
        let rows = sqlx::query!(
            r#"SELECT u.id, m.role, u.time_zone,
                      (SELECT max(i.created_at) FROM outbound_intent i
                       WHERE i.organization_id = m.organization_id AND i.user_id = u.id
                         AND i.purpose = 'telegram-digest' AND i.status IN ('pending', 'sent', 'unknown'))
                          AS "last_digest_at: jiff_sqlx::Timestamp"
               FROM organization_membership m
               JOIN app_user u ON u.id = m.user_id
               JOIN telegram_identity t ON t.user_id = u.id
               WHERE m.organization_id = $1 AND u.reminders_opt_in_at IS NOT NULL AND t.blocked_at IS NULL
               ORDER BY u.id"#,
            scope.organization_id().as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(Recipient {
                    user_id: UserId::from_uuid(row.id),
                    role: organization_role(&row.role)?,
                    zone: EventTimeZone::parse(&row.time_zone)
                        .map_err(|_| InvalidRow("app_user.time_zone"))?,
                    last_digest_at: row.last_digest_at.map(|at| at.to_jiff()),
                })
            })
            .collect()
    }

    async fn candidates(
        &self,
        scope: OrgScope,
        user: UserId,
        events: &EventReach,
        until: Date,
    ) -> Result<Vec<Candidate>, StoreError> {
        // `None` is each event of the organization.
        let events: Option<Vec<Uuid>> = match events {
            EventReach::Organization => None,
            EventReach::Events(events) => Some(events.iter().map(|id| id.as_uuid()).collect()),
        };
        let rows = sqlx::query!(
            r#"SELECT kind AS "kind!", id AS "id!", version AS "version!",
                      due_date AS "due_date: jiff_sqlx::Date", blocked AS "blocked!"
               FROM (
                   SELECT 'action' AS kind, a.id, a.version, a.due_date, a.status = 'blocked' AS blocked
                   FROM action a
                   JOIN event e ON e.organization_id = a.organization_id AND e.id = a.event_id
                   WHERE a.organization_id = $1 AND a.owner_user_id = $2 AND e.status = 'active'
                     AND ($4::uuid[] IS NULL OR a.event_id = ANY($4))
                     AND a.status IN ('open', 'in-progress', 'blocked')
                     AND (a.status = 'blocked' OR a.due_date <= $3)
                   UNION ALL
                   SELECT 'commitment', c.id, c.version, c.due_date, false
                   FROM commitment c
                   JOIN event e ON e.organization_id = c.organization_id AND e.id = c.event_id
                   WHERE c.organization_id = $1 AND c.owner_user_id = $2 AND e.status = 'active'
                     AND ($4::uuid[] IS NULL OR c.event_id = ANY($4))
                     AND c.status IN ('conditional', 'firm') AND c.due_date <= $3
               ) AS work
               ORDER BY due_date NULLS LAST, kind, id"#,
            scope.organization_id().as_uuid(),
            user.as_uuid(),
            until.to_sqlx() as _,
            events.as_deref(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(Candidate {
                    kind: kind(&row.kind, "work.kind")?,
                    id: row.id,
                    version: version(row.version, "work.version")?,
                    due_date: row.due_date.map(|date| date.to_jiff()),
                    blocked: row.blocked,
                })
            })
            .collect()
    }

    async fn memory(
        &self,
        scope: OrgScope,
        user: UserId,
        records: &[(RecordKind, Uuid)],
    ) -> Result<Vec<((RecordKind, Uuid), Memory)>, StoreError> {
        let kinds: Vec<&str> = records.iter().map(|(kind, _)| kind.as_str()).collect();
        let ids: Vec<Uuid> = records.iter().map(|(_, id)| *id).collect();
        let rows = sqlx::query!(
            r#"SELECT s.record_kind, s.record_id, s.notified_version,
                      s.notified_at AS "notified_at: jiff_sqlx::Timestamp", s.response, s.response_version,
                      s.response_at AS "response_at: jiff_sqlx::Timestamp"
               FROM reminder_state s
               JOIN unnest($3::text[], $4::uuid[]) AS r (kind, id)
                 ON r.kind = s.record_kind AND r.id = s.record_id
               WHERE s.organization_id = $1 AND s.user_id = $2"#,
            scope.organization_id().as_uuid(),
            user.as_uuid(),
            &kinds as &[&str],
            &ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let response = row
                    .response
                    .map(|code| Answer::parse(&code).ok_or(InvalidRow("reminder_state.response")))
                    .transpose()?;
                let response_version = row
                    .response_version
                    .map(|value| version(value, "reminder_state.response_version"))
                    .transpose()?;
                Ok((
                    (
                        kind(&row.record_kind, "reminder_state.record_kind")?,
                        row.record_id,
                    ),
                    Memory {
                        notified_version: version(
                            row.notified_version,
                            "reminder_state.notified_version",
                        )?,
                        notified_at: row.notified_at.to_jiff(),
                        response,
                        response_version,
                        response_at: row.response_at.map(|at| at.to_jiff()),
                    },
                ))
            })
            .collect()
    }

    async fn plan_digest(
        &self,
        scope: OrgScope,
        user: UserId,
        items: &[(Candidate, Reason)],
        now: Timestamp,
        send_at: Timestamp,
    ) -> Result<Uuid, StoreError> {
        let organization = scope.organization_id();
        let id = Uuid::now_v7();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The time of the check, from the app clock (ADR 0038): the cooldown of the next check compares with it.
        sqlx::query!(
            "INSERT INTO outbound_intent (id, organization_id, user_id, purpose, message_id, created_at)
             VALUES ($1, $2, $3, 'telegram-digest', $4, $5)",
            id,
            organization.as_uuid(),
            user.as_uuid(),
            id.simple().to_string(),
            now.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        for (position, (candidate, reason)) in items.iter().enumerate() {
            let position =
                i16::try_from(position).map_err(|_| InvalidRow("digest_item.position"))?;
            sqlx::query!(
                "INSERT INTO digest_item
                     (id, intent_id, organization_id, record_kind, record_id, record_version, reason, position)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                random_reference()?,
                id,
                organization.as_uuid(),
                candidate.kind.as_str(),
                candidate.id,
                candidate.version.get(),
                reason.as_str(),
                position,
            )
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        }
        enqueue(
            &mut tx,
            &NewJob {
                kind: SEND_DIGEST_JOB,
                version: 1,
                payload: json!({"intent_id": id}),
                organization_id: Some(organization),
                request_id: None,
                run_at: Some(send_at),
            },
        )
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(id)
    }
}
