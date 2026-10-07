//! The `worker` process role: jobs and schedules (ADRs 0007 and 0054).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, SmtpConfig, SmtpMailer};
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SendOutbound;
use tada_store_pg::Database;
use uuid::Uuid;

use crate::settings::WorkerSettings;
use crate::shutdown;

/// The time between two looks at the queue when it is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Another worker can take a job over after this time. A handler must complete within it.
const LEASE: Duration = Duration::from_secs(300);
/// The time limit of one SMTP send. A send that exceeds it has an unknown outcome (ADR 0042).
const MAIL_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn run((database, public_url, mail, smtp): WorkerSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let mailer = SmtpMailer::new(SmtpConfig {
        host: smtp.host,
        port: smtp.port,
        tls: smtp.tls,
        credentials: smtp.credentials,
        from: mail.from,
        timeout: MAIL_TIMEOUT,
    })
    .context("invalid mail settings")?;
    let texts = FluentMailTexts::new().context("invalid mail texts")?;
    let host = public_url
        .url
        .host_str()
        .context("the public URL has no host")?;
    let send = SendOutbound::new(
        Arc::new(db.clone()),
        Arc::new(mailer),
        Arc::new(texts),
        Arc::new(SystemClock),
        public_url.url.as_str(),
        host,
    );
    let handlers = Handlers::default().with(Arc::new(send));
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
                Ok(Ran::CompletedWithWarning(job_id, warning)) => {
                    tracing::warn!(%job_id, warning = warning.0, "the job completed with a warning")
                }
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
