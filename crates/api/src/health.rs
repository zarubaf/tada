//! `GET /healthz` and `GET /readyz` (ADR 0025). They are not part of the versioned API.

use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use tokio::task::JoinSet;

use crate::ApiState;

/// A check that takes longer than this counts as a failure.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
}

/// The process runs.
async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// Each dependency responds. The response names no dependency; the log does.
async fn readyz(State(state): State<ApiState>) -> StatusCode {
    let mut checks = JoinSet::new();
    for dependency in state.dependencies {
        checks.spawn(async move {
            match tokio::time::timeout(CHECK_TIMEOUT, dependency.check()).await {
                Ok(Ok(())) => true,
                Ok(Err(error)) => {
                    tracing::warn!(dependency = dependency.name(), error = %error.0, "dependency check failed");
                    false
                }
                Err(_) => {
                    tracing::warn!(dependency = dependency.name(), "dependency check timed out");
                    false
                }
            }
        });
    }
    let results = checks.join_all().await;
    if results.into_iter().all(|ready| ready) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
