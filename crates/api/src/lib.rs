//! HTTP handlers, DTOs and the OpenAPI document.

mod contract;
mod events;
mod extract;
mod health;
mod problem;
mod request_id;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::middleware;
use ipnet::IpNet;
use tada_app::auth::Authenticator;
use tada_app::clock::Clock;
use tada_app::events::EventStore;
use tada_app::health::DependencyCheck;
use tada_app::problem::ProblemCode;
use utoipa::openapi::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::problem::ApiError;

/// All routes of the versioned API have this prefix (ADR 0017).
pub const API_PREFIX: &str = "/api/v1";

/// What the HTTP handlers need from the composition root.
#[derive(Debug, Clone)]
pub struct ApiState {
    /// The dependencies that `GET /readyz` checks.
    pub dependencies: Vec<Arc<dyn DependencyCheck>>,
    pub authenticator: Arc<dyn Authenticator>,
    pub events: Arc<dyn EventStore>,
    pub clock: Arc<dyn Clock>,
    /// The proxies whose `X-Request-Id` the server accepts (ADR 0035).
    pub trusted_proxies: Vec<IpNet>,
}

pub use contract::{PROBLEM_CODES_EXTENSION, problem_catalog};

/// The versioned API: its routes and its OpenAPI document.
fn api() -> (Router<ApiState>, OpenApi) {
    let (router, document) = OpenApiRouter::<ApiState>::new()
        .nest(API_PREFIX, events::routes())
        .split_for_parts();
    let problem_codes = events::problem_codes().into_iter().collect();
    (router, contract::complete(document, &problem_codes))
}

/// All routes of the `serve` process role.
pub fn router(state: ApiState) -> Router {
    let (api, _) = api();
    Router::new()
        .merge(api)
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_id::track,
        ))
        .merge(health::routes())
        .with_state(state)
}

/// The OpenAPI document of the versioned API.
pub fn openapi() -> OpenApi {
    api().1
}

async fn not_found() -> ApiError {
    ApiError::new(ProblemCode::NotFound)
}

/// Serves `router` on `listener` until `shutdown` completes, then waits for the open requests.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
