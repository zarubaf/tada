//! HTTP handlers, DTOs and the OpenAPI document.

mod contract;
mod events;
mod extract;
mod health;
mod problem;
mod request_id;
mod telegram;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::any;
use ipnet::IpNet;
use tada_app::auth::Authenticator;
use tada_app::clock::Clock;
use tada_app::events::EventStore;
use tada_app::health::DependencyCheck;
use tada_app::problem::ProblemCode;
use tada_app::telegram::TelegramLinks;
use tower_http::services::{ServeDir, ServeFile};
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
    pub telegram: Arc<dyn TelegramLinks>,
    pub clock: Arc<dyn Clock>,
    /// The proxies whose `X-Request-Id` the server accepts (ADR 0035).
    pub trusted_proxies: Vec<IpNet>,
}

pub use contract::{PROBLEM_CODES_EXTENSION, problem_catalog};

/// The versioned API: its routes and its OpenAPI document.
fn api() -> (Router<ApiState>, OpenApi) {
    let (router, document) = OpenApiRouter::<ApiState>::new()
        .nest(API_PREFIX, events::routes().merge(telegram::routes()))
        .split_for_parts();
    let problem_codes = events::problem_codes()
        .into_iter()
        .chain(telegram::problem_codes())
        .collect();
    (router, contract::complete(document, &problem_codes))
}

/// All routes of the `serve` process role.
///
/// With `web_root`, the server also delivers the built web client from this folder (ADR 0005).
/// A path outside `/api` that is not a file gets `index.html`, so that the client handles its own routes.
pub fn router(state: ApiState, web_root: Option<&Path>) -> Router {
    let (api, _) = api();
    let router = Router::new()
        .merge(api)
        .route("/api/{*path}", any(not_found));
    let router = match web_root {
        Some(root) => router.fallback_service(
            ServeDir::new(root).fallback(ServeFile::new(root.join("index.html"))),
        ),
        None => router.fallback(not_found),
    };
    router
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

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tada_app::auth::{AuthenticationError, Credential};
    use tada_app::caller::MemberCaller;
    use tower::ServiceExt;

    use super::*;

    #[derive(Debug)]
    struct NoCaller;

    #[async_trait::async_trait]
    impl Authenticator for NoCaller {
        async fn authenticate(
            &self,
            _credential: Option<Credential<'_>>,
        ) -> Result<MemberCaller, AuthenticationError> {
            Err(AuthenticationError::Unauthenticated)
        }
    }

    #[derive(Debug)]
    struct NoClock;

    impl Clock for NoClock {
        fn now(&self) -> jiff::Timestamp {
            jiff::Timestamp::UNIX_EPOCH
        }
    }

    #[derive(Debug)]
    struct NoEvents;

    #[async_trait::async_trait]
    impl EventStore for NoEvents {
        async fn insert(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::domain::events::Event,
        ) -> Result<tada_app::events::Inserted, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn get(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<Option<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn list(
            &self,
            _: tada_app::caller::OrgScope,
            _: Option<&tada_app::events::EventCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn list_of_member(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: Option<&tada_app::events::EventCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoTelegram;

    #[async_trait::async_trait]
    impl TelegramLinks for NoTelegram {
        async fn create_code(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: jiff::Timestamp,
        ) -> Result<String, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn claim(
            &self,
            _: &str,
            _: tada_app::telegram::TelegramUserId,
            _: &tada_app::telegram::TelegramName,
            _: jiff::Timestamp,
        ) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn requests(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: jiff::Timestamp,
        ) -> Result<Vec<tada_app::telegram::LinkRequest>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn confirm(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: uuid::Uuid,
            _: jiff::Timestamp,
        ) -> Result<tada_app::telegram::Confirmed, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn record_update(&self, _: i64) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    fn state() -> ApiState {
        ApiState {
            dependencies: Vec::new(),
            authenticator: Arc::new(NoCaller),
            events: Arc::new(NoEvents),
            telegram: Arc::new(NoTelegram),
            clock: Arc::new(NoClock),
            trusted_proxies: Vec::new(),
        }
    }

    async fn get(router: &Router, path: &str) -> (StatusCode, String) {
        let response = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_the_web_client_and_keeps_problems_for_the_api() {
        let web = tempfile::tempdir().unwrap();
        fs::write(web.path().join("index.html"), "<html>tada</html>").unwrap();
        fs::create_dir(web.path().join("assets")).unwrap();
        fs::write(web.path().join("assets/app.js"), "console.log(1)").unwrap();
        let router = router(state(), Some(web.path()));

        assert_eq!(
            get(&router, "/").await,
            (StatusCode::OK, "<html>tada</html>".to_owned())
        );
        assert_eq!(
            get(&router, "/assets/app.js").await,
            (StatusCode::OK, "console.log(1)".to_owned())
        );
        assert_eq!(
            get(&router, "/events/FLY28").await,
            (StatusCode::OK, "<html>tada</html>".to_owned())
        );

        let (status, body) = get(&router, "/api/v1/nothing").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("\"code\":\"not-found\""));
        let (status, _) = get(&router, "/api/v1/events").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn without_the_web_client_each_unknown_path_is_a_problem() {
        let router = router(state(), None);
        let (status, body) = get(&router, "/events").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("\"code\":\"not-found\""));
    }
}
