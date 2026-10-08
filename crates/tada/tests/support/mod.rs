//! The shared support of the integration tests. Each test file uses `mod support;`.

// Each test file uses only some of the helpers.
#![allow(dead_code)]
// The helpers are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

pub mod logs;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, Response, header, request};
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use serde_json::Value;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_api::ApiState;
pub use tada_api::SESSION_COOKIE;
use tada_app::auth::Authenticator;
use tada_app::clock::Clock;
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SendOutbound;
use tada_app::public_url::PublicUrl;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::PgSignInRequestStore;
use tada_store_pg::rate_limit::PgRateLimiter;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;
use uuid::Uuid;

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
        event_members: database.clone(),
        members: database.clone(),
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

/// A clock that the test moves. The API, the authenticator and the worker share it.
#[derive(Debug)]
pub struct TestClock(Mutex<Timestamp>);

impl TestClock {
    pub fn new(start: Timestamp) -> Self {
        Self(Mutex::new(start))
    }

    pub fn advance(&self, duration: SignedDuration) {
        let mut now = self.0.lock().unwrap();
        *now += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

/// The router of one `serve` process on the test database, with session sign-in.
pub fn session_router(test: &TestDatabase, clock: Arc<dyn Clock>) -> Router {
    let database = Arc::new(test.database.clone());
    let authenticator = Arc::new(SessionAuthenticator::new(
        database.clone(),
        database,
        clock.clone(),
    ));
    tada_api::router(api_state(test, authenticator, clock), None)
}

/// Sends a request and returns the response with its JSON body.
/// Each response must forbid the referrer (ADR 0008).
pub async fn send(router: &Router, request: Request<Body>) -> (Response<Body>, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers()[header::REFERRER_POLICY],
        "no-referrer",
        "each response forbids the referrer"
    );
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (Response::from_parts(parts, Body::empty()), value)
}

/// The value of the session cookie that the response sets.
pub fn session_cookie(response: &Response<Body>) -> Option<String> {
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)?
        .to_str()
        .unwrap();
    let value = cookie.strip_prefix(&format!("{SESSION_COOKIE}="))?;
    Some(value.split(';').next().unwrap().to_owned())
}

/// The lease of a job that the worker of the tests runs.
const LEASE: Duration = Duration::from_secs(60);

/// The API with session sign-in, the worker that sends mail into memory, and a clock that the
/// test moves. Tests use it to sign in through the mail.
pub struct MailApp {
    pub router: Router,
    pub test: TestDatabase,
    pub mailer: Arc<MemoryMailer>,
    pub handlers: Handlers,
    pub clock: Arc<TestClock>,
}

impl MailApp {
    /// Starts a test database and the API. The clock starts at 2030-05-18 08:00 UTC.
    pub async fn start() -> Self {
        logs::install();
        let test = TestDatabase::start().await;
        let clock = Arc::new(TestClock::new("2030-05-18T08:00:00Z".parse().unwrap()));
        let router = session_router(&test, clock.clone());
        let mailer = Arc::new(MemoryMailer::new());
        let handler = SendOutbound::new(
            Arc::new(test.database.clone()),
            mailer.clone(),
            Arc::new(FluentMailTexts::new().unwrap()),
            clock.clone(),
            public_url(),
        );
        Self {
            router,
            test,
            mailer,
            handlers: Handlers::default().with(Arc::new(handler)),
            clock,
        }
    }

    /// Sends a request. Each response must forbid the referrer (ADR 0008).
    pub async fn send(&self, request: Request<Body>) -> (Response<Body>, Value) {
        send(&self.router, request).await
    }

    /// Runs the worker until no job is left.
    pub async fn run_jobs(&self) {
        while run_next(&self.test.database, &self.handlers, Uuid::now_v7(), LEASE)
            .await
            .unwrap()
            != Ran::Idle
        {}
    }

    /// Runs the worker, then reads the token from the fragment of the link of the last mail.
    /// `link_prefix` is the link up to the token, for example `.../sign-in/link#token=`.
    pub async fn mailed_token(&self, link_prefix: &str) -> String {
        self.run_jobs().await;
        let sent = self.mailer.sent();
        let text = &sent.last().unwrap().text;
        let start = text.find(link_prefix).unwrap() + link_prefix.len();
        text[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    }
}
