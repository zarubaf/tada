//! HTTP handlers, DTOs and the OpenAPI document.

mod health;

use std::sync::Arc;

use axum::Router;
use tada_app::health::DependencyCheck;

/// What the HTTP handlers need from the composition root.
#[derive(Debug, Clone)]
pub struct ApiState {
    /// The dependencies that `GET /readyz` checks.
    pub dependencies: Vec<Arc<dyn DependencyCheck>>,
}

/// All routes of the `serve` process role.
pub fn router(state: ApiState) -> Router {
    health::routes().with_state(state)
}

/// Serves `router` on `listener` until `shutdown` completes, then waits for the open requests.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
}
