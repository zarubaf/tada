//! The `worker` process role: jobs and schedules (ADRs 0007 and 0054).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use jiff::Timestamp;
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, SmtpConfig, SmtpMailer};
use tada_app::clock::Clock;
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::mail::{MailTexts, Mailer};
use tada_app::outbound::SendOutbound;
use tada_app::public_url::PublicUrl;
use tada_app::rate_limit::{RateWindow, WINDOW};
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

/// The handler of each job kind that the worker runs. Tests use the same registration.
pub fn handlers(
    db: &Database,
    mailer: Arc<dyn Mailer>,
    texts: Arc<dyn MailTexts>,
    clock: Arc<dyn Clock>,
    public_url: PublicUrl,
) -> Handlers {
    let send = SendOutbound::new(Arc::new(db.clone()), mailer, texts, clock, public_url);
    Handlers::default().with(Arc::new(send))
}

/// The deletion of ended rate-limit counters, once in each rate-limit window (ADR 0065).
/// The counters that a sweep can delete change only when a new window starts.
#[derive(Debug, Default)]
struct CounterSweep {
    /// The start of the window of the last successful sweep.
    swept: Option<Timestamp>,
}

impl CounterSweep {
    /// The start of the window of `now`, if no sweep in this window succeeded.
    fn due(&self, now: Timestamp) -> Option<Timestamp> {
        let start = RateWindow::containing(now, WINDOW).start;
        (self.swept != Some(start)).then_some(start)
    }

    /// Records a successful sweep in the window that starts at `window`.
    fn done(&mut self, window: Timestamp) {
        self.swept = Some(window);
    }
}

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
    let handlers = handlers(
        &db,
        Arc::new(mailer),
        Arc::new(texts),
        Arc::new(SystemClock),
        public_url,
    );
    let worker_id = Uuid::now_v7();
    tracing::info!(%worker_id, "worker started");

    let stop = shutdown::signal_received();
    tokio::pin!(stop);
    let mut ticks = tokio::time::interval(POLL_INTERVAL);
    let mut sweep = CounterSweep::default();
    'outer: loop {
        tokio::select! {
            () = &mut stop => break,
            _ = ticks.tick() => {}
        }
        if let Err(error) = db.record_heartbeat(worker_id).await {
            tracing::warn!(%error, "the heartbeat failed");
        }
        // Sign-in requests delete ended rate-limit counters too, but only if one comes (ADR 0065).
        let now = SystemClock.now();
        if let Some(window) = sweep.due(now) {
            match db.delete_ended_rate_limit_counters(now).await {
                Ok(_) => sweep.done(window),
                Err(error) => tracing::warn!(%error, "cannot delete the ended rate-limit counters"),
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(time: &str) -> Timestamp {
        time.parse().unwrap()
    }

    #[test]
    fn the_sweep_is_due_once_per_window_and_again_after_a_failure() {
        let mut sweep = CounterSweep::default();
        let eight = at("2030-05-18T08:00:00Z");
        assert_eq!(sweep.due(at("2030-05-18T08:30:00Z")), Some(eight));
        // A failed sweep records nothing, so the next loop tries again.
        assert_eq!(sweep.due(at("2030-05-18T08:30:02Z")), Some(eight));
        sweep.done(eight);
        assert_eq!(sweep.due(at("2030-05-18T08:59:59Z")), None);
        assert_eq!(
            sweep.due(at("2030-05-18T09:00:01Z")),
            Some(at("2030-05-18T09:00:00Z"))
        );
    }
}
