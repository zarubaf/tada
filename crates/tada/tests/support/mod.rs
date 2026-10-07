//! The shared support of the integration tests. Each test file uses `mod support;`.

// Each test file uses only some of the helpers.
#![allow(dead_code)]
// The helpers are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::extract::ConnectInfo;
use axum::http::{Method, Request, header, request};
use secrecy::SecretString;
use tada_api::ApiState;
use tada_app::auth::Authenticator;
use tada_app::clock::Clock;
use tada_app::public_url::PublicUrl;
use tada_store_pg::PgSignInRequestStore;
use tada_store_pg::rate_limit::PgRateLimiter;
use tada_store_pg::testing::TestDatabase;

/// `TADA_PUBLIC_URL` of the tests.
pub const PUBLIC_URL: &str = "https://tada.example.org";

pub fn public_url() -> PublicUrl {
    PublicUrl::parse(PUBLIC_URL).unwrap()
}

/// The address of the client of each request, from a range for documentation (RFC 5737).
pub const PEER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10));

/// The rate-limit key of each `serve` process of the tests.
pub fn rate_limiter() -> PgRateLimiter {
    PgRateLimiter::new(SecretString::from("test rate limit key"))
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
        sign_in: database.clone(),
        sign_in_requests: Arc::new(PgSignInRequestStore::new(
            test.database.clone(),
            rate_limiter(),
        )),
        clock,
        trusted_proxies: Vec::new(),
        public_url: public_url(),
    }
}

/// A request as a browser on the page of tada sends it, from `PEER`.
/// A state-changing request has the `Origin` of `PUBLIC_URL` (ADR 0008).
pub fn request(method: Method, path: &str) -> request::Builder {
    request_from(PEER, method, path)
}

/// A request from the client `peer`, without a proxy.
pub fn request_from(peer: IpAddr, method: Method, path: &str) -> request::Builder {
    let changes_state = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .extension(ConnectInfo(SocketAddr::new(peer, 40000)));
    if changes_state {
        builder.header(header::ORIGIN, public_url().origin())
    } else {
        builder
    }
}
