//! The `worker` process role: jobs and schedules (ADRs 0007 and 0054).

use std::time::Duration;

use anyhow::Context;
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_store_pg::Database;
use uuid::Uuid;

use crate::settings::WorkerSettings;
use crate::shutdown;

/// The time between two looks at the queue when it is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Another worker can take a job over after this time. A handler must complete within it.
const LEASE: Duration = Duration::from_secs(300);

pub async fn run((database, ..): WorkerSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    // The first job kinds come with Slice 1, for example the owner invitation of ADR 0036.
    let handlers = Handlers::default();
    let worker_id = Uuid::now_v7();
    tracing::info!(%worker_id, "worker started");

    let stop = shutdown::signal_received();
    tokio::pin!(stop);
    let mut ticks = tokio::time::interval(POLL_INTERVAL);
    'outer: loop {
        tokio::select! {
            () = &mut stop => break,
            _ = ticks.tick() => {}
        }
        if let Err(error) = db.record_heartbeat(worker_id).await {
            tracing::warn!(%error, "the heartbeat failed");
        }
        // Run the due jobs one at a time. The stop signal ends the loop between two jobs.
        loop {
            let ran = tokio::select! {
                () = &mut stop => break 'outer,
                ran = run_next(&db, &handlers, worker_id, LEASE) => ran,
            };
            match ran {
                Ok(Ran::Idle) => break,
                Ok(Ran::Completed(job_id)) => tracing::info!(%job_id, "the job completed"),
                Ok(Ran::Failed(job_id)) => tracing::warn!(%job_id, "the job failed"),
                Ok(Ran::LeaseLost(job_id)) => {
                    tracing::warn!(%job_id, "the lease expired before the job completed")
                }
                Err(error) => {
                    tracing::warn!(%error, "the queue is unavailable");
                    break;
                }
            }
        }
    }

    tracing::info!("worker stops");
    if let Err(error) = db.remove_heartbeat(worker_id).await {
        tracing::warn!(%error, "cannot remove the heartbeat");
    }
    db.close().await;
    Ok(())
}
