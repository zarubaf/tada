//! The AI state and the monthly AI cap of an organization over HTTP (ADRs 0047 and 0077).

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

const PATH: &str = "/api/v1/organization/ai";
const SET: &str = "/api/v1/organization/ai/set-cap";

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
}

#[tokio::test]
async fn an_admin_sets_the_cap_and_a_member_cannot() {
    let app = App::start().await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;
    app.app
        .test
        .execute(&[&format!(
            "INSERT INTO ai_usage (id, organization_id, task, prompt_version, model_id, input_tokens,
                 output_tokens, cost_chf, at)
             VALUES (gen_random_uuid(), '{}', 'pm-digest', 1, 'model-a', 1000, 200, 0.0036, '{}')",
            app.testwil,
            app.app.clock.now()
        )])
        .await;

    // Without a row, the cap is the default of the operator.
    let (status, read) = app.call(&admin, Method::GET, PATH, None).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(
        read,
        json!({"enabled": false, "configured": false, "cap_chf": "25.00", "used_chf": "0.0036", "version": 1})
    );

    let body = json!({"cap_chf": "40.50", "expected_version": 1});
    let (status, problem) = app.call(&member, Method::POST, SET, Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, _) = app.call(&member, Method::GET, PATH, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, set) = app.call(&admin, Method::POST, SET, Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["cap_chf"], "40.50");
    assert_eq!(set["version"], 2);

    let (status, problem) = app.call(&admin, Method::POST, SET, Some(&body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");

    let zero = json!({"cap_chf": "0", "expected_version": 2});
    let (status, problem) = app.call(&admin, Method::POST, SET, Some(&zero)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["errors"][0]["pointer"], "/cap_chf");

    let audits: i64 = app
        .app
        .test
        .scalar("SELECT count(*) FROM audit_event WHERE action = 'organization.set_ai_cap'")
        .await;
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn setting_the_cap_needs_a_recent_sign_in() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    app.app.clock.advance(jiff::SignedDuration::from_mins(16));

    let body = json!({"cap_chf": "100", "expected_version": 1});
    let (status, problem) = app.call(&owner, Method::POST, SET, Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "recent-sign-in-required");
    let (status, read) = app.call(&owner, Method::GET, PATH, None).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["cap_chf"], "25.00");
}
