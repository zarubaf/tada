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

/// The source text of the proposals of the closing tests.
const SOURCE: &str = "Das Open Day findet im Mai 2030 statt. Wer klärt die Bewilligung?";

/// An event of Testwil with its people: the owner, who created it, an admin, an event manager
/// and an event contributor (ADR 0071).
struct Closing {
    api: Api,
    event: String,
    version: i64,
    admin: String,
    manager: String,
    contributor: String,
    contributor_user: UserId,
}

impl Closing {
    async fn start() -> Self {
        let api = Api::start().await;
        let (_, event) = api
            .post(
                "/api/v1/events",
                &json!({"key": "OPEN30", "name": "Open Day Testwil"}),
            )
            .await;
        let id = event["id"].as_str().unwrap().to_owned();
        let (_, _, admin) = api.test.member("testwil", OrganizationRole::Admin).await;
        let (_, manager_user, manager) = api.test.member("testwil", OrganizationRole::Member).await;
        let (_, contributor_user, contributor) =
            api.test.member("testwil", OrganizationRole::Member).await;
        for (user, role) in [
            (manager_user, "event-manager"),
            (contributor_user, "event-contributor"),
        ] {
            let (response, _) = api
                .post(
                    &format!("/api/v1/events/{id}/memberships"),
                    &json!({"user_id": user.as_uuid(), "event_role": role}),
                )
                .await;
            assert_eq!(response.status(), StatusCode::CREATED);
        }
        Self {
            api,
            event: id,
            version: 1,
            admin: Self::cookie(admin),
            manager: Self::cookie(manager),
            contributor: Self::cookie(contributor),
            contributor_user,
        }
    }

    fn cookie(value: String) -> String {
        format!("{}={value}", support::SESSION_COOKIE)
    }

    async fn send_as(
        &self,
        cookie: &str,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let request = support::request(method, path).header(header::COOKIE, cookie);
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = self.api.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn post_as(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send_as(cookie, Method::POST, path, Some(body)).await
    }

    /// Closes (`action` is `close`) or reopens (`reopen`) the event with the version that the test knows.
    async fn change(&self, cookie: &str, action: &str, version: i64) -> (StatusCode, Value) {
        self.post_as(
            cookie,
            &format!("/api/v1/events/{}/{action}", self.event),
            &json!({"expected_version": version}),
        )
        .await
    }

    /// Proposes `operation` with `quote` of `SOURCE` as evidence, as the contributor. Returns the status and the body.
    async fn propose(&self, operation: Value, quote: &str) -> (StatusCode, Value) {
        let start = SOURCE[..SOURCE.find(quote).unwrap()].chars().count();
        let body = json!({
            "source_text": SOURCE,
            "proposals": [{
                "id": uuid::Uuid::now_v7(),
                "operation": operation,
                "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
                "reason": "The member wrote it.",
            }],
        });
        self.post_as(
            &self.contributor,
            &format!("/api/v1/events/{}/changesets", self.event),
            &body,
        )
        .await
    }

    fn question(&self) -> Value {
        json!({"kind": "create-open-question", "id": uuid::Uuid::now_v7(), "event_id": self.event,
               "text": "Wer klärt die Bewilligung?", "owner": self.contributor_user.as_uuid()})
    }

    fn draft(&self) -> Value {
        json!({"kind": "create-document-draft", "event_id": self.event,
               "document": {"new": {"id": uuid::Uuid::now_v7(), "name": "Konzept"}}, "markdown": "# Konzept"})
    }
}

#[tokio::test]
async fn manager_closes_owner_reopens() {
    let c = Closing::start().await;

    let (status, closed) = c.change(&c.manager, "close", 1).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(closed["status"], "closed");
    assert_eq!(closed["version"], 2);
    assert_eq!(closed["redacted"], false);
    assert!(closed["closed_at"].is_string(), "{closed}");

    // The event stays readable, and closing twice is no transition.
    let (status, read) = c
        .send_as(
            &c.contributor,
            Method::GET,
            &format!("/api/v1/events/{}", c.event),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, closed);
    let (status, problem) = c.change(&c.admin, "close", 2).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "invalid-transition");

    let (status, problem) = c.change(&c.api.cookie, "reopen", 1).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "record-version-conflict");
    let (status, reopened) = c.change(&c.api.cookie, "reopen", 2).await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(reopened["status"], "active");
    assert_eq!(reopened["version"], 3);
    assert!(reopened.get("closed_at").is_none(), "{reopened}");

    let actions: String = c
        .api
        .test
        .scalar("SELECT string_agg(action, ',' ORDER BY id) FROM audit_event WHERE action LIKE 'event.%'")
        .await;
    assert_eq!(actions, "event.create,event.close,event.reopen");
}

#[tokio::test]
async fn contributor_cannot_close() {
    let c = Closing::start().await;
    let (status, problem) = c.change(&c.contributor, "close", 1).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "forbidden");
    let (status, event) = c.change(&c.admin, "close", 1).await;
    assert_eq!(status, StatusCode::OK, "an admin closes: {event}");
}

#[tokio::test]
async fn manager_cannot_reopen() {
    let mut c = Closing::start().await;
    c.change(&c.manager, "close", 1).await;
    c.version = 2;
    let (status, problem) = c.change(&c.manager, "reopen", c.version).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let (status, event) = c.change(&c.admin, "reopen", c.version).await;
    assert_eq!(status, StatusCode::OK, "an admin reopens: {event}");
}

#[tokio::test]
async fn reopen_after_redaction_is_invalid_transition() {
    let c = Closing::start().await;
    c.change(&c.manager, "close", 1).await;
    c.api
        .test
        .redact(&["UPDATE event SET redacted_at = now()"])
        .await;

    let (status, problem) = c.change(&c.api.cookie, "reopen", 2).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "invalid-transition");
    let (_, read) = c
        .send_as(
            &c.api.cookie,
            Method::GET,
            &format!("/api/v1/events/{}", c.event),
            None,
        )
        .await;
    assert_eq!(read["status"], "closed");
    assert_eq!(read["redacted"], true);
}

/// ADR 0071: a closed event refuses each write with `event-closed`, and each read still works.
/// The uploads and the MCP proposal have their own tests (`document_uploads.rs`, `mcp.rs`).
#[tokio::test]
async fn closed_event_refuses_every_write() {
    let c = Closing::start().await;
    let event = &c.event;
    // Before the closing: an open proposal and a commitment to change.
    let (status, open) = c.propose(c.question(), "Wer klärt die Bewilligung?").await;
    assert_eq!(status, StatusCode::CREATED, "{open}");
    let (status, action) = c
        .post_as(
            &c.contributor,
            &format!("/api/v1/events/{event}/actions"),
            &json!({"title": "Generator bestellen", "owner_user_id": c.contributor_user.as_uuid()}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{action}");

    // A draft version that waits for its approval.
    let document = uuid::Uuid::now_v7();
    let operation = json!({"kind": "create-document-draft", "event_id": event,
        "document": {"new": {"id": document, "name": "Konzept"}}, "markdown": "# Konzept"});
    let (status, draft) = c.propose(operation, "Das Open Day").await;
    assert_eq!(status, StatusCode::CREATED, "{draft}");
    let (status, applied) = c
        .post_as(
            &c.api.cookie,
            &format!("/api/v1/changesets/{}/apply", draft["id"].as_str().unwrap()),
            &json!({"selected": draft["proposal_ids"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let (_, shown) = c
        .send_as(
            &c.api.cookie,
            Method::GET,
            &format!("/api/v1/documents/{document}"),
            None,
        )
        .await;
    let version = shown["newest_version"]["id"].as_str().unwrap().to_owned();

    let (status, closed) = c.change(&c.manager, "close", 1).await;
    assert_eq!(status, StatusCode::OK, "{closed}");

    let actions = format!("/api/v1/events/{event}/actions");
    let action_path = format!("{actions}/{}", action["id"].as_str().unwrap());
    let new_member = c
        .api
        .test
        .member("testwil", OrganizationRole::Member)
        .await
        .1;
    let apply = format!("/api/v1/changesets/{}/apply", open["id"].as_str().unwrap());
    let reject = format!("/api/v1/changesets/{}/reject", open["id"].as_str().unwrap());
    let writes = [
        (
            "create action",
            Method::POST,
            actions.clone(),
            json!({"title": "X", "owner_user_id": c.contributor_user.as_uuid()}),
        ),
        (
            "change action",
            Method::PATCH,
            action_path,
            json!({"title": "Y", "expected_version": 1}),
        ),
        (
            "create workstream",
            Method::POST,
            format!("/api/v1/events/{event}/workstreams"),
            json!({"name": "Gelände", "lead_user_id": c.contributor_user.as_uuid()}),
        ),
        (
            "add event member",
            Method::POST,
            format!("/api/v1/events/{event}/memberships"),
            json!({"user_id": new_member.as_uuid(), "event_role": "event-viewer"}),
        ),
        (
            "approve a draft version",
            Method::POST,
            format!("/api/v1/document-versions/{version}/approve"),
            json!({"expected_version": 1}),
        ),
        (
            "apply an open proposal",
            Method::POST,
            apply,
            json!({"selected": open["proposal_ids"]}),
        ),
        (
            "reject an open proposal",
            Method::POST,
            reject,
            json!({"proposal_ids": open["proposal_ids"]}),
        ),
    ];
    for (name, method, path, body) in writes {
        // The owner could write in an active event, so only the closing refuses.
        let (status, problem) = c.send_as(&c.api.cookie, method, &path, Some(&body)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{name}: {problem}");
        assert_eq!(problem["code"], "event-closed", "{name}");
    }
    for (name, operation, quote) in [
        ("propose", c.question(), "Wer klärt die Bewilligung?"),
        ("create a draft", c.draft(), "Das Open Day"),
    ] {
        let (status, problem) = c.propose(operation, quote).await;
        assert_eq!(status, StatusCode::CONFLICT, "{name}: {problem}");
        assert_eq!(problem["code"], "event-closed", "{name}");
    }

    for path in [
        format!("/api/v1/events/{event}"),
        actions,
        format!("/api/v1/events/{event}/workstreams"),
        format!("/api/v1/events/{event}/memberships"),
    ] {
        let (status, _) = c.send_as(&c.contributor, Method::GET, &path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }

    // After the reopening, the event takes writes again.
    let (status, _) = c.change(&c.api.cookie, "reopen", 2).await;
    assert_eq!(status, StatusCode::OK);
    let (status, problem) = c.propose(c.question(), "Wer klärt die Bewilligung?").await;
    assert_eq!(status, StatusCode::CREATED, "{problem}");
}

/// ADR 0071: the open proposals of a closed event leave the Review Inbox and "My Work" and cannot be reviewed.
/// They come back when the event reopens.
#[tokio::test]
async fn a_closed_event_leaves_the_review_inbox_until_it_reopens() {
    let c = Closing::start().await;
    let (status, open) = c.propose(c.question(), "Wer klärt die Bewilligung?").await;
    assert_eq!(status, StatusCode::CREATED, "{open}");
    let id = open["id"].as_str().unwrap().to_owned();
    let owner = &c.api.cookie;

    let state = |label: &'static str| {
        let (id, c) = (id.clone(), &c);
        async move {
            let (_, inbox) = c
                .send_as(owner, Method::GET, "/api/v1/changesets?status=open", None)
                .await;
            let listed = inbox["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == id.as_str());
            let (_, work) = c.send_as(owner, Method::GET, "/api/v1/me/work", None).await;
            let (status, detail) = c
                .send_as(
                    owner,
                    Method::GET,
                    &format!("/api/v1/changesets/{id}"),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{label}: {detail}");
            let can_review = detail["proposals"][0]["can_review"].as_bool().unwrap();
            (listed, work["review_count"].as_u64().unwrap(), can_review)
        }
    };

    assert_eq!(state("active").await, (true, 1, true));
    let (status, closed) = c.change(&c.manager, "close", 1).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(state("closed").await, (false, 0, false));
    let (status, reopened) = c.change(owner, "reopen", 2).await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(state("reopened").await, (true, 1, true));
}
