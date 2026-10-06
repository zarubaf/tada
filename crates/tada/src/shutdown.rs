//! The stop signal of a process (ADR 0025).

use std::time::Duration;

use tokio::signal::unix::{SignalKind, signal};

/// The time that a process gives its current work after the stop signal.
/// The platform contract allows 25 seconds in total; the rest is for closing the connections.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(20);

/// Completes on `SIGTERM` or `SIGINT`.
pub async fn signal_received() {
    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(terminate) => terminate,
        Err(error) => {
            tracing::error!(%error, "cannot listen for SIGTERM; only SIGINT stops the process");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}
