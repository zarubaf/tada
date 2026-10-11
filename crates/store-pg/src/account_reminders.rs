//! The `ReminderSettingsStore` adapter: the opt-in and the time zone of an account (ADR 0076).

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use tada_app::account_reminders::{ReminderChange, ReminderSettings, ReminderSettingsStore};
use tada_app::audit::AuditEvent;
use tada_app::caller::MemberCaller;
use tada_app::domain::events::EventTimeZone;
use tada_app::store::StoreError;

use crate::error::{InvalidRow, store_error};
use crate::{Database, audit};

/// The opt-in time, the time zone and the link state of a user.
type Row = (Option<jiff_sqlx::Timestamp>, String, bool);

/// A confirmed link is a `telegram_identity` row: the member confirmed it after the Telegram account accepted.
const COLUMNS: &str = "reminders_opt_in_at, time_zone,
     EXISTS (SELECT 1 FROM telegram_identity t WHERE t.user_id = app_user.id)";

fn settings(row: Row) -> Result<ReminderSettings, InvalidRow> {
    Ok(ReminderSettings {
        opted_in_at: row.0.map(|at| at.to_jiff()),
        time_zone: EventTimeZone::parse(&row.1).map_err(|_| InvalidRow("app_user.time_zone"))?,
        telegram_linked: row.2,
    })
}

#[async_trait]
impl ReminderSettingsStore for Database {
    async fn get(&self, caller: &MemberCaller) -> Result<ReminderSettings, StoreError> {
        let row: Row = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM app_user WHERE id = $1"
        )))
        .bind(caller.user_id().as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(settings(row)?)
    }

    async fn set(
        &self,
        caller: &MemberCaller,
        change: &ReminderChange,
        event: &AuditEvent,
    ) -> Result<Option<ReminderSettings>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // An opt-in needs the link. An unlink after the opt-in is harmless: the send checks the link again.
        // A user who opted in already keeps the first opt-in time.
        let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "UPDATE app_user
             SET reminders_opt_in_at = CASE WHEN $2::timestamptz IS NULL THEN NULL
                                            ELSE coalesce(reminders_opt_in_at, $2) END,
                 time_zone = $3
             WHERE id = $1
               AND ($2::timestamptz IS NULL
                    OR EXISTS (SELECT 1 FROM telegram_identity t WHERE t.user_id = app_user.id))
             RETURNING {COLUMNS}"
        )))
        .bind(caller.user_id().as_uuid())
        .bind(change.opt_in_at.map(|at| at.to_sqlx()))
        .bind(change.time_zone.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        audit::record(&mut tx, event).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(settings(row)?))
    }
}
