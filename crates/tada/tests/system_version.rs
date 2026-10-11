//! `GET /api/v1/system/version`: the versions of the product and of the running roles (ADR 0075).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE};
use tada_app::build_info::{BuildInfo, ProcessRole};
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use uuid::Uuid;

const PATH: &str = "/api/v1/system/version";

const OLD: BuildInfo = BuildInfo {
    version: "0.0.9",
    commit: "1111111",
    built_at: "2026-01-01T00:00:00Z",
    build_id: "0123456789abcdef0123456789abcdef",
};

async fn member(app: &MailApp) -> String {
    let organization = app.test.create_organization("testwil").await;
    let user = app
        .test
        .create_user(
            &DisplayName::parse("Mia Muster").unwrap(),
            &Email::parse("mia@example.org").unwrap(),
        )
        .await;
    app.test
        .add_membership(organization, user, OrganizationRole::Member)
        .await;
    app.test
        .sign_in(user, Some(organization), app.clock.now())
        .await
}

async fn get(app: &MailApp, cookie: Option<&str>) -> (StatusCode, Value) {
    let mut request = support::request(Method::GET, PATH);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
    }
    let (response, body) = app
        .send(request.body(axum::body::Body::empty()).unwrap())
        .await;
    (response.status(), body)
}

/// Records a heartbeat of the role.
async fn beat(app: &MailApp, role: ProcessRole, build: &BuildInfo) {
    app.test
        .database
        .record_heartbeat(Uuid::now_v7(), role, build)
        .await
        .unwrap();
}

#[tokio::test]
async fn system_version_needs_sign_in() {
    let app = MailApp::start().await;
    beat(&app, ProcessRole::Worker, &tada::BUILD).await;
    let (status, body) = get(&app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // The problem names no version.
    assert!(!body.to_string().contains(tada::BUILD.version));
}

#[tokio::test]
async fn system_version_lists_roles_and_marks_mismatch() {
    let app = MailApp::start().await;
    let cookie = member(&app).await;
    beat(&app, ProcessRole::Serve, &tada::BUILD).await;
    beat(&app, ProcessRole::Worker, &tada::BUILD).await;

    let (status, body) = get(&app, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["product"], tada::BUILD.version);
    assert_eq!(body["commit"], tada::BUILD.commit);
    assert_eq!(body["web_build"], tada::BUILD.build_id);
    assert_eq!(body["api"], "v1");
    assert_eq!(body["mcp"], tada::BUILD.version);
    assert!(body["schema"].as_i64().unwrap() >= 27);
    assert_eq!(body["mismatch"], false);
    let roles: Vec<&Value> = body["roles"].as_array().unwrap().iter().collect();
    assert_eq!(roles.len(), 2);
    assert_eq!(roles[0]["role"], "serve");

    beat(&app, ProcessRole::Telegram, &OLD).await;
    let (_, body) = get(&app, Some(&cookie)).await;
    assert_eq!(body["mismatch"], true);
    let telegram = body["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|role| role["role"] == "telegram")
        .unwrap();
    assert_eq!(telegram["version"], json!("0.0.9"));
    assert_eq!(telegram["commit"], json!("1111111"));
}

#[tokio::test]
async fn a_stopped_role_is_not_listed() {
    let app = MailApp::start().await;
    let cookie = member(&app).await;
    beat(&app, ProcessRole::Worker, &OLD).await;
    app.test
        .execute(&[
            "UPDATE worker_heartbeat SET last_loop_at = last_loop_at - interval '10 minutes'",
        ])
        .await;
    let (_, body) = get(&app, Some(&cookie)).await;
    assert_eq!(body["roles"], json!([]));
    assert_eq!(body["mismatch"], false);
}
