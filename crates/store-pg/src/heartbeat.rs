//! The worker heartbeat (ADR 0025). The database clock gives the times.

use sqlx::types::Uuid;

use crate::Database;

impl Database {
    /// Records that the worker `worker_id` completed a loop, with the number of due jobs and the age of
    /// the oldest one (ADR 0035).
    pub async fn record_heartbeat(&self, worker_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO worker_heartbeat (worker_id, started_at, last_loop_at, queue_depth, oldest_due_seconds)
             SELECT $1, now(), now(), count(*), extract(epoch FROM now() - min(run_at))::bigint
             FROM job
             WHERE failed_at IS NULL AND run_at <= now()
             ON CONFLICT (worker_id) DO UPDATE
             SET last_loop_at = now(), queue_depth = excluded.queue_depth,
                 oldest_due_seconds = excluded.oldest_due_seconds",
            worker_id,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    /// Removes the heartbeat of a worker that stops cleanly.
    pub async fn remove_heartbeat(&self, worker_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM worker_heartbeat WHERE worker_id = $1",
            worker_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }
}
