//! The worker heartbeat (ADR 0025). The database clock gives the times.

use sqlx::types::Uuid;

use crate::Database;

impl Database {
    /// Records that the worker `worker_id` completed a loop.
    pub async fn record_heartbeat(&self, worker_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO worker_heartbeat (worker_id, started_at, last_loop_at)
             VALUES ($1, now(), now())
             ON CONFLICT (worker_id) DO UPDATE SET last_loop_at = now()",
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
