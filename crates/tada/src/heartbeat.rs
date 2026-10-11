//! The start checks and the heartbeat of a process role (ADR 0075).

use std::time::Duration;

use anyhow::Context;
use tada_app::build_info::{BuildInfo, Role};
use tada_store_pg::{Database, SchemaCheckError};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// The time between two heartbeats of `serve` and `telegram`.
const INTERVAL: Duration = Duration::from_secs(10);

/// Refuses to start on a schema that is newer than this binary knows.
/// A database that is down does not stop the start: the readiness check shows it (ADR 0025).
pub async fn check_schema(db: &Database) -> anyhow::Result<()> {
    match db.check_schema().await {
        Ok(()) => Ok(()),
        Err(error @ SchemaCheckError::TooNew { .. }) => {
            Err(error).context("start the newer image, or restore the previous schema")
        }
        Err(SchemaCheckError::Unavailable(error)) => {
            tracing::warn!(%error, "cannot check the schema version; the database is not available");
            Ok(())
        }
    }
}

/// The heartbeat of a role that has no loop of its own.
#[derive(Debug)]
pub struct Heartbeat {
    db: Database,
    id: Uuid,
    task: JoinHandle<()>,
}

impl Heartbeat {
    /// Writes the heartbeat of `role` now and then every ten seconds.
    pub fn start(db: &Database, role: Role) -> Self {
        let id = Uuid::now_v7();
        let writer = db.clone();
        let task = tokio::spawn(async move {
            let mut ticks = tokio::time::interval(INTERVAL);
            loop {
                ticks.tick().await;
                if let Err(error) = writer.record_heartbeat(id, role, &BuildInfo::CURRENT).await {
                    tracing::warn!(%error, "the heartbeat failed");
                }
            }
        });
        Self {
            db: db.clone(),
            id,
            task,
        }
    }

    /// Stops the heartbeat and removes its row, as a clean stop of the role does.
    pub async fn stop(self) {
        self.task.abort();
        if let Err(error) = self.db.remove_heartbeat(self.id).await {
            tracing::warn!(%error, "cannot remove the heartbeat");
        }
    }
}
