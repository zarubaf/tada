//! `GET /healthz` and `GET /readyz` (ADR 0025). They are not part of the versioned API.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use tada_app::health::DependencyCheck;
use tokio::task::JoinSet;

/// A check that takes longer than this counts as a failure.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// The two routes over `dependencies`. Each process role with a listener serves them (ADR 0025).
pub fn routes(dependencies: Vec<Arc<dyn DependencyCheck>>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(Arc::from(dependencies))
}

/// The process runs.
async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// Each dependency responds. The response names no dependency; the log does.
async fn readyz(State(dependencies): State<Arc<[Arc<dyn DependencyCheck>]>>) -> StatusCode {
    let mut checks = JoinSet::new();
    for dependency in dependencies.iter() {
        let dependency = Arc::clone(dependency);
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
