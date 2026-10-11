//! The `worker` process role: jobs and schedules (ADRs 0007 and 0054).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, SmtpConfig, SmtpMailer};
use tada_app::blobs::BlobStore;
use tada_app::build_info::{BuildInfo, Role};
use tada_app::caller::{JobRunner, ServiceCaller};
use tada_app::clock::Clock;
use tada_app::housekeeping::HousekeepingStore;
use tada_app::jobs::{Handlers, Ran, ScheduleRunner, run_next};
use tada_app::mail::{MailTexts, Mailer};
use tada_app::orphans;
use tada_app::outbound::SendOutbound;
use tada_app::public_url::PublicUrl;
use tada_app::rate_limit::WINDOW;
use tada_app::redaction::WriteRedactionJournal;
use tada_app::retention;
use tada_store_pg::Database;
use uuid::Uuid;

use crate::heartbeat;
use crate::settings::WorkerSettings;
use crate::shutdown;

mod sweep;

use sweep::Sweep;

/// The time between two looks at the queue when it is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Another worker can take a job over after this time. A handler must complete within it.
const LEASE: Duration = Duration::from_secs(300);
/// The time limit of one SMTP send. A send that exceeds it has an unknown outcome (ADR 0042).
const MAIL_TIMEOUT: Duration = Duration::from_secs(30);

/// The handler of each job kind that the worker runs. Tests use the same registration.
pub fn handlers(
    db: &Database,
    mailer: Arc<dyn Mailer>,
    texts: Arc<dyn MailTexts>,
    clock: Arc<dyn Clock>,
    public_url: PublicUrl,
    blobs: Arc<dyn BlobStore>,
) -> Handlers {
    let send = SendOutbound::new(Arc::new(db.clone()), mailer, texts, clock, public_url);
    let journal = WriteRedactionJournal::new(Arc::new(db.clone()), blobs);
    Handlers::default()
        .with(Arc::new(send))
        .with(Arc::new(journal))
}

pub async fn run(
    // The AI settings are read and checked at start; the AI-PM digest uses them (ADR 0077).
    (database, public_url, mail, smtp, storage, _ai): WorkerSettings,
) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    heartbeat::check_schema(&db).await?;
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
    let blobs = Arc::new(storage.open());
    let handlers = handlers(
        &db,
        Arc::new(mailer),
        Arc::new(texts),
        Arc::new(SystemClock),
        public_url,
        blobs.clone(),
    );
    let worker_id = Uuid::now_v7();
    tracing::info!(%worker_id, "worker started");

    let stop = shutdown::signal_received();
    tokio::pin!(stop);
    let mut ticks = tokio::time::interval(POLL_INTERVAL);
    // The counters that a sweep can delete change only when a new window starts (ADR 0065).
    let mut counter_sweep = Sweep::every(WINDOW);
    let mut orphan_sweep = Sweep::daily();
    let mut housekeeping_sweep = Sweep::daily();
    let mut retention_sweep = Sweep::daily();
    'outer: loop {
        tokio::select! {
            () = &mut stop => break,
            _ = ticks.tick() => {}
        }
        if let Err(error) = db
            .record_heartbeat(worker_id, Role::Worker, &BuildInfo::CURRENT)
            .await
        {
            tracing::warn!(%error, "the heartbeat failed");
        }
        // Sign-in requests delete ended rate-limit counters too, but only if one comes (ADR 0065).
        let now = SystemClock.now();
        if let Some(window) = counter_sweep.due(now) {
            match db.delete_ended_rate_limit_counters(now).await {
                Ok(_) => counter_sweep.done(window),
                Err(error) => {
                    counter_sweep.failed(now);
                    tracing::warn!(%error, "cannot delete the ended rate-limit counters");
                }
            }
        }
        if let Some(day) = orphan_sweep.due(now) {
            match orphans::sweep(blobs.as_ref(), &db, now).await {
                Ok(report) => {
                    tracing::info!(
                        deleted = report.deleted,
                        aborted = report.aborted,
                        kept = report.kept,
                        "the sweep of orphan objects completed"
                    );
                    orphan_sweep.done(day);
                }
                Err(error) => {
                    orphan_sweep.failed(now);
                    tracing::warn!(%error, "cannot sweep the orphan objects");
                }
            }
        }
        if let Some(day) = housekeeping_sweep.due(now) {
            match db.run(&ServiceCaller::<JobRunner>::new(), now).await {
                Ok(report) => {
                    tracing::info!(?report, "the housekeeping completed");
                    housekeeping_sweep.done(day);
                }
                Err(error) => {
                    housekeeping_sweep.failed(now);
                    tracing::warn!(%error, "cannot run the housekeeping");
                }
            }
        }
        if let Some(day) = retention_sweep.due(now) {
            match retention::run(&db, &db, blobs.as_ref(), now).await {
                Ok(report) if report.completed() => {
                    tracing::info!(
                        events = report.events,
                        sources = report.sources,
                        "the retention job completed"
                    );
                    retention_sweep.done(day);
                }
                Ok(report) => {
                    // The redacted rows are done. The next attempt finds the failed targets only.
                    for failed in &report.failed {
                        tracing::warn!(
                            organization_id = %failed.organization_id,
                            kind = failed.target.kind.as_str(),
                            id = %failed.target.id,
                            code = failed.code.as_str(),
                            "cannot redact a target at the end of its retention period"
                        );
                    }
                    tracing::warn!(
                        events = report.events,
                        sources = report.sources,
                        failed = report.failed.len(),
                        "the retention job completed with failed targets"
                    );
                    retention_sweep.failed(now);
                }
                Err(error) => {
                    retention_sweep.failed(now);
                    tracing::warn!(%error, "cannot find the targets of the retention job");
                }
            }
        }
        // Durable schedules add their jobs before the drain, so a due run starts in this tick (ADR 0076).
        // An organization with the default policy gets its schedule first.
        if let Err(error) = db.ensure_default_schedules().await {
            tracing::warn!(%error, "cannot add the default schedules");
        }
        match db.start_due().await {
            Ok(step) => {
                if step.started > 0 {
                    tracing::info!(started = step.started, "the schedules started jobs");
                }
                for (schedule_id, kind) in &step.skipped {
                    tracing::warn!(%schedule_id, kind, "cannot read a schedule; it moved one day forward");
                }
            }
            Err(error) => tracing::warn!(%error, "cannot start the due schedules"),
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
