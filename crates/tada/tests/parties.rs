//! Persons and institutions over HTTP, with real sessions (ADR 0069).

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
use tada_app::session::SessionAuthenticator;
use tada_store_pg::testing::TestDatabase;

struct Api {
    router: axum::Router,
    test: TestDatabase,
}

/// A member who calls the API.
struct Member {
    router: axum::Router,
    cookie: String,
}

impl Member {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut request = support::request(method, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={}", self.cookie));
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let (response, value) = support::send(&self.router, request.body(body).unwrap()).await;
        (response.status(), value)
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, Some(body)).await
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call(Method::GET, path, None).await
    }

    async fn patch(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::PATCH, path, Some(body)).await
    }
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

    async fn member(&self, slug: &str, role: OrganizationRole) -> Member {
        let (_, _, cookie) = self.test.member(slug, role).await;
        Member {
            router: self.router.clone(),
            cookie,
        }
    }
}

#[tokio::test]
async fn persons_are_isolated_between_organizations() {
    let api = Api::start().await;
    let owner_a = api.member("testwil", OrganizationRole::Owner).await;
    let owner_b = api.member("musterhausen", OrganizationRole::Owner).await;

    let (status, person) = owner_a
        .post("/api/v1/persons", json!({"name": "Beat Muster"}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{person}");
    assert_eq!(person["local_id"], "PER-001");
    let (_, institution) = owner_a
        .post(
            "/api/v1/institutions",
            json!({"name": "Testwil Generatoren AG", "kind": "company"}),
        )
        .await;
    assert_eq!(institution["local_id"], "INS-001");

    let path = format!("/api/v1/persons/{}", person["id"].as_str().unwrap());
    let (status, body) = owner_b.get(&path).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = owner_b
        .patch(
            &path,
            json!({"name": "Anna Beispiel", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = owner_b
        .get(&format!(
            "/api/v1/institutions/{}",
            institution["id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let (_, list) = owner_b.get("/api/v1/persons").await;
    assert_eq!(list["items"], json!([]));
    // The numbers of organization B start at 1, whatever organization A holds.
    let (_, first_of_b) = owner_b
        .post("/api/v1/persons", json!({"name": "Anna Beispiel"}))
        .await;
    assert_eq!(first_of_b["local_id"], "PER-001");
    let (_, unchanged) = owner_a.get(&path).await;
    assert_eq!(unchanged["name"], "Beat Muster");
    assert_eq!(unchanged["version"], 1);
}

#[tokio::test]
async fn the_list_filters_by_name() {
    let api = Api::start().await;
    let owner = api.member("testwil", OrganizationRole::Owner).await;
    for name in ["Beat Müller", "Anna Beispiel", "Clara MÜLLER"] {
        owner.post("/api/v1/persons", json!({"name": name})).await;
    }

    let (status, list) = owner.get("/api/v1/persons?q=muller").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let names: Vec<_> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Beat Müller", "Clara MÜLLER"]);

    let (_, first) = owner.get("/api/v1/persons?limit=2").await;
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    let cursor = first["next_cursor"].as_str().unwrap();
    let (_, second) = owner
        .get(&format!("/api/v1/persons?limit=2&cursor={cursor}"))
        .await;
    assert_eq!(second["items"][0]["local_id"], "PER-003");
    assert!(second.get("next_cursor").is_none());
}

#[tokio::test]
async fn a_member_without_an_event_role_creates_and_reads_nothing() {
    let api = Api::start().await;
    let member = api.member("testwil", OrganizationRole::Member).await;
    let (status, body) = member
        .post("/api/v1/persons", json!({"name": "Beat Muster"}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = member.get("/api/v1/institutions").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn a_person_changes_with_the_expected_version() {
    let api = Api::start().await;
    let owner = api.member("testwil", OrganizationRole::Owner).await;
    let (_, person) = owner
        .post(
            "/api/v1/persons",
            json!({"name": "Beat Muster", "email": "beat@example.org"}),
        )
        .await;
    let path = format!("/api/v1/persons/{}", person["id"].as_str().unwrap());

    let (status, changed) = owner
        .patch(
            &path,
            json!({"email": null, "phone": "+41 00 000 00 00", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["name"], "Beat Muster");
    assert_eq!(changed["email"], Value::Null);
    assert_eq!(changed["phone"], "+41 00 000 00 00");
    assert_eq!(changed["version"], 2);

    let (status, body) = owner
        .patch(
            &path,
            json!({"name": "Anna Beispiel", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "record-version-conflict");
}

#[tokio::test]
async fn two_changes_with_the_same_version_have_one_winner() {
    let api = Api::start().await;
    let owner = api.member("testwil", OrganizationRole::Owner).await;
    let (_, institution) = owner
        .post(
            "/api/v1/institutions",
            json!({"name": "Testwil Generatoren AG", "kind": "company"}),
        )
        .await;
    let path = format!(
        "/api/v1/institutions/{}",
        institution["id"].as_str().unwrap()
    );
    let (first, second) = tokio::join!(
        owner.patch(&path, json!({"kind": "club", "expected_version": 1})),
        owner.patch(&path, json!({"kind": "other", "expected_version": 1})),
    );
    let mut statuses = [first.0, second.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
}
