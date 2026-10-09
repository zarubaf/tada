//! Sign-in end to end: the magic-link request, the mail, the session cookie and sign-out (ADR 0008, ADR 0056).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::net::IpAddr;
use std::ops::Deref;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, header};
use jiff::SignedDuration;
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE, session_cookie};
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::OrganizationId;

const LINK: &str = "https://tada.example.org/sign-in/link#token=";
const SECOND: SignedDuration = SignedDuration::from_secs(1);

/// The shared test application with the steps of a sign-in.
struct App(MailApp);

impl Deref for App {
    type Target = MailApp;

    fn deref(&self) -> &MailApp {
        &self.0
    }
}

impl App {
    async fn start() -> Self {
        Self(MailApp::start().await)
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
            request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        }
        self.send(request.body(Body::from(body.to_string())).unwrap())
            .await
    }

    async fn get(&self, path: &str, cookie: Option<&str>) -> (Response<Body>, Value) {
        let mut request = Request::get(path);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        }
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn request_link(&self, email: &str) -> (Response<Body>, Value) {
        self.post("/api/v1/sign-in/requests", &json!({"email": email}), None)
            .await
    }

    /// Requests a magic link for `email`, sends it and reads the token from the link fragment.
    async fn magic_link(&self, email: &str) -> String {
        let (response, _) = self.request_link(email).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        self.mailed_token(LINK).await
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

/// A sign-in request for `email` from the client `peer`.
async fn request_link_from(router: &Router, peer: IpAddr, email: &str) -> (Response<Body>, Value) {
    let request = support::request_from(peer, Method::POST, "/api/v1/sign-in/requests")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"email": email}).to_string()))
        .unwrap();
    support::send(router, request).await
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

    support::logs::assert_clean(&[
        "nobody@example.org",
        "ben@example.org",
        "anna@example.org",
        "Anna@Example.org",
        "Anna Muster",
    ]);
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
        format!(
            "{SESSION_COOKIE}={cookie}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=7776000"
        )
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

    support::logs::assert_clean(&[
        &token,
        &cookie,
        "anna@example.org",
        "Anna Muster",
        "Firefox",
    ]);
    support::logs::assert_route_logged("/api/v1/sign-in/magic-link");
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

    // The failed redemptions are the error path of the sign-in.
    support::logs::assert_clean(&[&token, "unknown", "anna@example.org"]);
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
        format!("{SESSION_COOKIE}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
    );

    for path in ["/api/v1/session", "/api/v1/events"] {
        let (response, _) = app.get(path, Some(&cookie)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }

    let (response, _) = app.post("/api/v1/sign-out", &json!({}), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// A sign-in ends the session token that the request already sends (ASVS 7.2.4), also a token of
/// another user. Only the new token works afterwards.
#[tokio::test]
async fn a_sign_in_ends_the_session_that_the_request_sends() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let old = app.sign_in("anna@example.org").await;

    let token = app.magic_link("anna@example.org").await;
    let (response, _) = app
        .post(
            "/api/v1/sign-in/magic-link",
            &json!({"token": token}),
            Some(&old),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let new = session_cookie(&response).unwrap();
    assert_ne!(new, old);

    let (response, _) = app.get("/api/v1/session", Some(&old)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (response, _) = app.get("/api/v1/session", Some(&new)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

/// A failed sign-in keeps the session that the request sends.
#[tokio::test]
async fn a_failed_sign_in_keeps_the_session_that_the_request_sends() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let old = app.sign_in("anna@example.org").await;

    let (response, _) = app
        .post(
            "/api/v1/sign-in/magic-link",
            &json!({"token": "unknown"}),
            Some(&old),
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (response, _) = app.get("/api/v1/session", Some(&old)).await;
    assert_eq!(response.status(), StatusCode::OK);
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

#[tokio::test]
async fn the_31st_request_from_one_ip_address_is_rate_limited() {
    let app = App::start().await;
    for n in 0..30 {
        let (response, _) = request_link_from(
            &app.router,
            support::PEER,
            &format!("person{n}@example.org"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "request {}", n + 1);
    }
    let (response, problem) =
        request_link_from(&app.router, support::PEER, "person30@example.org").await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(problem["code"], "rate-limited");
    // The window started at the time of the test clock and lasts one hour.
    assert_eq!(response.headers()[header::RETRY_AFTER], "3600");

    support::logs::assert_clean(&["person0@example.org", "person30@example.org"]);
}

/// A host with IPv6 has a whole /64 and can send each request from another address of it.
#[tokio::test]
async fn the_31st_request_from_one_ipv6_network_is_rate_limited() {
    let app = App::start().await;
    for n in 0..30u16 {
        let peer = IpAddr::from([0x2001, 0xdb8, 0, 0x64, 0, 0, 0, n + 1]);
        let (response, _) =
            request_link_from(&app.router, peer, &format!("person{n}@example.org")).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "request {}", n + 1);
    }
    let peer = IpAddr::from([0x2001, 0xdb8, 0, 0x64, 0xffff, 0, 0, 1]);
    let (response, problem) = request_link_from(&app.router, peer, "person30@example.org").await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(problem["code"], "rate-limited");

    let other_network = IpAddr::from([0x2001, 0xdb8, 0, 0x65, 0, 0, 0, 1]);
    let (response, _) = request_link_from(&app.router, other_network, "person31@example.org").await;
    assert_eq!(response.status(), StatusCode::ACCEPTED, "another /64");
}
/// An attacker who knows the address of a member asks for links again and again. The member
/// still gets the answer of each request and finds a valid link in the inbox (ADR 0056).
#[tokio::test]
async fn requests_of_another_client_do_not_lock_a_member_out() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let attacker: IpAddr = "198.51.100.66".parse().unwrap();

    for n in 0..10 {
        let (response, _) = request_link_from(&app.router, attacker, "anna@example.org").await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "request {}", n + 1);
    }
    app.clock.advance(SignedDuration::from_mins(4));
    let (response, _) = request_link_from(&app.router, support::PEER, "anna@example.org").await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "the request of the member"
    );

    let token = app.mailed_token(LINK).await;
    let (response, _) = app.redeem(&token).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the link in the inbox works"
    );
}

/// The cooldown bounds the mails to one address, also with many clients and two processes.
#[tokio::test]
async fn one_address_gets_at_most_one_mail_in_each_cooldown_also_with_two_processes() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let other_process = support::session_router(&app.test, app.clock.clone());
    let processes = [&app.router, &other_process];

    // One request each minute for one hour, each from another client.
    for minute in 0..60u8 {
        let client = IpAddr::from([198, 51, 100, minute]);
        let (response, _) = request_link_from(
            processes[usize::from(minute) % 2],
            client,
            "Anna@Example.org",
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "minute {minute}");
        app.clock.advance(SignedDuration::from_mins(1));
    }
    app.run_jobs().await;
    assert_eq!(app.mailer.sent().len(), 6, "one mail in each 10 minutes");

    support::logs::assert_clean(&["anna@example.org", "198.51.100.1"]);
}

/// An operator who forgets `TADA_TRUSTED_PROXIES` behind a reverse proxy gives all clients one
/// rate limit. The server says so once, without the addresses.
#[tokio::test]
async fn x_forwarded_for_without_a_trusted_proxy_gives_one_warning() {
    let app = App::start().await;
    for _ in 0..2 {
        let request = support::request(Method::POST, "/api/v1/sign-in/requests")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", "198.51.100.23")
            .body(Body::from(json!({"email": "anna@example.org"}).to_string()))
            .unwrap();
        let (response, _) = support::send(&app.router, request).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }
    assert_eq!(
        support::logs::count_messages("WARN", "TADA_TRUSTED_PROXIES"),
        1
    );
    support::logs::assert_clean(&["198.51.100.23", "anna@example.org"]);
}

/// Common proxies pass the `X-Request-Id` of the client on. So the request ID of a trusted proxy goes
/// to the log line only, and the intent and the job of the request get the ID of the server.
#[tokio::test]
async fn the_request_id_of_a_trusted_proxy_goes_to_the_log_line_only() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    app.user("anna@example.org", &[testwil]).await;
    let mut state = support::session_state(&app.test, app.clock.clone());
    state.trusted_proxies = vec!["10.0.0.0/8".parse().unwrap()];
    let router = tada_api::router(state, None);
    let chosen = "01920000-0000-7000-8000-000000000001";

    let request = support::request_from(
        "10.0.0.5".parse().unwrap(),
        Method::POST,
        "/api/v1/sign-in/requests",
    )
    .header(header::CONTENT_TYPE, "application/json")
    .header("x-request-id", chosen)
    .header("x-forwarded-for", "198.51.100.23")
    .body(Body::from(json!({"email": "anna@example.org"}).to_string()))
    .unwrap();
    let (response, _) = support::send(&router, request).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    assert_ne!(request_id, chosen);

    for table in ["outbound_intent", "job"] {
        let stored: uuid::Uuid = app
            .test
            .scalar(&format!("SELECT request_id FROM {table}"))
            .await;
        assert_eq!(stored.to_string(), request_id, "{table}");
    }
    let line = support::logs::lines_with_message("request completed")
        .into_iter()
        .find(|line| line["request_id"] == request_id.as_str())
        .expect("the request line");
    assert_eq!(line["proxy_request_id"], chosen);
    support::logs::assert_clean(&["198.51.100.23", "anna@example.org"]);
}
