//! The `ScheduleRunner` adapter: the durable schedules of the worker (ADR 0054, ADR 0076).
//!
//! A schedule row names a job kind, a local time, a time zone and weekdays. The step locks one due row at a
//! time with `SKIP LOCKED`, so two workers never start the same run, and adds its jobs in the same transaction
//! in which it moves the row to the next run. The database clock gives the current time.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use serde_json::json;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::domain::events::EventTimeZone;
use tada_app::domain::ids::OrganizationId;
use tada_app::domain::schedule::{Due, LocalSchedule, Weekdays};
use tada_app::jobs::{NewJob, ScheduleRunner, ScheduleStep};
use tada_app::pm::PM_CHECK_JOB;
use tada_app::store::StoreError;

use crate::error::{InvalidRow, store_error};
use crate::jobs::enqueue;
use crate::{Database, pm_policy};

/// The job kinds that a schedule can start.
const SCHEDULED_KINDS: &[&str] = &[PM_CHECK_JOB];

/// One due schedule: ID, organization, kind, local time, time zone, weekdays, next run, missed-run limit.
type DueRow = (
    Uuid,
    Option<Uuid>,
    String,
    jiff_sqlx::Time,
    String,
    i16,
    jiff_sqlx::Timestamp,
    i16,
);

/// The time of the database in the transaction of `conn`.
pub(crate) async fn database_now(conn: &mut PgConnection) -> Result<Timestamp, sqlx::Error> {
    let now: jiff_sqlx::Timestamp = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await?;
    Ok(now.to_jiff())
}

/// Creates the schedule of `kind` in the organization or replaces its time, zone, days and limit.
/// The next run is the first slot after the database time; a change does not start a run of the past.
/// A change that keeps the time, the zone and the days keeps the next run, so a change just after a slot
/// does not skip the run of that slot.
pub(crate) async fn put(
    conn: &mut PgConnection,
    organization: OrganizationId,
    kind: &str,
    schedule: &LocalSchedule,
    missed_run_limit: u8,
) -> Result<(), sqlx::Error> {
    let next_run_at = schedule.next_after(database_now(conn).await?);
    sqlx::query(
        "INSERT INTO schedule
             (id, organization_id, kind, local_time, time_zone, weekdays, next_run_at, missed_run_limit,
              created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
         ON CONFLICT (organization_id, kind) DO UPDATE
         SET local_time = excluded.local_time, time_zone = excluded.time_zone, weekdays = excluded.weekdays,
             next_run_at = CASE
                 WHEN (schedule.local_time, schedule.time_zone, schedule.weekdays)
                      IS DISTINCT FROM (excluded.local_time, excluded.time_zone, excluded.weekdays)
                 THEN excluded.next_run_at ELSE schedule.next_run_at END,
             missed_run_limit = excluded.missed_run_limit,
             updated_at = excluded.updated_at",
    )
    .bind(Uuid::now_v7())
    .bind(organization.as_uuid())
    .bind(kind)
    .bind(schedule.time.to_sqlx())
    .bind(schedule.zone.as_str())
    .bind(i16::from(schedule.weekdays.bits()))
    .bind(next_run_at.to_sqlx())
    .bind(i16::from(missed_run_limit))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn local_schedule(
    time: jiff_sqlx::Time,
    zone: &str,
    weekdays: i16,
) -> Result<LocalSchedule, InvalidRow> {
    Ok(LocalSchedule {
        time: time.to_jiff(),
        zone: EventTimeZone::parse(zone).map_err(|_| InvalidRow("schedule.time_zone"))?,
        weekdays: u8::try_from(weekdays)
            .ok()
            .and_then(|bits| Weekdays::from_bits(bits).ok())
            .ok_or(InvalidRow("schedule.weekdays"))?,
    })
}

/// The kind of job and the runs of a due row, or the reason why the row cannot be read.
fn plan(
    kind: &str,
    time: jiff_sqlx::Time,
    zone: &str,
    weekdays: i16,
    next_run_at: Timestamp,
    limit: i16,
    now: Timestamp,
) -> Result<(&'static str, Due), InvalidRow> {
    let kind = SCHEDULED_KINDS
        .iter()
        .copied()
        .find(|known| *known == kind)
        .ok_or(InvalidRow("schedule.kind"))?;
    let schedule = local_schedule(time, zone, weekdays)?;
    let limit = u16::try_from(limit).map_err(|_| InvalidRow("schedule.missed_run_limit"))?;
    let due = schedule.due(next_run_at, now, limit);
    // A row that stays due would make `start_due` loop for ever.
    if due.next_run_at <= now {
        return Err(InvalidRow("schedule.next_run_at"));
    }
    Ok((kind, due))
}

/// What `start_one` did with one row.
enum Started {
    Jobs(u32),
    /// The row cannot be read; it moved one day forward.
    Skipped(Uuid, String),
}

impl Database {
    /// Starts the runs of one due schedule in one transaction. Returns `None` if no schedule is due.
    /// A row that cannot be read moves one day forward, so it does not block the rows after it.
    async fn start_one(&self) -> Result<Option<Started>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let now = database_now(&mut tx).await.map_err(store_error)?;
        let row: Option<DueRow> = sqlx::query_as(
            "SELECT id, organization_id, kind, local_time, time_zone, weekdays, next_run_at, missed_run_limit
             FROM schedule WHERE next_run_at <= $1
             ORDER BY next_run_at, id
             LIMIT 1
             FOR UPDATE SKIP LOCKED",
        )
        .bind(now.to_sqlx())
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some((id, organization, kind, time, zone, weekdays, next_run_at, limit)) = row else {
            return Ok(None);
        };
        let started = match plan(
            &kind,
            time,
            &zone,
            weekdays,
            next_run_at.to_jiff(),
            limit,
            now,
        ) {
            Ok((kind, due)) => {
                for _ in 0..due.runs {
                    let job = NewJob {
                        kind,
                        version: 1,
                        payload: json!({}),
                        organization_id: organization.map(OrganizationId::from_uuid),
                        request_id: None,
                        run_at: None,
                    };
                    enqueue(&mut tx, &job).await.map_err(store_error)?;
                }
                sqlx::query(
                    "UPDATE schedule SET next_run_at = $2, last_run_at = $3, updated_at = $3 WHERE id = $1",
                )
                .bind(id)
                .bind(due.next_run_at.to_sqlx())
                .bind(now.to_sqlx())
                .execute(&mut *tx)
                .await
                .map_err(store_error)?;
                Started::Jobs(u32::from(due.runs))
            }
            Err(_) => {
                sqlx::query(
                    "UPDATE schedule SET next_run_at = $2::timestamptz + interval '1 day', updated_at = $2
                     WHERE id = $1",
                )
                .bind(id)
                .bind(now.to_sqlx())
                .execute(&mut *tx)
                .await
                .map_err(store_error)?;
                Started::Skipped(id, kind)
            }
        };
        tx.commit().await.map_err(store_error)?;
        Ok(Some(started))
    }
}

#[async_trait]
impl ScheduleRunner for Database {
    async fn ensure_default_schedules(&self) -> Result<u32, StoreError> {
        pm_policy::ensure_default_schedules(&self.pool)
            .await
            .map_err(store_error)
    }

    async fn start_due(&self) -> Result<ScheduleStep, StoreError> {
        let mut step = ScheduleStep::default();
        while let Some(started) = self.start_one().await? {
            match started {
                Started::Jobs(jobs) => step.started += jobs,
                Started::Skipped(id, kind) => step.skipped.push((id, kind)),
            }
        }
        Ok(step)
    }
}
