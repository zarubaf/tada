//! Draft versions over HTTP, with PostgreSQL and real sessions (ADR 0051, ADR 0052):
//! the approval, „Fakten geändert“, the difference of two versions, the rendering and the draft in the review.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::Timestamp;
use serde_json::{Value, json};
use support::{SESSION_COOKIE, TestClock};
use tada_app::caller::OrganizationRole;
use tada_app::clock::Clock;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const SOURCE: &str = "Das Open Day findet im Mai 2030 auf dem Flugfeld statt.";

/// The API with one organization, its owner, and the event OPEN30 that the owner manages.
struct Api {
    router: axum::Router,
    test: TestDatabase,
    clock: Arc<TestClock>,
    owner: String,
    event: String,
}

impl Api {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let start = Timestamp::from_second(Timestamp::now().as_second()).unwrap();
        let clock = Arc::new(TestClock::new(start));
        let router = support::session_router(&test, clock.clone());
        let (organization, user, _) = test.member("testwil", OrganizationRole::Owner).await;
        let owner = test.sign_in(user, Some(organization), clock.now()).await;
        let mut api = Self {
            router,
            test,
            clock,
            owner,
            event: String::new(),
        };
        let (status, event) = api
            .post(
                &api.owner,
                "/api/v1/events",
                &json!({"key": "OPEN30", "name": "Open Day Testwil"}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{event}");
        api.event = event["id"].as_str().unwrap().to_owned();
        api
    }

    /// A new member of the organization with the event role `role` in OPEN30. Returns its cookie.
    async fn with_role(&self, role: &str) -> String {
        let (organization, user, _) = self.test.member("testwil", OrganizationRole::Member).await;
        let (status, _) = self
            .post(
                &self.owner,
                &format!("/api/v1/events/{}/memberships", self.event),
                &json!({"user_id": user.as_uuid(), "event_role": role}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        self.test
            .sign_in(user, Some(organization), self.clock.now())
            .await
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

    /// Proposes one operation with the passage `quote` of `SOURCE` as evidence. Returns the changeset.
    async fn propose(&self, operation: Value, quote: &str) -> Value {
        let start = SOURCE[..SOURCE.find(quote).unwrap()].chars().count();
        let body = json!({
            "source_text": SOURCE,
            "proposals": [{
                "id": Uuid::now_v7(),
                "operation": operation,
                "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
                "reason": "The member wrote it.",
            }],
        });
        let path = format!("/api/v1/events/{}/changesets", self.event);
        let (status, created) = self.post(&self.owner, &path, &body).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        created
    }

    async fn apply(&self, changeset: &Value) {
        let path = format!(
            "/api/v1/changesets/{}/apply",
            changeset["id"].as_str().unwrap()
        );
        let body = json!({"selected": changeset["proposal_ids"]});
        let (status, applied) = self.post(&self.owner, &path, &body).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
    }

    /// Sets the date window of OPEN30 to the month `month` of 2030. Returns the ID of the fact.
    async fn set_date(&self, month: u8, expected_version: Option<i64>) -> String {
        let (_, fields) = self
            .get(
                &self.owner,
                &format!("/api/v1/events/{}/fields", self.event),
            )
            .await;
        let field = fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == "date_window")
            .unwrap()["id"]
            .clone();
        let operation = json!({
            "kind": "set-fact", "event_id": self.event, "field_id": field, "expected_version": expected_version,
            "state": {"state": "accepted", "value": {"type": "date-window",
                "start": format!("2030-{month:02}-01"), "end": format!("2030-{month:02}-28"), "granularity": "month"}},
        });
        let changeset = self.propose(operation, "im Mai 2030").await;
        self.apply(&changeset).await;
        let (_, profile) = self
            .get(
                &self.owner,
                &format!("/api/v1/events/{}/profile", self.event),
            )
            .await;
        profile["facts"][0]["id"].as_str().unwrap().to_owned()
    }

    /// Proposes a draft of `document` and returns the changeset.
    async fn propose_draft(&self, document: Value, markdown: &str) -> Value {
        let operation = json!({
            "kind": "create-document-draft", "event_id": self.event,
            "document": document, "markdown": markdown,
        });
        self.propose(operation, "Das Open Day").await
    }

    /// Proposes and applies a draft of `document`. Returns the ID of the newest version of the document.
    async fn add_draft(&self, document: &Uuid, target: Value, markdown: &str) -> String {
        let changeset = self.propose_draft(target, markdown).await;
        self.apply(&changeset).await;
        let (_, document) = self
            .get(&self.owner, &format!("/api/v1/documents/{document}"))
            .await;
        document["newest_version"]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn approve(&self, cookie: &str, version: &str, expected: i64) -> (StatusCode, Value) {
        self.post(
            cookie,
            &format!("/api/v1/document-versions/{version}/approve"),
            &json!({"expected_version": expected}),
        )
        .await
    }
}

fn new_document(id: &Uuid) -> Value {
    json!({"new": {"id": id, "name": "Konzept Open Day"}})
}

fn existing(id: &Uuid, expected_version: i64) -> Value {
    json!({"existing": {"document_id": id, "expected_version": expected_version}})
}

/// Acceptance (7): nobody can overwrite an approved version. Only an event manager approves (ADR 0052).
#[tokio::test]
async fn an_event_manager_approves_and_an_approved_version_stays_unchanged() {
    let api = Api::start().await;
    let document = Uuid::now_v7();
    let first = api
        .add_draft(&document, new_document(&document), "Erste Fassung.\n")
        .await;

    for role in ["event-viewer", "event-contributor"] {
        let cookie = api.with_role(role).await;
        let (status, problem) = api.approve(&cookie, &first, 1).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role}: {problem}");
        assert_eq!(problem["code"], "forbidden");
    }
    let (status, approved) = api.approve(&api.owner, &first, 1).await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["id"], first);
    assert_eq!(approved["status"], "approved");
    let (status, problem) = api.approve(&api.owner, &first, 1).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "invalid-transition");

    let second = api
        .add_draft(&document, existing(&document, 1), "Zweite Fassung.\n")
        .await;
    let (_, versions) = api
        .get(
            &api.owner,
            &format!("/api/v1/documents/{document}/versions"),
        )
        .await;
    let statuses: Vec<_> = versions["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| (version["number"].clone(), version["status"].clone()))
        .collect();
    assert_eq!(
        statuses,
        [(json!(1), json!("approved")), (json!(2), json!("draft"))]
    );
    let (status, problem) = api.approve(&api.owner, &second, 1).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "record-version-conflict");
    let (status, _) = api.approve(&api.owner, &second, 2).await;
    assert_eq!(status, StatusCode::OK);
    let (_, versions) = api
        .get(
            &api.owner,
            &format!("/api/v1/documents/{document}/versions"),
        )
        .await;
    assert_eq!(versions["items"][0]["status"], "superseded");
    assert_eq!(versions["items"][1]["status"], "approved");

    let (status, rendering) = api
        .get(
            &api.owner,
            &format!("/api/v1/document-versions/{first}/rendering"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rendering}");
    assert_eq!(rendering["draft"]["markdown"], "Erste Fassung.\n");
}

/// Demonstration steps 6 and 7: a new date shows „Fakten geändert“, and the new draft shows the differences.
#[tokio::test]
async fn a_changed_date_window_shows_facts_changed_and_the_differences() {
    let api = Api::start().await;
    let fact = api.set_date(5, None).await;
    let document = Uuid::now_v7();
    let text = |version: i64, month: &str| {
        format!(
            "# Konzept\n\nDas Open Day ist am [](tada:fact/{fact}?v={version}).\nEs ist im {month}.\n"
        )
    };
    let first = api
        .add_draft(&document, new_document(&document), &text(1, "Mai"))
        .await;
    let document_path = format!("/api/v1/documents/{document}");
    let (_, read) = api.get(&api.owner, &document_path).await;
    assert_eq!(read["facts_changed"], false, "{read}");

    api.set_date(6, Some(1)).await;
    let (_, read) = api.get(&api.owner, &document_path).await;
    assert_eq!(read["facts_changed"], true, "{read}");

    let second = api
        .add_draft(&document, existing(&document, 1), &text(2, "Juni"))
        .await;
    let (_, read) = api.get(&api.owner, &document_path).await;
    assert_eq!(read["facts_changed"], false);

    let (status, diff) = api
        .get(
            &api.owner,
            &format!("/api/v1/documents/{document}/diff?from={first}&to={second}"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{diff}");
    assert_eq!(
        diff["facts"],
        json!({"changed": [{"fact_id": fact, "from": 1, "to": 2}], "added": [], "removed": []})
    );
    let changed: Vec<_> = diff["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|line| line["kind"] != "unchanged")
        .map(|line| (line["kind"].clone(), line["text"].clone()))
        .collect();
    assert_eq!(
        changed,
        [
            (
                json!("removed"),
                json!(format!("Das Open Day ist am [](tada:fact/{fact}?v=1)."))
            ),
            (json!("removed"), json!("Es ist im Mai.")),
            (
                json!("added"),
                json!(format!("Das Open Day ist am [](tada:fact/{fact}?v=2)."))
            ),
            (json!("added"), json!("Es ist im Juni.")),
        ]
    );
    assert_eq!(
        diff["lines"][0],
        json!({"kind": "unchanged", "old_line": 1, "new_line": 1, "text": "# Konzept"})
    );

    // The rendering of the first version shows the value of the version that it cites, not the current one.
    let (status, rendering) = api
        .get(
            &api.owner,
            &format!("/api/v1/document-versions/{first}/rendering"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rendering}");
    let link = &rendering["draft"]["links"][format!("tada:fact/{fact}?v=1")];
    assert_eq!(link["kind"], "fact", "{rendering}");
    assert_eq!(link["state"], "accepted");
    assert_eq!(link["version"], 1);
    assert_eq!(link["value"]["start"], "2030-05-01");
}

/// The Review Inbox shows a draft proposal with its Markdown, its lint warnings and the target of each link.
#[tokio::test]
async fn the_review_of_a_draft_proposal_shows_its_markdown_lint_warnings_and_links() {
    let api = Api::start().await;
    let fact = api.set_date(5, None).await;
    let markdown = format!(
        "Das Open Day ist am [](tada:fact/{fact}?v=1).\nWir erwarten 20000 Gäste.\n<b>fett</b>\n"
    );
    let changeset = api
        .propose_draft(new_document(&Uuid::now_v7()), &markdown)
        .await;

    let (status, review) = api
        .get(
            &api.owner,
            &format!("/api/v1/changesets/{}", changeset["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{review}");
    let draft = &review["proposals"][0]["draft"];
    assert_eq!(draft["markdown"], markdown);
    assert_eq!(
        draft["lint_warnings"],
        json!([{"line": 2, "kind": "number"}, {"line": 3, "kind": "raw-html"}])
    );
    let links = draft["links"].as_object().unwrap();
    assert_eq!(links.len(), 1);
    let link = &links[&format!("tada:fact/{fact}?v=1")];
    assert_eq!(link["kind"], "fact");
    assert_eq!(link["fact_id"], fact);
    assert_eq!(link["value"]["type"], "date-window");
}

#[tokio::test]
async fn an_upload_version_and_a_version_of_another_document_are_not_found() {
    let api = Api::start().await;
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    let first = api.add_draft(&a, new_document(&a), "A.\n").await;
    let other = api.add_draft(&b, new_document(&b), "B.\n").await;

    let (status, _) = api
        .get(
            &api.owner,
            &format!("/api/v1/documents/{a}/diff?from={first}&to={other}"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api
        .get(
            &api.owner,
            &format!("/api/v1/document-versions/{}/rendering", Uuid::now_v7()),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (organization, user, _) = api
        .test
        .member("musterhausen", OrganizationRole::Owner)
        .await;
    let stranger = api
        .test
        .sign_in(user, Some(organization), api.clock.now())
        .await;
    let (status, _) = api
        .get(
            &stranger,
            &format!("/api/v1/document-versions/{first}/rendering"),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api.approve(&stranger, &first, 1).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
