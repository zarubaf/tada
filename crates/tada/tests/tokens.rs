//! API tokens and the MCP switch over HTTP, with real sessions (ADR 0039, ADR 0045).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE};
use tada_app::caller::MemberCaller;
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, EventRole, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::event_members::add_event_member;

struct App {
    app: MailApp,
    testwil: OrganizationId,
}

struct Member {
    id: UserId,
    cookie: String,
}

impl App {
    async fn start() -> Self {
        let app = MailApp::start().await;
        let testwil = app.test.create_organization("testwil").await;
        Self { app, testwil }
    }

    async fn member(&self, name: &str, role: OrganizationRole) -> Member {
        let email = format!("{}@example.org", name.to_lowercase().replace(' ', "."));
        let id = self
            .app
            .test
            .create_user(
                &DisplayName::parse(name).unwrap(),
                &Email::parse(&email).unwrap(),
            )
            .await;
        self.app.test.add_membership(self.testwil, id, role).await;
        let cookie = self
            .app
            .test
            .sign_in(id, Some(self.testwil), self.app.clock.now())
            .await;
        Member { id, cookie }
    }

    async fn call(
        &self,
        cookie: Option<&str>,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let mut request = support::request(method, path);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = self.app.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, member: &Member, path: &str) -> (StatusCode, Value) {
        self.call(Some(&member.cookie), Method::GET, path, None)
            .await
    }

    async fn post(&self, member: &Member, path: &str, body: &Value) -> (StatusCode, Value) {
        self.call(Some(&member.cookie), Method::POST, path, Some(body))
            .await
    }

    /// The body of a valid creation request, one day from the test clock.
    fn new_token(&self, scope: &str) -> Value {
        let expires = self.app.clock.now() + jiff::SignedDuration::from_hours(24);
        json!({
            "name": "Claude Code",
            "scope": scope,
            "expires_at": expires.to_string(),
            "notice_version_confirmed": 1,
        })
    }
}

#[tokio::test]
async fn a_token_needs_the_confirmed_notice() {
    let app = App::start().await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;

    let (status, notice) = app.get(&anna, "/api/v1/token-notice").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notice, json!({"version": 1}));

    let mut body = app.new_token("read");
    body.as_object_mut()
        .unwrap()
        .remove("notice_version_confirmed");
    let (status, problem) = app.post(&anna, "/api/v1/tokens", &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["code"], "validation-failed");

    body["notice_version_confirmed"] = json!(0);
    let (status, problem) = app.post(&anna, "/api/v1/tokens", &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["code"], "validation-failed");
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/notice_version_confirmed", "code": "not-confirmed"}])
    );
}

#[tokio::test]
async fn a_pure_event_viewer_cannot_create_a_propose_token() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let event = app.app.test.create_event(app.testwil, "OPEN30").await;
    let database = &app.app.test.database;
    add_event_member(
        &MemberCaller::new(owner.id, app.testwil, OrganizationRole::Owner),
        event,
        anna.id,
        EventRole::EventViewer,
        database,
        database,
        &*app.app.clock,
    )
    .await
    .unwrap();

    let (status, problem) = app
        .post(&anna, "/api/v1/tokens", &app.new_token("propose"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");

    let (status, _) = app
        .post(&anna, "/api/v1/tokens", &app.new_token("read"))
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn the_secret_shows_once_and_a_revoked_token_stays_listed() {
    let app = App::start().await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let ben = app.member("Ben Beispiel", OrganizationRole::Member).await;

    let (status, created) = app
        .post(&anna, "/api/v1/tokens", &app.new_token("read"))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("tada_pat_"));
    assert_eq!(created["token"]["name"], "Claude Code");
    assert_eq!(created["token"]["scope"], "read");
    assert_eq!(created["token"]["notice_version"], 1);
    let id = created["token"]["id"].as_str().unwrap().to_owned();

    let (status, list) = app.get(&anna, "/api/v1/tokens").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!list.to_string().contains(&secret));
    assert!(!list.to_string().contains("secret"));
    assert_eq!(list["items"][0]["id"], id);
    assert!(list["items"][0]["revoked_at"].is_null());

    // Another member sees no token of Anna and cannot revoke it.
    let (_, list) = app.get(&ben, "/api/v1/tokens").await;
    assert_eq!(list["items"], json!([]));
    let path = format!("/api/v1/tokens/{id}/revoke");
    let (status, problem) = app.post(&ben, &path, &json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");

    let (status, _) = app.post(&anna, &path, &json!({})).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, list) = app.get(&anna, "/api/v1/tokens").await;
    assert!(!list["items"][0]["revoked_at"].is_null());

    app.app.test.assert_no_plaintext(&secret).await;
    support::logs::assert_clean(&[&secret, &anna.cookie, "anna.muster@example.org"]);
}

#[tokio::test]
async fn only_an_owner_switches_the_mcp_tokens() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;

    let (status, features) = app.get(&member, "/api/v1/organization/features").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        features["items"],
        json!([{"feature": "mcp-tokens", "enabled": true, "version": 1}])
    );

    let path = "/api/v1/organization/features/mcp-tokens/set";
    let off = json!({"enabled": false, "expected_version": 1});
    for caller in [&admin, &member] {
        let (status, problem) = app.post(caller, path, &off).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
    }

    let (status, state) = app.post(&owner, path, &off).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        state,
        json!({"feature": "mcp-tokens", "enabled": false, "version": 2})
    );

    let (status, problem) = app.post(&owner, path, &off).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");

    // With the switch off, nobody creates a token.
    let (status, problem) = app
        .post(&member, "/api/v1/tokens", &app.new_token("read"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");

    let (status, _) = app
        .post(
            &owner,
            path,
            &json!({"enabled": true, "expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_bearer_token_does_not_open_the_http_api() {
    let app = App::start().await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let (_, created) = app
        .post(&anna, "/api/v1/tokens", &app.new_token("read"))
        .await;
    let secret = created["secret"].as_str().unwrap();

    let request = support::request(Method::GET, "/api/v1/events")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .body(Body::empty())
        .unwrap();
    let (response, problem) = app.app.send(request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(problem["code"], "unauthenticated");
    support::logs::assert_clean(&[secret]);
}
