//! The privacy notice of the organization over HTTP, with real sessions (ADR 0045).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE};
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::OrganizationId;

const PATH: &str = "/api/v1/organization/privacy-notice";
const SET: &str = "/api/v1/organization/privacy-notice/set";
const LINK: &str = "https://tada.example.org/invitation#token=";

struct App {
    app: MailApp,
    testwil: OrganizationId,
}

impl App {
    async fn start() -> Self {
        let app = MailApp::start().await;
        let testwil = app.test.create_organization("testwil").await;
        Self { app, testwil }
    }

    /// The session cookie of a new member.
    async fn member(&self, name: &str, role: OrganizationRole) -> String {
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
        self.app
            .test
            .sign_in(id, Some(self.testwil), self.app.clock.now())
            .await
    }

    async fn call(
        &self,
        cookie: &str,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let request = support::request(method, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = self.app.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn set(&self, cookie: &str, markdown: Value, version: i64) -> (StatusCode, Value) {
        let body = json!({"markdown": markdown, "expected_version": version});
        self.call(cookie, Method::POST, SET, Some(&body)).await
    }
}

#[tokio::test]
async fn an_owner_sets_the_notice_and_a_member_reads_it() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;

    // Without a text, the client shows the template.
    let (status, notice) = app.call(&member, Method::GET, PATH, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notice, json!({"markdown": null, "version": 1}));

    let text = "# Datenschutz\n\nDer Verein Testwil verarbeitet Daten.";
    let (status, notice) = app.set(&owner, json!(text), 1).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notice, json!({"markdown": text, "version": 2}));

    let (_, notice) = app.call(&member, Method::GET, PATH, None).await;
    assert_eq!(notice, json!({"markdown": text, "version": 2}));

    // The owner goes back to the template with a null text.
    let (status, notice) = app.set(&owner, Value::Null, 2).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notice, json!({"markdown": null, "version": 3}));
}

#[tokio::test]
async fn only_an_owner_sets_the_notice() {
    let app = App::start().await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;
    for cookie in [&admin, &member] {
        let (status, problem) = app.set(cookie, json!("Text"), 1).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
    }
    let (_, notice) = app.call(&member, Method::GET, PATH, None).await;
    assert_eq!(notice["markdown"], Value::Null);
}

#[tokio::test]
async fn a_stale_version_conflicts_and_a_bad_text_is_invalid() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;

    let (status, _) = app.set(&owner, json!("Eins"), 1).await;
    assert_eq!(status, StatusCode::OK);
    let (status, problem) = app.set(&owner, json!("Zwei"), 1).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");

    for markdown in [json!("   "), json!("x".repeat(20_001))] {
        let (status, problem) = app.set(&owner, markdown, 2).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(problem["code"], "validation-failed");
        assert_eq!(problem["errors"][0]["pointer"], "/markdown");
    }
}

#[tokio::test]
async fn the_change_is_in_the_audit_log_without_the_text() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    app.set(&owner, json!("Geheimer Datenschutztext"), 1).await;
    let events: i64 = app
        .app
        .test
        .scalar("SELECT count(*) FROM audit_event WHERE action = 'organization.set_privacy_notice'")
        .await;
    assert_eq!(events, 1);
    let leaks: i64 = app
        .app
        .test
        .scalar("SELECT count(*) FROM audit_event WHERE detail::text LIKE '%Geheimer%'")
        .await;
    assert_eq!(leaks, 0);
}

#[tokio::test]
async fn the_invitation_preview_holds_the_notice() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    app.app
        .test
        .queue_invitation(
            app.testwil,
            &Email::parse("neu@example.org").unwrap(),
            &DisplayName::parse("Nora Neu").unwrap(),
            OrganizationRole::Member,
        )
        .await;
    let token = app.app.mailed_token(LINK).await;
    let preview = |token: String| {
        let request = support::request(Method::POST, "/api/v1/invitations/preview")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"token": token}).to_string()))
            .unwrap();
        app.app.send(request)
    };

    let (_, shown) = preview(token.clone()).await;
    assert_eq!(shown["privacy_notice"], Value::Null);

    app.set(&owner, json!("Text des Vereins"), 1).await;
    let (response, shown) = preview(token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(shown["privacy_notice"], "Text des Vereins");
}
