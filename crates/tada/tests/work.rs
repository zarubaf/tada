//! Actions and commitments over HTTP, with real sessions (ADR 0068).

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

/// An event of Testwil with an owner, a contributor and a viewer.
struct Event {
    id: String,
    owner: String,
    ben: UserId,
    ben_cookie: String,
    viewer: String,
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

    async fn send(
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
        let (response, value) = support::send(&self.router, request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, cookie: &str, path: &str) -> (StatusCode, Value) {
        self.send(cookie, Method::GET, path, None).await
    }

    async fn post(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(cookie, Method::POST, path, Some(body)).await
    }

    async fn patch(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(cookie, Method::PATCH, path, Some(body)).await
    }

    async fn created(&self, cookie: &str, path: &str, body: &Value) -> Value {
        let (status, value) = self.post(cookie, path, body).await;
        assert_eq!(status, StatusCode::CREATED, "{value}");
        value
    }

    async fn event(&self) -> Event {
        let (_, _, owner) = self.test.member("testwil", OrganizationRole::Owner).await;
        let (_, ben, ben_cookie) = self.test.member("testwil", OrganizationRole::Member).await;
        let (_, carla, viewer) = self.test.member("testwil", OrganizationRole::Member).await;
        let event = self
            .created(
                &owner,
                "/api/v1/events",
                &json!({"key": "TEST30", "name": "Open Day Testwil"}),
            )
            .await;
        let id = event["id"].as_str().unwrap().to_owned();
        for (user, role) in [(ben, "event-contributor"), (carla, "event-viewer")] {
            self.created(
                &owner,
                &format!("/api/v1/events/{id}/memberships"),
                &json!({"user_id": user.as_uuid(), "event_role": role}),
            )
            .await;
        }
        Event {
            id,
            owner,
            ben,
            ben_cookie,
            viewer,
        }
    }

    async fn institution(&self, cookie: &str, name: &str) -> String {
        let institution = self
            .created(
                cookie,
                "/api/v1/institutions",
                &json!({"name": name, "kind": "company"}),
            )
            .await;
        institution["id"].as_str().unwrap().to_owned()
    }
}

fn record(event: &Event, kind: &str, record: &Value) -> String {
    format!(
        "/api/v1/events/{}/{kind}/{}",
        event.id,
        record["id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn a_contributor_creates_and_changes_an_action() {
    let api = Api::start().await;
    let e = api.event().await;
    let actions = format!("/api/v1/events/{}/actions", e.id);
    let action = api
        .created(
            &e.ben_cookie,
            &actions,
            &json!({
                "title": "Generator bestellen",
                "owner_user_id": e.ben.as_uuid(),
                "due_date": "2030-06-01",
            }),
        )
        .await;
    assert_eq!(action["local_id"], "ACT-001");
    assert_eq!(action["status"], "open");
    assert_eq!(action["evidence"], json!([]));
    assert_eq!(action["due_date"], "2030-06-01");
    assert_eq!(action["version"], 1);

    let (status, changed) = api
        .patch(
            &e.ben_cookie,
            &record(&e, "actions", &action),
            &json!({"status": "in-progress", "due_date": null, "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["status"], "in-progress");
    assert_eq!(changed["due_date"], Value::Null);
    assert_eq!(changed["version"], 2);

    let (status, problem) = api
        .patch(
            &e.ben_cookie,
            &record(&e, "actions", &action),
            &json!({"expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["code"], "validation-failed");

    // A viewer reads but changes nothing.
    let (status, page) = api
        .get(&e.viewer, &format!("{actions}?owner={}", e.ben.as_uuid()))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let (status, page) = api.get(&e.viewer, &format!("{actions}?owner=me")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"].as_array().unwrap().is_empty());
    let (status, problem) = api
        .post(
            &e.viewer,
            &actions,
            &json!({"title": "X", "owner_user_id": e.ben.as_uuid()}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
}

#[tokio::test]
async fn two_changes_with_the_same_version_conflict() {
    let api = Api::start().await;
    let e = api.event().await;
    let action = api
        .created(
            &e.owner,
            &format!("/api/v1/events/{}/actions", e.id),
            &json!({"title": "Generator bestellen", "owner_user_id": e.ben.as_uuid()}),
        )
        .await;
    let path = record(&e, "actions", &action);
    let (alpha, beta) = (
        json!({"title": "Alpha", "expected_version": 1}),
        json!({"title": "Beta", "expected_version": 1}),
    );
    let ((first, a), (second, b)) = tokio::join!(
        api.patch(&e.owner, &path, &alpha),
        api.patch(&e.ben_cookie, &path, &beta)
    );
    let mut statuses = [first, second];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT], "{a} {b}");
    let problem = if first == StatusCode::CONFLICT { a } else { b };
    assert_eq!(problem["code"], "record-version-conflict");
}

#[tokio::test]
async fn a_conditional_commitment_becomes_firm_only_with_a_reason() {
    let api = Api::start().await;
    let e = api.event().await;
    let supplier = api
        .institution(&e.ben_cookie, "Testwil Generatoren AG")
        .await;
    let commitment = api
        .created(
            &e.ben_cookie,
            &format!("/api/v1/events/{}/commitments", e.id),
            &json!({
                "text": "Generator delivery Friday 15:00",
                "condition": "subject to signed order",
                "promisor": {"kind": "institution", "id": supplier},
                "owner_user_id": e.ben.as_uuid(),
            }),
        )
        .await;
    assert_eq!(commitment["status"], "conditional");
    assert_eq!(commitment["local_id"], "COM-001");
    assert_eq!(commitment["promisor"]["local_id"], "INS-001");
    assert_eq!(commitment["promisor"]["name"], "Testwil Generatoren AG");
    assert_eq!(commitment["firm_reason"], Value::Null);
    assert_eq!(commitment["evidence"], json!([]));
    let path = record(&e, "commitments", &commitment);

    let (status, problem) = api
        .patch(
            &e.ben_cookie,
            &path,
            &json!({"status": "firm", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "invalid-transition");

    let (status, changed) = api
        .patch(
            &e.ben_cookie,
            &path,
            &json!({"due_date": "2030-06-07", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["status"], "conditional");

    let firm = format!("{path}/firm");
    let (status, problem) = api
        .post(
            &e.ben_cookie,
            &firm,
            &json!({"reason": " ", "expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/reason", "code": "empty"}])
    );
    let (status, made) = api
        .post(
            &e.ben_cookie,
            &firm,
            &json!({"reason": "The order is signed.", "expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["status"], "firm");
    assert_eq!(made["firm_reason"], "The order is signed.");
    assert_eq!(made["condition"], "subject to signed order");

    let (status, read) = api.get(&e.viewer, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["version"], 3);
    let (status, page) = api
        .get(
            &e.viewer,
            &format!("/api/v1/events/{}/commitments?status=firm", e.id),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_promisor_of_another_organization_is_unknown() {
    let api = Api::start().await;
    let e = api.event().await;
    let (_, _, stranger) = api
        .test
        .member("musterhausen", OrganizationRole::Owner)
        .await;
    let foreign = api.institution(&stranger, "Musterhausen Strom AG").await;
    let (status, problem) = api
        .post(
            &e.ben_cookie,
            &format!("/api/v1/events/{}/commitments", e.id),
            &json!({
                "text": "Strom ab Freitag",
                "promisor": {"kind": "institution", "id": foreign},
                "owner_user_id": e.ben.as_uuid(),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/promisor", "code": "unknown-record"}])
    );
}

#[tokio::test]
async fn a_closed_workstream_cannot_be_set() {
    let api = Api::start().await;
    let e = api.event().await;
    let ground = api
        .created(
            &e.owner,
            &format!("/api/v1/events/{}/workstreams", e.id),
            &json!({"name": "Gelände", "lead_user_id": e.ben.as_uuid()}),
        )
        .await;
    let (status, _) = api
        .patch(
            &e.owner,
            &record(&e, "workstreams", &ground),
            &json!({"status": "closed", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, problem) = api
        .post(
            &e.ben_cookie,
            &format!("/api/v1/events/{}/actions", e.id),
            &json!({
                "title": "Zaun stellen",
                "owner_user_id": e.ben.as_uuid(),
                "workstream_id": ground["id"],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/workstream", "code": "closed"}])
    );
}

/// A workstream of another event or another organization is unknown in the event (Review Focus 2).
#[tokio::test]
async fn a_workstream_of_another_event_or_organization_is_unknown() {
    let api = Api::start().await;
    let e = api.event().await;
    // A workstream of a second event of Testwil.
    let second = api
        .created(
            &e.owner,
            "/api/v1/events",
            &json!({"key": "TEST31", "name": "Flugtag Testwil"}),
        )
        .await;
    let second = second["id"].as_str().unwrap();
    api.created(
        &e.owner,
        &format!("/api/v1/events/{second}/memberships"),
        &json!({"user_id": e.ben.as_uuid(), "event_role": "event-contributor"}),
    )
    .await;
    let other_event = api
        .created(
            &e.owner,
            &format!("/api/v1/events/{second}/workstreams"),
            &json!({"name": "Gelände", "lead_user_id": e.ben.as_uuid()}),
        )
        .await;
    // A workstream of an event of Musterhausen.
    let (_, stranger, stranger_cookie) = api
        .test
        .member("musterhausen", OrganizationRole::Owner)
        .await;
    let foreign_event = api
        .created(
            &stranger_cookie,
            "/api/v1/events",
            &json!({"key": "TEST30", "name": "Open Day Musterhausen"}),
        )
        .await;
    let other_organization = api
        .created(
            &stranger_cookie,
            &format!(
                "/api/v1/events/{}/workstreams",
                foreign_event["id"].as_str().unwrap()
            ),
            &json!({"name": "Gelände", "lead_user_id": stranger.as_uuid()}),
        )
        .await;

    let actions = format!("/api/v1/events/{}/actions", e.id);
    let commitments = format!("/api/v1/events/{}/commitments", e.id);
    let supplier = api
        .institution(&e.ben_cookie, "Testwil Generatoren AG")
        .await;
    let action = api
        .created(
            &e.ben_cookie,
            &actions,
            &json!({"title": "Zaun stellen", "owner_user_id": e.ben.as_uuid()}),
        )
        .await;
    let commitment = api
        .created(
            &e.ben_cookie,
            &commitments,
            &json!({
                "text": "Generator delivery Friday 15:00",
                "promisor": {"kind": "institution", "id": supplier},
                "owner_user_id": e.ben.as_uuid(),
            }),
        )
        .await;
    let unknown = json!([{"pointer": "/workstream", "code": "unknown-record"}]);

    for workstream in [&other_event, &other_organization] {
        let id = &workstream["id"];
        let attempts = [
            (
                Method::POST,
                actions.clone(),
                json!({"title": "Zaun", "owner_user_id": e.ben.as_uuid(), "workstream_id": id}),
            ),
            (
                Method::POST,
                commitments.clone(),
                json!({
                    "text": "Strom ab Freitag",
                    "promisor": {"kind": "institution", "id": supplier},
                    "owner_user_id": e.ben.as_uuid(),
                    "workstream_id": id,
                }),
            ),
            (
                Method::PATCH,
                record(&e, "actions", &action),
                json!({"workstream_id": id, "expected_version": 1}),
            ),
            (
                Method::PATCH,
                record(&e, "commitments", &commitment),
                json!({"workstream_id": id, "expected_version": 1}),
            ),
        ];
        for (method, path, body) in attempts {
            let (status, problem) = api.send(&e.owner, method, &path, Some(&body)).await;
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{path}: {problem}"
            );
            assert_eq!(problem["code"], "validation-failed");
            assert_eq!(problem["errors"], unknown, "{path}");
        }
    }

    // Nothing changed: one record of each kind, still at version 1 without a workstream.
    for (list, record) in [(&actions, &action), (&commitments, &commitment)] {
        let (status, page) = api.get(&e.viewer, list).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        assert_eq!(page["items"][0]["id"], record["id"]);
        assert_eq!(page["items"][0]["version"], 1);
        assert_eq!(page["items"][0]["workstream_id"], Value::Null);
    }
}

#[tokio::test]
async fn my_work_lists_only_my_open_records_by_due_date() {
    let api = Api::start().await;
    let e = api.event().await;
    let actions = format!("/api/v1/events/{}/actions", e.id);
    let ben = e.ben.as_uuid();
    for (title, due) in [
        ("Spät", json!("2030-06-09")),
        ("Ohne Datum", Value::Null),
        ("Früh", json!("2030-06-01")),
    ] {
        api.created(
            &e.ben_cookie,
            &actions,
            &json!({"title": title, "owner_user_id": ben, "due_date": due}),
        )
        .await;
    }
    let done = api
        .created(
            &e.ben_cookie,
            &actions,
            &json!({"title": "Erledigt", "owner_user_id": ben}),
        )
        .await;
    let (status, _) = api
        .patch(
            &e.ben_cookie,
            &record(&e, "actions", &done),
            &json!({"status": "done", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    api.created(
        &e.owner,
        &actions,
        &json!({"title": "Vom Chef", "owner_user_id": ben}),
    )
    .await;
    let supplier = api.institution(&e.owner, "Testwil Generatoren AG").await;
    api.created(
        &e.ben_cookie,
        &format!("/api/v1/events/{}/commitments", e.id),
        &json!({
            "text": "Generator delivery Friday 15:00",
            "condition": "subject to signed order",
            "promisor": {"kind": "institution", "id": supplier},
            "owner_user_id": ben,
        }),
    )
    .await;

    let (status, work) = api.get(&e.ben_cookie, "/api/v1/me/work").await;
    assert_eq!(status, StatusCode::OK, "{work}");
    let titles: Vec<_> = work["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Früh", "Spät", "Ohne Datum", "Vom Chef"]);
    assert_eq!(work["actions"][0]["event_key"], "TEST30");
    assert_eq!(work["actions"][0]["local_id"], "ACT-003");
    assert_eq!(work["commitments"].as_array().unwrap().len(), 1);
    assert_eq!(work["commitments"][0]["event_key"], "TEST30");
    assert_eq!(work["commitments"][0]["local_id"], "COM-001");
    assert_eq!(work["review_count"], 0);

    let (status, work) = api.get(&e.viewer, "/api/v1/me/work").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(work["actions"], json!([]));
    assert_eq!(work["commitments"], json!([]));
}

#[tokio::test]
async fn my_work_drops_events_where_i_lost_my_role() {
    let api = Api::start().await;
    let e = api.event().await;
    api.created(
        &e.owner,
        &format!("/api/v1/events/{}/actions", e.id),
        &json!({"title": "Generator bestellen", "owner_user_id": e.ben.as_uuid()}),
    )
    .await;
    let (_, before) = api.get(&e.ben_cookie, "/api/v1/me/work").await;
    assert_eq!(before["actions"].as_array().unwrap().len(), 1);

    let (status, _) = api
        .post(
            &e.owner,
            &format!(
                "/api/v1/events/{}/memberships/{}/remove",
                e.id,
                e.ben.as_uuid()
            ),
            &json!({"expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, after) = api.get(&e.ben_cookie, "/api/v1/me/work").await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(after["actions"], json!([]));
    assert_eq!(after["commitments"], json!([]));
}

#[tokio::test]
async fn my_work_includes_events_without_membership_for_an_admin() {
    let api = Api::start().await;
    let e = api.event().await;
    let (_, admin, admin_cookie) = api.test.member("testwil", OrganizationRole::Admin).await;
    api.created(
        &admin_cookie,
        &format!("/api/v1/events/{}/actions", e.id),
        &json!({"title": "Generator bestellen", "owner_user_id": admin.as_uuid()}),
    )
    .await;

    let (status, work) = api.get(&admin_cookie, "/api/v1/me/work").await;
    assert_eq!(status, StatusCode::OK, "{work}");
    assert_eq!(work["actions"].as_array().unwrap().len(), 1);
    assert_eq!(work["actions"][0]["event_key"], "TEST30");
}
