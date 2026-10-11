//! The `PmPolicyStore` adapter: the reminder policy of an organization (ADR 0076).
//!
//! An organization without a row has the defaults with version 1. The first change inserts the row with
//! version 2. Each change writes the schedule of the daily check in the same transaction. An organization
//! without a row gets the schedule of the defaults from `ensure_default_schedules`.

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::reminders::PmPolicy;
use tada_app::pm::PM_CHECK_JOB;
use tada_app::pm::policy::PmPolicyStore;
use tada_app::store::StoreError;

use crate::error::{InvalidRow, store_error};
use crate::{Database, audit, schedules};
use sqlx::PgPool;

/// A missed daily check starts once: a worker that was down sends no burst of checks.
const PM_CHECK_MISSED_RUN_LIMIT: u8 = 1;

/// The columns of a row, in the order of `COLUMNS`.
type Row = (
    jiff_sqlx::Time,
    String,
    i16,
    jiff_sqlx::Time,
    jiff_sqlx::Time,
    i16,
    i16,
    i16,
    bool,
    bool,
    i64,
);

/// Adds the schedule of the default policy for each organization without a policy and without a schedule.
/// Returns the number of new schedules. A policy that an owner or admin saved keeps its own schedule.
pub(crate) async fn ensure_default_schedules(pool: &PgPool) -> Result<u32, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let schedule = PmPolicy::default().schedule;
    let next_run_at = schedule.next_after(schedules::database_now(&mut tx).await?);
    let added = sqlx::query(
        "INSERT INTO schedule
             (id, organization_id, kind, local_time, time_zone, weekdays, next_run_at, missed_run_limit,
              created_at, updated_at)
         SELECT uuidv7(), o.id, $1, $2, $3, $4, $5, $6, now(), now()
         FROM organization o
         WHERE NOT EXISTS (SELECT 1 FROM schedule s WHERE s.organization_id = o.id AND s.kind = $1)
           AND NOT EXISTS (SELECT 1 FROM pm_policy p WHERE p.organization_id = o.id)
         ON CONFLICT (organization_id, kind) DO NOTHING",
    )
    .bind(PM_CHECK_JOB)
    .bind(schedule.time.to_sqlx())
    .bind(schedule.zone.as_str())
    .bind(i16::from(schedule.weekdays.bits()))
    .bind(next_run_at.to_sqlx())
    .bind(i16::from(PM_CHECK_MISSED_RUN_LIMIT))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(u32::try_from(added).unwrap_or(u32::MAX))
}

fn small(value: i16, column: &'static str) -> Result<u8, InvalidRow> {
    u8::try_from(value).map_err(|_| InvalidRow(column))
}

fn policy(row: Row) -> Result<(PmPolicy, RecordVersion), InvalidRow> {
    let policy = PmPolicy::new(
        row.0.to_jiff(),
        &row.1,
        small(row.2, "pm_policy.weekdays")?,
        row.3.to_jiff(),
        row.4.to_jiff(),
        small(row.5, "pm_policy.cooldown_hours")?,
        small(row.6, "pm_policy.follow_up_working_days")?,
        small(row.7, "pm_policy.look_ahead_working_days")?,
        row.8,
        row.9,
    )
    .map_err(|_| InvalidRow("pm_policy"))?;
    let version = RecordVersion::new(row.10).ok_or(InvalidRow("pm_policy.version"))?;
    Ok((policy, version))
}

#[async_trait]
impl PmPolicyStore for Database {
    async fn get(&self, scope: OrgScope) -> Result<(PmPolicy, RecordVersion), StoreError> {
        let row: Option<Row> = sqlx::query_as(
            "SELECT local_time, time_zone, weekdays, quiet_start, quiet_end, cooldown_hours,
                    follow_up_working_days, look_ahead_working_days, paused, ai_enabled, version
             FROM pm_policy WHERE organization_id = $1",
        )
        .bind(scope.organization_id().as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        match row {
            Some(row) => Ok(policy(row)?),
            None => Ok((PmPolicy::default(), RecordVersion::FIRST)),
        }
    }

    async fn set(
        &self,
        scope: OrgScope,
        policy: &PmPolicy,
        expected: RecordVersion,
        event: &AuditEvent,
    ) -> Result<Option<RecordVersion>, StoreError> {
        let organization = scope.organization_id();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The defaults have version 1 and no row: the first change inserts. A concurrent first change finds
        // the row of the other and conflicts.
        let sql = if expected == RecordVersion::FIRST {
            "INSERT INTO pm_policy
                 (organization_id, local_time, time_zone, weekdays, quiet_start, quiet_end, cooldown_hours,
                  follow_up_working_days, look_ahead_working_days, paused, ai_enabled, version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12 + 1)
             ON CONFLICT (organization_id) DO NOTHING
             RETURNING version"
        } else {
            "UPDATE pm_policy
             SET local_time = $2, time_zone = $3, weekdays = $4, quiet_start = $5, quiet_end = $6,
                 cooldown_hours = $7, follow_up_working_days = $8, look_ahead_working_days = $9,
                 paused = $10, ai_enabled = $11, version = version + 1
             WHERE organization_id = $1 AND version = $12
             RETURNING version"
        };
        let version: Option<i64> = sqlx::query_scalar(sql)
            .bind(organization.as_uuid())
            .bind(policy.schedule.time.to_sqlx())
            .bind(policy.schedule.zone.as_str())
            .bind(i16::from(policy.schedule.weekdays.bits()))
            .bind(policy.quiet.start.to_sqlx())
            .bind(policy.quiet.end.to_sqlx())
            .bind(i16::from(policy.cooldown_hours))
            .bind(i16::from(policy.follow_up_working_days))
            .bind(i16::from(policy.look_ahead_working_days))
            .bind(policy.paused)
            .bind(policy.ai_enabled)
            .bind(expected.get())
            .fetch_optional(&mut *tx)
            .await
            .map_err(store_error)?;
        let Some(version) = version else {
            return Ok(None);
        };
        schedules::put(
            &mut tx,
            organization,
            PM_CHECK_JOB,
            &policy.schedule,
            PM_CHECK_MISSED_RUN_LIMIT,
        )
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, event).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(
            RecordVersion::new(version).ok_or(InvalidRow("pm_policy.version"))?,
        ))
    }
}
