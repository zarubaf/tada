//! The job queue (ADR 0054). The queries without an organization scope are infrastructure queries
//! (ADR 0039): each one returns the organization ID of the job.

use std::time::Duration;

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::{Json, Uuid};
use tada_app::domain::ids::OrganizationId;
use tada_app::jobs::{Job, JobQueue, NewJob};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;

/// Adds a job inside the transaction of a command. A rollback removes the job with the change.
pub(crate) async fn enqueue(conn: &mut PgConnection, job: &NewJob) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO job (id, organization_id, kind, version, payload, request_id, run_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, coalesce($7, now()), now())",
        id,
        job.organization_id.map(OrganizationId::as_uuid),
        job.kind,
        job.version,
        Json(&job.payload) as _,
        job.request_id,
        job.run_at.map(|at| at.to_sqlx()) as _,
    )
    .execute(conn)
    .await?;
    Ok(id)
}

#[async_trait]
impl JobQueue for Database {
    async fn claim(&self, worker_id: Uuid, lease: Duration) -> Result<Option<Job>, StoreError> {
        let row = sqlx::query!(
            r#"UPDATE job
               SET locked_by = $1, locked_until = now() + make_interval(secs => $2), attempts = attempts + 1
               WHERE id = (
                   SELECT id FROM job
                   WHERE failed_at IS NULL AND run_at <= now() AND (locked_until IS NULL OR locked_until < now())
                   ORDER BY run_at, id
                   FOR UPDATE SKIP LOCKED
                   LIMIT 1
               )
               RETURNING id, kind, version, payload AS "payload: Json<serde_json::Value>", organization_id,
                         request_id, attempts, max_attempts"#,
            worker_id,
            lease.as_secs_f64(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(|row| Job {
            id: row.id,
            kind: row.kind,
            version: row.version,
            payload: row.payload.0,
            organization_id: row.organization_id.map(OrganizationId::from_uuid),
            request_id: row.request_id,
            attempt: row.attempts,
            max_attempts: row.max_attempts,
        }))
    }

    async fn complete(&self, job: &Job, worker_id: Uuid) -> Result<bool, StoreError> {
        let result = sqlx::query!(
            "DELETE FROM job WHERE id = $1 AND locked_by = $2 AND attempts = $3",
            job.id,
            worker_id,
            job.attempt,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn fail(&self, job: &Job, worker_id: Uuid, reason: &str) -> Result<bool, StoreError> {
        // The backoff doubles from 20 seconds and stops growing at one hour.
        let result = sqlx::query!(
            "UPDATE job
             SET locked_by = NULL, locked_until = NULL, last_error = $4,
                 failed_at = CASE WHEN attempts >= max_attempts THEN now() END,
                 run_at = CASE WHEN attempts >= max_attempts THEN run_at
                               ELSE now() + make_interval(secs => least(power(2, attempts) * 10, 3600)) END
             WHERE id = $1 AND locked_by = $2 AND attempts = $3",
            job.id,
            worker_id,
            job.attempt,
            reason,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;
    use tada_app::jobs::{Handlers, JobFailed, JobHandler, JobWarning, Ran, run_next};

    use super::*;
    use crate::testing::TestDatabase;

    const LEASE: Duration = Duration::from_secs(60);

    fn job(kind: &'static str) -> NewJob {
        NewJob {
            kind,
            version: 1,
            payload: json!({"n": 1}),
            organization_id: None,
            request_id: Some(Uuid::now_v7()),
            run_at: None,
        }
    }

    async fn add(test: &TestDatabase, job: &NewJob) -> Uuid {
        let mut conn = test.database.pool.acquire().await.unwrap();
        enqueue(&mut conn, job).await.unwrap()
    }

    #[tokio::test]
    async fn a_rolled_back_command_leaves_no_job() {
        let test = TestDatabase::start().await;
        let mut tx = test.database.pool.begin().await.unwrap();
        enqueue(&mut tx, &job("ping")).await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(
            test.database.claim(Uuid::now_v7(), LEASE).await.unwrap(),
            None
        );

        let mut tx = test.database.pool.begin().await.unwrap();
        let id = enqueue(&mut tx, &job("ping")).await.unwrap();
        tx.commit().await.unwrap();
        let claimed = test
            .database
            .claim(Uuid::now_v7(), LEASE)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (claimed.id, claimed.kind.as_str(), claimed.attempt),
            (id, "ping", 1)
        );
        assert_eq!(claimed.payload, json!({"n": 1}));
    }

    #[tokio::test]
    async fn two_workers_never_run_the_same_job() {
        let test = TestDatabase::start().await;
        for _ in 0..40 {
            add(&test, &job("ping")).await;
        }
        let mut workers = Vec::new();
        for _ in 0..2 {
            let database = test.database.clone();
            workers.push(tokio::spawn(async move {
                let worker_id = Uuid::now_v7();
                let mut ids = Vec::new();
                while let Some(job) = database.claim(worker_id, LEASE).await.unwrap() {
                    assert!(database.complete(&job, worker_id).await.unwrap());
                    ids.push(job.id);
                }
                ids
            }));
        }
        let mut all = Vec::new();
        for worker in workers {
            all.extend(worker.await.unwrap());
        }
        assert_eq!(all.len(), 40);
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), 40);
    }

    #[tokio::test]
    async fn an_expired_lease_returns_the_job_to_the_queue() {
        let test = TestDatabase::start().await;
        let id = add(&test, &job("ping")).await;
        let (first, second) = (Uuid::now_v7(), Uuid::now_v7());

        let lost = test
            .database
            .claim(first, Duration::from_millis(200))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            test.database.claim(second, LEASE).await.unwrap(),
            None,
            "the lease holds"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        let again = test.database.claim(second, LEASE).await.unwrap().unwrap();
        assert_eq!((again.id, again.attempt), (id, 2));

        assert!(
            !test.database.complete(&lost, first).await.unwrap(),
            "the first worker lost the job"
        );
        assert!(test.database.complete(&again, second).await.unwrap());
    }

    #[tokio::test]
    async fn a_failed_job_waits_for_its_backoff_and_fails_for_good_after_the_last_attempt() {
        let test = TestDatabase::start().await;
        let id = add(&test, &job("ping")).await;
        sqlx::query!("UPDATE job SET max_attempts = 2 WHERE id = $1", id)
            .execute(&test.database.pool)
            .await
            .unwrap();
        let worker = Uuid::now_v7();

        let first = test.database.claim(worker, LEASE).await.unwrap().unwrap();
        assert!(!first.is_last_attempt());
        assert!(
            test.database
                .fail(&first, worker, "the provider refused")
                .await
                .unwrap()
        );
        assert_eq!(
            test.database.claim(worker, LEASE).await.unwrap(),
            None,
            "the backoff holds"
        );

        sqlx::query!("UPDATE job SET run_at = now() WHERE id = $1", id)
            .execute(&test.database.pool)
            .await
            .unwrap();
        let last = test.database.claim(worker, LEASE).await.unwrap().unwrap();
        assert!(last.is_last_attempt());
        assert!(
            test.database
                .fail(&last, worker, "the provider refused")
                .await
                .unwrap()
        );
        let row = sqlx::query!(
            "SELECT failed_at IS NOT NULL AS \"failed!\", last_error FROM job WHERE id = $1",
            id
        )
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert!(row.failed);
        assert_eq!(row.last_error.as_deref(), Some("the provider refused"));
        assert_eq!(test.database.claim(worker, LEASE).await.unwrap(), None);
    }

    #[derive(Debug, Default)]
    struct Counter(AtomicUsize);

    #[async_trait]
    impl JobHandler for Counter {
        fn kind(&self) -> &'static str {
            "count"
        }

        async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed> {
            assert_eq!(job.version, 1);
            let count = self.0.fetch_add(1, Ordering::SeqCst);
            Ok((count > 0).then(|| JobWarning("counted again".into())))
        }
    }

    #[tokio::test]
    async fn the_worker_step_runs_the_handler_of_the_kind() {
        let test = TestDatabase::start().await;
        let counter = Arc::new(Counter::default());
        let handlers = Handlers::default().with(counter.clone());
        let worker = Uuid::now_v7();

        let counted = add(&test, &job("count")).await;
        let unknown = add(&test, &job("unknown")).await;
        let again = add(&test, &job("count")).await;
        assert_eq!(
            run_next(&test.database, &handlers, worker, LEASE)
                .await
                .unwrap(),
            Ran::Completed(counted)
        );
        assert_eq!(
            run_next(&test.database, &handlers, worker, LEASE)
                .await
                .unwrap(),
            Ran::Failed(unknown)
        );
        assert_eq!(
            run_next(&test.database, &handlers, worker, LEASE)
                .await
                .unwrap(),
            Ran::CompletedWithWarning(again, JobWarning("counted again".into()))
        );
        assert_eq!(
            run_next(&test.database, &handlers, worker, LEASE)
                .await
                .unwrap(),
            Ran::Idle
        );
        assert_eq!(counter.0.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_heartbeat_shows_the_due_jobs() {
        let test = TestDatabase::start().await;
        add(&test, &job("ping")).await;
        add(&test, &job("ping")).await;
        let worker = Uuid::now_v7();
        test.database.record_heartbeat(worker).await.unwrap();
        let row = sqlx::query!(
            "SELECT queue_depth, oldest_due_seconds FROM worker_heartbeat WHERE worker_id = $1",
            worker
        )
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(row.queue_depth, 2);
        assert!(row.oldest_due_seconds.is_some());
    }
}
