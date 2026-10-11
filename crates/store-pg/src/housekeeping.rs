//! The `HousekeepingStore` adapter: the daily deletion of technical rows (ADR 0071).
//!
//! Each rule is one statement, or a small group in the order of the foreign keys. The cutoff of a rule is
//! computed here from the constants of the application, so no period is written twice.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::{AssertSqlSafe, PgConnection};
use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::caller::{JobRunner, ServiceCaller};
use tada_app::domain::ids::UserId;
use tada_app::housekeeping::{
    HousekeepingReport, HousekeepingStore, LONG_KEEP, SHORT_KEEP, TOMBSTONE_NAME,
};
use tada_app::session::{ABSOLUTE_TIMEOUT, IDLE_TIMEOUT};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::store_error;

/// A sign-in link ends when it expires. A link that a sign-in used is deleted at once.
const MAGIC_LINKS: &str = "DELETE FROM magic_link WHERE expires_at < $1";

/// A session ends by its idle timeout (since its last use) or its absolute timeout (since its creation).
/// $1 is the last use before which the idle timeout ended long enough ago, $2 the same for the creation.
/// A sign-out and a revocation delete the row at once.
const SESSIONS: &str = "DELETE FROM session WHERE last_used_at < $1 OR created_at < $2";

/// A claim ends when the member confirms it or, if not, when its code expires.
const TELEGRAM_CLAIMS: &str =
    "DELETE FROM telegram_link_code WHERE coalesce(confirmed_at, expires_at) < $1";

/// A mail intent ends with its outcome. An intent with an unknown outcome stays for the operator.
const OUTBOUND_INTENTS: &str = "DELETE FROM outbound_intent
     WHERE purpose <> 'telegram-digest' AND status IN ('sent', 'failed') AND finished_at < $1";

/// A digest ends with its outcome or its cancel. It holds IDs only, so a digest with an unknown outcome goes too.
/// A digest that is still pending long after its plan lost its send job; it goes after the same period from its
/// plan. Its items go with it (`ON DELETE CASCADE`).
const DIGESTS: &str = "DELETE FROM outbound_intent
     WHERE purpose = 'telegram-digest'
       AND ((status IN ('sent', 'failed', 'unknown', 'canceled') AND finished_at < $1)
            OR (status = 'pending' AND created_at < $1))";

/// A notice ends with the next notice of the record, which replaces it. A response belongs to its notice.
const REMINDER_STATES: &str = "DELETE FROM reminder_state WHERE notified_at < $1";

const FAILED_JOBS: &str = "DELETE FROM job WHERE failed_at < $1";

/// An invitation ends when a member accepts or an administrator revokes it. Its mail intents and tokens
/// refer to it, so they go first, each in its own statement.
const FINISHED_INVITATIONS: [&str; 3] = [
    "DELETE FROM outbound_intent WHERE invitation_id IN (
         SELECT id FROM invitation WHERE coalesce(accepted_at, revoked_at) < $1)",
    "DELETE FROM invitation_token WHERE invitation_id IN (
         SELECT id FROM invitation WHERE coalesce(accepted_at, revoked_at) < $1)",
    "DELETE FROM invitation WHERE coalesce(accepted_at, revoked_at) < $1",
];

/// An account without membership is anonymized once, 90 days after its last membership ended.
/// The membership check repeats here, so an account that joined again in the meantime stays.
const ANONYMIZE: &str = "UPDATE app_user
     SET display_name = $2, anonymized_at = $3, reminders_opt_in_at = NULL, time_zone = DEFAULT
     WHERE anonymized_at IS NULL AND membership_ended_at < $1
       AND NOT EXISTS (SELECT 1 FROM organization_membership WHERE user_id = app_user.id)
     RETURNING id";

/// A person record that linked to an anonymized account loses the link.
const PERSONS_OF_ACCOUNTS: &str =
    "UPDATE person SET user_id = NULL, version = version + 1, updated_at = $2
     WHERE user_id = ANY($1)";

/// The tables with data of an anonymized account, apart from the audit log, which keeps the ID (ADR 0039).
/// An API token cannot exist without a membership, so its delete is a safeguard.
const ACCOUNT_TABLES: [&str; 6] = [
    "email_identity",
    "telegram_identity",
    "telegram_link_code",
    "session",
    "magic_link",
    "api_token",
];

async fn delete_before(
    conn: &mut PgConnection,
    sql: &'static str,
    cutoff: &[Timestamp],
) -> Result<u64, StoreError> {
    let mut query = sqlx::query(sql);
    for time in cutoff {
        query = query.bind(time.to_sqlx());
    }
    let done = query.execute(conn).await.map_err(store_error)?;
    Ok(done.rows_affected())
}

fn before(now: Timestamp, age: jiff::SignedDuration) -> Timestamp {
    now.saturating_sub(age).unwrap_or(Timestamp::MIN)
}

#[async_trait]
impl HousekeepingStore for Database {
    async fn run(
        &self,
        caller: &ServiceCaller<JobRunner>,
        now: Timestamp,
    ) -> Result<HousekeepingReport, StoreError> {
        let short = before(now, SHORT_KEEP);
        let long = before(now, LONG_KEEP);
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let magic_links = delete_before(&mut conn, MAGIC_LINKS, &[short]).await?;
        let sessions = delete_before(
            &mut conn,
            SESSIONS,
            &[before(short, IDLE_TIMEOUT), before(short, ABSOLUTE_TIMEOUT)],
        )
        .await?;
        let telegram_claims = delete_before(&mut conn, TELEGRAM_CLAIMS, &[short]).await?;
        let outbound_intents = delete_before(&mut conn, OUTBOUND_INTENTS, &[long]).await?;
        let digests = delete_before(&mut conn, DIGESTS, &[long]).await?;
        let reminder_states = delete_before(&mut conn, REMINDER_STATES, &[long]).await?;
        let failed_jobs = delete_before(&mut conn, FAILED_JOBS, &[long]).await?;
        let invitations = delete_invitations(&mut conn, long).await?;
        let anonymized_accounts = anonymize_accounts(&mut conn, caller, now, long).await?;
        Ok(HousekeepingReport {
            magic_links,
            sessions,
            telegram_claims,
            invitations,
            outbound_intents,
            digests,
            reminder_states,
            failed_jobs,
            anonymized_accounts,
        })
    }
}

/// Returns the number of deleted invitations.
async fn delete_invitations(conn: &mut PgConnection, cutoff: Timestamp) -> Result<u64, StoreError> {
    let mut tx = sqlx::Connection::begin(conn).await.map_err(store_error)?;
    let mut deleted = 0;
    for sql in FINISHED_INVITATIONS {
        deleted = delete_before(&mut tx, sql, &[cutoff]).await?;
    }
    tx.commit().await.map_err(store_error)?;
    Ok(deleted)
}

/// Returns the number of anonymized accounts.
async fn anonymize_accounts(
    conn: &mut PgConnection,
    caller: &ServiceCaller<JobRunner>,
    now: Timestamp,
    cutoff: Timestamp,
) -> Result<u64, StoreError> {
    let mut tx = sqlx::Connection::begin(conn).await.map_err(store_error)?;
    let users: Vec<sqlx::types::Uuid> = sqlx::query_scalar(ANONYMIZE)
        .bind(cutoff.to_sqlx())
        .bind(TOMBSTONE_NAME)
        .bind(now.to_sqlx())
        .fetch_all(&mut *tx)
        .await
        .map_err(store_error)?;
    for table in ACCOUNT_TABLES {
        // The table names are constants of this module.
        sqlx::query(AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE user_id = ANY($1)"
        )))
        .bind(&users)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
    }
    // A person record no longer points to the account. The record is a contact of the club (ADR 0069).
    sqlx::query(PERSONS_OF_ACCOUNTS)
        .bind(&users)
        .bind(now.to_sqlx())
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
    for user in &users {
        let event = AuditEvent::new(
            caller.actor(),
            AuditAction::AppUserAnonymize,
            Some(*user),
            None,
        )
        .about(UserId::from_uuid(*user));
        audit::record(&mut tx, &event).await.map_err(store_error)?;
    }
    tx.commit().await.map_err(store_error)?;
    Ok(users.len() as u64)
}
