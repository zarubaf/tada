//! The shared support of the integration tests. Each test file uses `mod support;`.

// Each test file uses only some of the helpers.
#![allow(dead_code)]
// The helpers are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use axum::http::{Method, Request, header, request};
use tada_api::ApiState;
use tada_app::auth::Authenticator;
use tada_app::clock::Clock;
use tada_app::public_url::PublicUrl;
use tada_store_pg::testing::TestDatabase;

/// `TADA_PUBLIC_URL` of the tests.
pub const PUBLIC_URL: &str = "https://tada.example.org";

pub fn public_url() -> PublicUrl {
    PublicUrl::parse(PUBLIC_URL).unwrap()
}

/// The state of the API on the test database. A test changes a field if it needs another value.
pub fn api_state(
    test: &TestDatabase,
    authenticator: Arc<dyn Authenticator>,
    clock: Arc<dyn Clock>,
) -> ApiState {
    let database = Arc::new(test.database.clone());
    ApiState {
        dependencies: vec![database.clone()],
        authenticator,
        events: database.clone(),
        telegram: database.clone(),
        identity: database.clone(),
        sessions: database.clone(),
        sign_in: database,
        clock,
        trusted_proxies: Vec::new(),
        public_url: public_url(),
    }
}

/// A request as a browser on the page of tada sends it.
/// A state-changing request has the `Origin` of `PUBLIC_URL` (ADR 0008).
pub fn request(method: Method, path: &str) -> request::Builder {
    let changes_state = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let builder = Request::builder().method(method).uri(path);
    if changes_state {
        builder.header(header::ORIGIN, public_url().origin())
    } else {
        builder
    }
}
