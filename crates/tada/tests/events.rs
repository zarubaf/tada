//! The walking skeleton end to end: HTTP, the `app` command, PostgreSQL and back.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, header};
use serde_json::{Value, json};
use tada_adapters::clock::SystemClock;
use tada_app::caller::OrganizationRole;
use tada_app::domain::ids::UserId;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;

struct Api {
    router: axum::Router,
    test: TestDatabase,
    user: UserId,
    cookie: String,
}

impl Api {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let (_, user, cookie) = test.member("testwil", OrganizationRole::Owner).await;
        let router = support::session_router(&test, Arc::new(SystemClock));
        Self {
            router,
            test,
            user,
            cookie: format!("{}={cookie}", support::SESSION_COOKIE),
        }
    }

    async fn send(&self, request: Request<Body>) -> (Response<Body>, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (Response::from_parts(parts, Body::empty()), value)
    }

    async fn post(&self, path: &str, body: &Value) -> (Response<Body>, Value) {
        let request = support::request(Method::POST, path)
            .header(header::COOKIE, &self.cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        self.send(request).await
    }

    async fn get(&self, path: &str) -> (Response<Body>, Value) {
        let request = Request::get(path)
            .header(header::COOKIE, &self.cookie)
            .body(Body::empty())
            .unwrap();
        self.send(request).await
    }
}

#[tokio::test]
async fn creates_an_event_and_lists_it() {
    let api = Api::start().await;

    let (response, event) = api
        .post(
            "/api/v1/events",
            &json!({"key": "TEST30", "name": "Open Day Testwil"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(event["key"], "TEST30");
    assert_eq!(event["time_zone"], "Europe/Zurich");
    assert_eq!(event["version"], 1);
    let id: uuid::Uuid = event["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(id.get_version_num(), 7);

    let (response, page) = api.get("/api/v1/events").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(page["items"], json!([event]));
    assert!(page.get("next_cursor").is_none());

    // The creator becomes the event manager of the new event (ADR 0052).
    let (response, members) = api.get(&format!("/api/v1/events/{id}/memberships")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let members = members["items"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["user_id"], json!(api.user.as_uuid()));
    assert_eq!(members[0]["event_role"], "event-manager");
}

#[tokio::test]
async fn a_retry_with_the_same_id_returns_the_event_once() {
    let api = Api::start().await;
    let body = json!({"id": "0199b8e0-1111-7000-8000-000000000001", "key": "TEST30", "name": "Open Day Testwil"});

    let (first, created) = api.post("/api/v1/events", &body).await;
    let (retry, existing) = api.post("/api/v1/events", &body).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(created, existing);

    let (_, page) = api.get("/api/v1/events").await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_values_give_a_problem_with_pointers() {
    let api = Api::start().await;
    let (response, problem) = api
        .post(
            "/api/v1/events",
            &json!({"key": "x", "name": "", "time_zone": "Mars/Olympus"}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let request_id = response.headers()["x-request-id"].to_str().unwrap();
    assert_eq!(problem["code"], "validation-failed");
    assert_eq!(problem["status"], 422);
    assert_eq!(
        problem["type"],
        "https://github.com/zarubaf/tada/blob/main/doc/problems.md#validation-failed"
    );
    assert_eq!(problem["request_id"], request_id);
    assert_eq!(problem["instance"], format!("urn:uuid:{request_id}"));
    assert_eq!(
        problem["errors"],
        json!([
            {"pointer": "/key", "code": "length"},
            {"pointer": "/name", "code": "empty"},
            {"pointer": "/time_zone", "code": "unknown"},
        ])
    );
}

#[tokio::test]
async fn a_taken_key_gives_a_problem() {
    let api = Api::start().await;
    let body = json!({"key": "TEST30", "name": "Open Day Testwil"});
    api.post("/api/v1/events", &body).await;
    let (response, problem) = api.post("/api/v1/events", &body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/key", "code": "taken"}])
    );
}

#[tokio::test]
async fn a_body_that_is_not_json_gives_a_problem_without_its_content() {
    let api = Api::start().await;
    let request = support::request(Method::POST, "/api/v1/events")
        .header(header::COOKIE, &api.cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"key": "secret-value""#))
        .unwrap();
    let (response, problem) = api.send(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(problem["code"], "malformed-request");
    assert!(!problem.to_string().contains("secret-value"));

    let request = support::request(Method::POST, "/api/v1/events")
        .header(header::COOKIE, &api.cookie)
        .body(Body::from("{}"))
        .unwrap();
    let (response, problem) = api.send(request).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(problem["code"], "unsupported-media-type");
}

#[tokio::test]
async fn lists_in_pages_with_an_opaque_cursor() {
    let api = Api::start().await;
    for key in ["CC", "AA", "BB"] {
        api.post("/api/v1/events", &json!({"key": key, "name": key}))
            .await;
    }
    let (_, first) = api.get("/api/v1/events?limit=2").await;
    let keys: Vec<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["key"].clone())
        .collect();
    assert_eq!(keys, ["AA", "BB"]);

    let cursor = first["next_cursor"].as_str().unwrap();
    let (_, second) = api
        .get(&format!("/api/v1/events?limit=2&cursor={cursor}"))
        .await;
    let keys: Vec<_> = second["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["key"].clone())
        .collect();
    assert_eq!(keys, ["CC"]);
    assert!(second.get("next_cursor").is_none());

    let (response, problem) = api.get("/api/v1/events?cursor=garbage").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(problem["code"], "malformed-request");
    let (response, _) = api.get("/api/v1/events?limit=201").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_unknown_route_gives_a_not_found_problem() {
    let api = Api::start().await;
    let (response, problem) = api.get("/api/v1/nothing").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");
}

#[tokio::test]
async fn a_state_change_from_another_origin_changes_nothing() {
    let api = Api::start().await;
    let body = json!({"key": "TEST30", "name": "Open Day Testwil"});
    let foreign =
        Request::post("/api/v1/events").header(header::ORIGIN, "https://evil.example.com");
    let missing = Request::post("/api/v1/events");
    // An origin differs also in the scheme or in the port.
    let http = Request::post("/api/v1/events").header(header::ORIGIN, "http://tada.example.org");
    for request in [foreign, missing, http] {
        let request = request
            .header(header::COOKIE, &api.cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (response, problem) = api.send(request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
    }
    let events: i64 = api.test.scalar("SELECT count(*) FROM event").await;
    assert_eq!(events, 0, "no handler ran");

    let (response, _) = api.post("/api/v1/events", &body).await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "the origin of tada works"
    );
}

#[tokio::test]
async fn a_read_needs_no_origin() {
    let api = Api::start().await;
    let request = Request::get("/api/v1/events")
        .header(header::COOKIE, &api.cookie)
        .header(header::ORIGIN, "https://evil.example.com")
        .body(Body::empty())
        .unwrap();
    let (response, _) = api.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_request_without_a_session_gets_401() {
    let api = Api::start().await;
    let request = Request::get("/api/v1/events").body(Body::empty()).unwrap();
    let (response, problem) = api.send(request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(problem["code"], "unauthenticated");
}
