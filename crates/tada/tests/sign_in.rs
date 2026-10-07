//! Sign-in end to end: the magic-link request, the mail, the session cookie and sign-out (ADR 0008, ADR 0056).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, header};
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::OrganizationId;
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SendOutbound;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;
use uuid::Uuid;

const LEASE: Duration = Duration::from_secs(60);
const LINK: &str = "https://tada.example.org/sign-in/link#token=";
const COOKIE: &str = "__Host-tada-session";
const SECOND: SignedDuration = SignedDuration::from_secs(1);

/// A clock that the test moves. The API, the authenticator and the worker share it.
#[derive(Debug)]
struct TestClock(Mutex<Timestamp>);

impl TestClock {
    fn advance(&self, duration: SignedDuration) {
        let mut now = self.0.lock().unwrap();
        *now += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

struct App {
    router: axum::Router,
    test: TestDatabase,
    mailer: Arc<MemoryMailer>,
    handlers: Handlers,
    clock: Arc<TestClock>,
}

impl App {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let database = Arc::new(test.database.clone());
        let clock = Arc::new(TestClock(Mutex::new(
            "2030-05-18T08:00:00Z".parse().unwrap(),
        )));
        let authenticator = Arc::new(SessionAuthenticator::new(
            database.clone(),
            database.clone(),
            clock.clone(),
        ));
        let router = tada_api::router(
            support::api_state(&test, authenticator, clock.clone()),
            None,
        );
        let mailer = Arc::new(MemoryMailer::new());
        let handler = SendOutbound::new(
            database,
            mailer.clone(),
            Arc::new(FluentMailTexts::new().unwrap()),
            clock.clone(),
            support::public_url(),
        );
        Self {
            router,
            test,
            mailer,
            handlers: Handlers::default().with(Arc::new(handler)),
            clock,
        }
    }

    /// Creates a user with a membership in each of `organizations`.
    async fn user(&self, email: &str, organizations: &[OrganizationId]) {
        let user = self
            .test
            .create_user(
                &DisplayName::parse("Anna Muster").unwrap(),
                &Email::parse(email).unwrap(),
            )
            .await;
        for organization in organizations {
            self.test
                .add_membership(*organization, user, OrganizationRole::Member)
                .await;
        }
    }

    /// Sends a request. Each response must forbid the referrer (ADR 0008).
    async fn send(&self, request: Request<Body>) -> (Response<Body>, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
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

    async fn post(
        &self,
        path: &str,
        body: &Value,
        cookie: Option<&str>,
    ) -> (Response<Body>, Value) {
        let mut request = support::request(Method::POST, path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, "Firefox");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, format!("{COOKIE}={cookie}"));
        }
        self.send(request.body(Body::from(body.to_string())).unwrap())
            .await
    }

    async fn get(&self, path: &str, cookie: Option<&str>) -> (Response<Body>, Value) {
        let mut request = Request::get(path);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, format!("{COOKIE}={cookie}"));
        }
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn request_link(&self, email: &str) -> (Response<Body>, Value) {
        self.post("/api/v1/sign-in/requests", &json!({"email": email}), None)
            .await
    }

    /// Runs the worker until no job is left.
    async fn run_jobs(&self) {
        while run_next(&self.test.database, &self.handlers, Uuid::now_v7(), LEASE)
            .await
            .unwrap()
            != Ran::Idle
        {}
    }

    /// Requests a magic link for `email`, sends it and reads the token from the link fragment.
    async fn magic_link(&self, email: &str) -> String {
        let (response, _) = self.request_link(email).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        self.run_jobs().await;
        let sent = self.mailer.sent();
        let text = &sent.last().unwrap().text;
        let start = text.find(LINK).unwrap() + LINK.len();
        text[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    }

    async fn redeem(&self, token: &str) -> (Response<Body>, Value) {
        self.post("/api/v1/sign-in/magic-link", &json!({"token": token}), None)
            .await
    }

    /// Signs `email` in and returns the value of the session cookie.
    async fn sign_in(&self, email: &str) -> String {
        let token = self.magic_link(email).await;
        let (response, _) = self.redeem(&token).await;
        assert_eq!(response.status(), StatusCode::OK);
        session_cookie(&response).unwrap()
    }
}

/// The value of the session cookie that the response sets.
fn session_cookie(response: &Response<Body>) -> Option<String> {
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)?
        .to_str()
        .unwrap();
    let value = cookie.strip_prefix(&format!("{COOKIE}="))?;
    Some(value.split(';').next().unwrap().to_owned())
}

#[tokio::test]
async fn a_sign_in_request_gets_the_same_answer_for_each_address_and_only_a_member_gets_mail() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    app.user("ben@example.org", &[]).await;

    let mut answers = Vec::new();
    for email in [
        "nobody@example.org",
        "ben@example.org",
        "Anna@Example.org",
        "no address",
    ] {
        let (response, body) = app.request_link(email).await;
        answers.push((response.status(), body));
    }
    assert_eq!(answers, vec![(StatusCode::ACCEPTED, Value::Null); 4]);

    app.run_jobs().await;
    let sent = app.mailer.sent();
    assert_eq!(sent.len(), 1, "only the member gets a mail");
    assert_eq!(sent[0].to, Email::parse("anna@example.org").unwrap());
}

#[tokio::test]
async fn a_member_signs_in_with_the_magic_link_in_the_organization() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;

    let token = app.magic_link("anna@example.org").await;
    let (response, session) = app.redeem(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let set_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    let cookie = session_cookie(&response).unwrap();
    assert_eq!(
        set_cookie,
        format!("{COOKIE}={cookie}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=7776000")
    );
    app.test.assert_no_plaintext(&cookie).await;
    app.test.assert_no_plaintext(&token).await;

    let organization = json!({
        "organization_id": testwil.as_uuid(),
        "name": "testwil",
        "role": "member",
    });
    assert_eq!(session["display_name"], "Anna Muster");
    assert_eq!(session["organization"], organization);
    assert_eq!(session["memberships"], json!([organization]));

    let (response, read) = app.get("/api/v1/session", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read, session);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let (response, _) = app.get("/api/v1/events", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_member_of_two_organizations_chooses_one() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let musterhausen = app.test.create_organization("musterhausen").await;
    let other = app.test.create_organization("andere").await;
    app.user("anna@example.org", &[testwil, musterhausen]).await;
    let cookie = app.sign_in("anna@example.org").await;

    let (response, problem) = app.get("/api/v1/events", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "organization-required");
    let (response, session) = app.get("/api/v1/session", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(session.get("organization").is_none(), "{session}");
    assert_eq!(session["memberships"].as_array().unwrap().len(), 2);

    let (response, problem) = app
        .post(
            "/api/v1/session/organization",
            &json!({"organization_id": other.as_uuid()}),
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");

    let (response, session) = app
        .post(
            "/api/v1/session/organization",
            &json!({"organization_id": musterhausen.as_uuid()}),
            Some(&cookie),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        session["organization"]["organization_id"],
        json!(musterhausen.as_uuid())
    );
    let (response, _) = app.get("/api/v1/events", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_magic_link_works_once() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;

    let token = app.magic_link("anna@example.org").await;
    let (first, _) = app.redeem(&token).await;
    assert_eq!(first.status(), StatusCode::OK);
    let (second, problem) = app.redeem(&token).await;
    assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
    assert!(session_cookie(&second).is_none());

    let (_, unknown) = app.redeem("unknown").await;
    assert_eq!(
        without_request_id(problem),
        without_request_id(unknown),
        "no detail difference"
    );
}

/// A problem without the fields that differ for each request.
fn without_request_id(mut problem: Value) -> Value {
    problem.as_object_mut().unwrap().remove("request_id");
    problem.as_object_mut().unwrap().remove("instance");
    problem
}

#[tokio::test]
async fn a_magic_link_expires_after_15_minutes() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;

    let token = app.magic_link("anna@example.org").await;
    app.clock.advance(SignedDuration::from_mins(15) - SECOND);
    let (response, _) = app.redeem(&token).await;
    assert_eq!(response.status(), StatusCode::OK);

    let token = app.magic_link("anna@example.org").await;
    app.clock.advance(SignedDuration::from_mins(15));
    let (response, problem) = app.redeem(&token).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(problem["code"], "unauthenticated");
}

#[tokio::test]
async fn a_get_request_on_the_link_does_not_sign_in() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let token = app.magic_link("anna@example.org").await;

    for path in [
        format!("/sign-in/link?token={token}"),
        format!("/api/v1/sign-in/magic-link?token={token}"),
    ] {
        let (response, _) = app.get(&path, None).await;
        assert_ne!(response.status(), StatusCode::OK, "{path}");
        assert!(session_cookie(&response).is_none(), "{path}");
    }

    let (response, _) = app.redeem(&token).await;
    assert_eq!(response.status(), StatusCode::OK, "the token is unused");
}

#[tokio::test]
async fn a_session_ends_after_14_idle_days() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let cookie = app.sign_in("anna@example.org").await;

    app.clock
        .advance(SignedDuration::from_hours(14 * 24) - SECOND);
    let (response, _) = app.get("/api/v1/events", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);

    app.clock.advance(SignedDuration::from_hours(14 * 24));
    let (response, problem) = app.get("/api/v1/events", Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(problem["code"], "unauthenticated");
}

#[tokio::test]
async fn after_sign_out_the_old_cookie_is_unauthenticated() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let cookie = app.sign_in("anna@example.org").await;

    let (response, body) = app
        .post("/api/v1/sign-out", &json!({}), Some(&cookie))
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);
    assert_eq!(
        response.headers()[header::SET_COOKIE],
        format!("{COOKIE}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
    );

    for path in ["/api/v1/session", "/api/v1/events"] {
        let (response, _) = app.get(path, Some(&cookie)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }

    let (response, _) = app.post("/api/v1/sign-out", &json!({}), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn each_response_forbids_the_referrer() {
    let app = App::start().await;
    // `App::send` checks the header of each response, also of problems and of the web routes.
    for path in [
        "/api/v1/session",
        "/api/v1/nothing",
        "/sign-in/link",
        "/healthz",
    ] {
        app.get(path, None).await;
    }
    app.request_link("nobody@example.org").await;
    app.redeem("unknown").await;
}
