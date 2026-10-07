//! Event memberships and event access over HTTP, with real sessions (ADR 0052).

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

    async fn add(
        &self,
        cookie: &str,
        event: &str,
        user: UserId,
        role: &str,
    ) -> (StatusCode, Value) {
        self.post(
            cookie,
            &format!("/api/v1/events/{event}/memberships"),
            &json!({"user_id": user.as_uuid(), "event_role": role}),
        )
        .await
    }

    async fn listed_keys(&self, cookie: &str) -> Vec<String> {
        let (status, page) = self.get(cookie, "/api/v1/events").await;
        assert_eq!(status, StatusCode::OK);
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["key"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[tokio::test]
async fn an_owner_adds_a_manager_who_adds_a_contributor_who_cannot_add_anyone() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (anna, anna_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let (ben, ben_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let (carla, _) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;

    let (status, membership) = api.add(&owner, &event, anna, "event-manager").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(membership["user_id"], anna.as_uuid().to_string());
    assert_eq!(membership["event_role"], "event-manager");
    assert_eq!(membership["version"], 1);
    assert!(
        membership["display_name"]
            .as_str()
            .unwrap()
            .starts_with("Member ")
    );

    let (status, _) = api
        .add(&anna_cookie, &event, ben, "event-contributor")
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, problem) = api.add(&ben_cookie, &event, carla, "event-viewer").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, _) = api
        .get(&ben_cookie, &format!("/api/v1/events/{event}/memberships"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, page) = api
        .get(&anna_cookie, &format!("/api/v1/events/{event}/memberships"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let mut roles: Vec<_> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["event_role"].as_str().unwrap())
        .collect();
    roles.sort_unstable();
    assert_eq!(roles, ["event-contributor", "event-manager"]);

    let (status, event_body) = api
        .get(&ben_cookie, &format!("/api/v1/events/{event}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(event_body["key"], "TEST30");
}

#[tokio::test]
async fn a_member_without_an_event_role_does_not_find_the_event() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (_, anna) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;

    let (status, problem) = api.get(&anna, &format!("/api/v1/events/{event}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");
    assert!(api.listed_keys(&anna).await.is_empty());
    assert_eq!(api.listed_keys(&owner).await, ["TEST30"]);
}

#[tokio::test]
async fn only_an_organization_member_gets_an_event_role() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (stranger, _) = api.member("musterhausen", OrganizationRole::Member).await;
    let (anna, _) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;

    let (status, problem) = api.add(&owner, &event, stranger, "event-viewer").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/user_id", "code": "not-a-member"}])
    );

    api.add(&owner, &event, anna, "event-viewer").await;
    let (status, problem) = api.add(&owner, &event, anna, "event-manager").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/user_id", "code": "taken"}])
    );

    let unknown = uuid::Uuid::now_v7();
    let (status, _) = api
        .get(&owner, &format!("/api/v1/events/{unknown}/memberships"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, problem) = api.get(&owner, "/api/v1/events/not-a-uuid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["code"], "malformed-request");
}

#[tokio::test]
async fn a_manager_changes_an_event_role_with_the_record_version() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (anna, anna_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, anna, "event-viewer").await;
    let path = format!(
        "/api/v1/events/{event}/memberships/{}/change-role",
        anna.as_uuid()
    );

    let (status, changed) = api
        .post(
            &owner,
            &path,
            &json!({"event_role": "event-manager", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed["event_role"], "event-manager");
    assert_eq!(changed["version"], 2);

    let (status, problem) = api
        .post(
            &owner,
            &path,
            &json!({"event_role": "event-viewer", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");

    // The new role works with the next request.
    let (status, _) = api
        .get(&anna_cookie, &format!("/api/v1/events/{event}/memberships"))
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_removed_member_does_not_find_the_event_with_the_next_request() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (anna, anna_cookie) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, anna, "event-contributor").await;
    let (status, _) = api
        .get(&anna_cookie, &format!("/api/v1/events/{event}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(api.listed_keys(&anna_cookie).await, ["TEST30"]);

    let path = format!(
        "/api/v1/events/{event}/memberships/{}/remove",
        anna.as_uuid()
    );
    let (status, problem) = api
        .post(&owner, &path, &json!({"expected_version": 7}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");
    let (status, _) = api
        .post(&owner, &path, &json!({"expected_version": 1}))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = api
        .get(&anna_cookie, &format!("/api/v1/events/{event}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(api.listed_keys(&anna_cookie).await.is_empty());
    let (status, _) = api
        .post(&owner, &path, &json!({"expected_version": 1}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_event_manager_does_not_see_the_event_with_a_session_of_another_organization() {
    let api = Api::start().await;
    let (_, owner) = api.member("testwil", OrganizationRole::Owner).await;
    let (anna, anna_testwil) = api.member("testwil", OrganizationRole::Member).await;
    let event = api.create_event(&owner, "TEST30").await;
    api.add(&owner, &event, anna, "event-manager").await;

    let musterhausen = api.test.create_organization("musterhausen").await;
    api.test
        .add_membership(musterhausen, anna, OrganizationRole::Owner)
        .await;
    let anna_musterhausen = api.test.sign_in(anna, Some(musterhausen)).await;

    let (status, _) = api
        .get(&anna_testwil, &format!("/api/v1/events/{event}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api
        .get(&anna_musterhausen, &format!("/api/v1/events/{event}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api
        .get(
            &anna_musterhausen,
            &format!("/api/v1/events/{event}/memberships"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(api.listed_keys(&anna_musterhausen).await.is_empty());
}
