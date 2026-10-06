//! The `worker` process role: jobs and schedules (ADR 0007). For now, it writes only its heartbeat.

use std::time::Duration;

use anyhow::Context;
use tada_store_pg::Database;
use uuid::Uuid;

use crate::settings::WorkerSettings;
use crate::shutdown;

const LOOP_INTERVAL: Duration = Duration::from_secs(10);

pub async fn run((database,): WorkerSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let worker_id = Uuid::now_v7();
    tracing::info!(%worker_id, "worker started");

    let stop = shutdown::signal_received();
    tokio::pin!(stop);
    let mut ticks = tokio::time::interval(LOOP_INTERVAL);
    loop {
        tokio::select! {
            () = &mut stop => break,
            _ = ticks.tick() => {
                if let Err(error) = db.record_heartbeat(worker_id).await {
                    tracing::warn!(%error, "the heartbeat failed");
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
