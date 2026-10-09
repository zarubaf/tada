//! Workstreams over HTTP, with real sessions (ADR 0067).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::SESSION_COOKIE;
use tada_adapters::clock::SystemClock;
use tada_app::caller::OrganizationRole;
use tada_app::domain::ids::UserId;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::testing::TestDatabase;

struct Api {
    router: axum::Router,
    test: TestDatabase,
}

impl Api {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let db = Arc::new(test.database.clone());
        let clock = Arc::new(SystemClock);
        let authenticator = Arc::new(SessionAuthenticator::new(db.clone(), db, clock.clone()));
        let router = tada_api::router(support::api_state(&test, authenticator, clock), None);
        Self { router, test }
    }

    async fn member(&self, slug: &str, role: OrganizationRole) -> (UserId, String) {
        let (_, user, cookie) = self.test.member(slug, role).await;
        (user, cookie)
    }

    async fn send(
        &self,
        cookie: &str,
        request: axum::http::request::Builder,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = support::send(&self.router, request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, cookie: &str, path: &str) -> (StatusCode, Value) {
        self.send(cookie, support::request(Method::GET, path), None)
            .await
    }

    async fn post(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(cookie, support::request(Method::POST, path), Some(body))
            .await
    }

    /// An owner creates an event and returns its ID.
    async fn create_event(&self, owner: &str, key: &str) -> String {
        let (status, event) = self
            .post(
                owner,
                "/api/v1/events",
                &json!({"key": key, "name": "Open Day Testwil"}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        event["id"].as_str().unwrap().to_owned()
    }

    async fn patch(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(cookie, support::request(Method::PATCH, path), Some(body))
            .await
    }

    async fn add(&self, owner: &str, event: &str, user: UserId, role: &str) {
        let (status, _) = self
            .post(
                owner,
                &format!("/api/v1/events/{event}/memberships"),
                &json!({"user_id": user.as_uuid(), "event_role": role}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    async fn workstream(&self, cookie: &str, event: &str, name: &str, lead: UserId) -> Value {
        let (status, workstream) = self
            .post(
                cookie,
                &format!("/api/v1/events/{event}/workstreams"),
                &json!({"name": name, "lead_user_id": lead.as_uuid()}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{workstream}");
        workstream
    }
}

fn path(event: &str, workstream: &Value) -> String {
    format!(
        "/api/v1/events/{event}/workstreams/{}",
        workstream["id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn an_event_manager_creates_lists_and_closes_a_workstream() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (anna, anna_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let (ben, ben_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let (carla, carla_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, anna, "event-manager").await;
    api.add(&owner, &event, ben, "event-contributor").await;
    api.add(&owner, &event, carla, "event-viewer").await;

    let ground = api.workstream(&anna_cookie, &event, "Gelände", ben).await;
    assert_eq!(ground["status"], "active");
    assert_eq!(ground["version"], 1);
    assert_eq!(ground["lead_user_id"], ben.as_uuid().to_string());

    // A contributor and a viewer read the list but change nothing.
    for cookie in [&ben_cookie, &carla_cookie] {
        let (status, page) = api
            .get(cookie, &format!("/api/v1/events/{event}/workstreams"))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        let (status, problem) = api
            .post(
                cookie,
                &format!("/api/v1/events/{event}/workstreams"),
                &json!({"name": "Küche", "lead_user_id": ben.as_uuid()}),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
        let (status, _) = api
            .patch(
                cookie,
                &path(&event, &ground),
                &json!({"name": "X", "expected_version": 1}),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    let (status, closed) = api
        .patch(
            &anna_cookie,
            &path(&event, &ground),
            &json!({"status": "closed", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(closed["status"], "closed");
    assert_eq!(closed["version"], 2);
    assert_eq!(closed["name"], "Gelände");
}

#[tokio::test]
async fn the_lead_is_a_contributor_or_manager_and_the_name_is_free() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (ben, _) = api.member("testwil", OrganizationRole::Member).await;
    let (carla, _) = api.member("testwil", OrganizationRole::Member).await;
    let (dora, _) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, ben, "event-contributor").await;
    api.add(&owner, &event, carla, "event-viewer").await;

    let create = format!("/api/v1/events/{event}/workstreams");
    for lead in [carla, dora] {
        let (status, problem) = api
            .post(
                &owner,
                &create,
                &json!({"name": "Küche", "lead_user_id": lead.as_uuid()}),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            problem["errors"],
            json!([{"pointer": "/lead", "code": "unknown-member"}])
        );
    }
    let ground = api.workstream(&owner, &event, "Gelände", ben).await;
    let (status, problem) = api
        .post(
            &owner,
            &create,
            &json!({"name": "GELÄNDE", "lead_user_id": ben.as_uuid()}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/name", "code": "taken"}])
    );

    let (status, problem) = api
        .patch(
            &owner,
            &path(&event, &ground),
            &json!({"lead_user_id": carla.as_uuid(), "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/lead", "code": "unknown-member"}])
    );
}

#[tokio::test]
async fn two_changes_with_one_version_leave_one_winner() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (ben, _) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, ben, "event-contributor").await;
    let ground = api.workstream(&owner, &event, "Gelände", ben).await;
    let path = path(&event, &ground);

    let (alpha, beta) = (
        json!({"name": "Alpha", "expected_version": 1}),
        json!({"name": "Beta", "expected_version": 1}),
    );
    let ((first, _), (second, _)) = tokio::join!(
        api.patch(&owner, &path, &alpha),
        api.patch(&owner, &path, &beta)
    );
    let mut statuses = [first, second];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
}

#[tokio::test]
async fn a_workstream_of_another_event_is_not_found() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (ben, _) = api.member("testwil", OrganizationRole::Member).await;
    let open_day = api.create_event(&owner, "TEST30").await;
    let other = api.create_event(&owner, "TEST31").await;
    api.add(&owner, &open_day, ben, "event-contributor").await;
    api.add(&owner, &other, ben, "event-contributor").await;
    let ground = api.workstream(&owner, &open_day, "Gelände", ben).await;

    let (status, problem) = api
        .patch(
            &owner,
            &path(&other, &ground),
            &json!({"name": "Bar", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");
    let (status, page) = api
        .get(&owner, &format!("/api/v1/events/{other}/workstreams"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"].as_array().unwrap().is_empty());

    // Another organization neither sees the event nor the workstream.
    let (_, stranger) = api.member("musterhausen", OrganizationRole::Owner).await;
    let (status, _) = api
        .patch(
            &stranger,
            &path(&open_day, &ground),
            &json!({"name": "Bar", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
