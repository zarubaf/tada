//! The supplier loop of Slice 2a end to end, through HTTP and MCP with invented data.
//!
//! A contributor's AI client proposes a supplier and its conditional promise. The designated workstream lead reviews
//! it without the event manager relaying the message. The promise stays conditional until the lead makes it firm
//! with a reason; no signature or approval is invented on the way.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};
use support::{SESSION_COOKIE, TestClock};
use tada_app::caller::OrganizationRole;
use tada_app::clock::Clock;
use tada_app::domain::ids::UserId;
use tada_app::tokens::TokenAuthenticator;
use tada_mcp::McpState;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// The text of C's message. The supplier, the promise and the request to make it firm quote it.
const SOURCE: &str = "Testwil Generatoren AG liefert den Generator am Freitag um 15 Uhr, \
                      sofern die Bestellung unterschrieben ist. Die Bestellung ist noch nicht unterschrieben.";
const SUPPLIER_QUOTE: &str = "Testwil Generatoren AG";
const PROMISE_QUOTE: &str =
    "liefert den Generator am Freitag um 15 Uhr, sofern die Bestellung unterschrieben ist";
const UNSIGNED_QUOTE: &str = "Die Bestellung ist noch nicht unterschrieben.";

/// The API and the MCP server on one database and one clock that the test moves, as `serve` connects them.
struct App {
    router: Router,
    test: TestDatabase,
    clock: Arc<TestClock>,
}

struct Member {
    user: UserId,
    cookie: String,
}

impl App {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        // Whole seconds: the database keeps microseconds, so a stored time equals the time of the clock.
        let start = Timestamp::from_second(Timestamp::now().as_second()).unwrap();
        let clock = Arc::new(TestClock::new(start));
        let database = Arc::new(test.database.clone());
        let api = support::session_state(&test, clock.clone());
        let mcp = McpState {
            authenticator: Arc::new(TokenAuthenticator::new(
                database.clone(),
                database.clone(),
                clock.clone(),
            )),
            events: database.clone(),
            identity: database.clone(),
            facts: database.clone(),
            sources: database.clone(),
            proposals: database.clone(),
            documents: database.clone(),
            clock: clock.clone(),
            public_url: support::public_url(),
            workstreams: database.clone(),
            parties: database.clone(),
            work: database,
        };
        let router = tada::serve::routes(api, mcp, None);
        Self {
            router,
            test,
            clock,
        }
    }

    /// A new member of Testwil, signed in at the time of the clock.
    async fn member(&self, role: OrganizationRole) -> Member {
        let (organization, user, _) = self.test.member("testwil", role).await;
        let cookie = self
            .test
            .sign_in(user, Some(organization), self.clock.now())
            .await;
        Member { user, cookie }
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

    async fn get(&self, member: &Member, path: &str) -> Value {
        let (status, value) = self.send(&member.cookie, Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    async fn post(&self, member: &Member, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(&member.cookie, Method::POST, path, Some(body))
            .await
    }

    async fn created(&self, member: &Member, path: &str, body: &Value) -> Value {
        let (status, value) = self.post(member, path, body).await;
        assert_eq!(status, StatusCode::CREATED, "{path}: {value}");
        value
    }

    async fn patch(&self, member: &Member, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(&member.cookie, Method::PATCH, path, Some(body))
            .await
    }

    /// The IDs of the changesets in the Review Inbox of the member.
    async fn inbox(&self, member: &Member) -> Vec<String> {
        let inbox = self.get(member, "/api/v1/changesets?status=open").await;
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_owned())
            .collect()
    }

    /// A `propose` API token of the member. Returns its secret.
    async fn propose_token(&self, member: &Member) -> String {
        let expires = self.clock.now() + SignedDuration::from_hours(24);
        let token = self
            .created(
                member,
                "/api/v1/tokens",
                &json!({
                    "name": "Claude Code", "scope": "propose",
                    "expires_at": expires.to_string(), "notice_version_confirmed": 1,
                }),
            )
            .await;
        token["secret"].as_str().unwrap().to_owned()
    }

    /// Calls the MCP tool `propose_changeset` with `token`. Returns the ID of the changeset.
    async fn mcp_propose(&self, token: &str, event: &str, proposals: Vec<Value>) -> String {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "propose_changeset", "arguments": {
                "event_id": event, "source_text": SOURCE, "proposals": proposals,
            }},
        });
        // An MCP client sends no `Origin`; `support::request` would add one to a POST.
        let request = support::request(Method::GET, "/mcp")
            .method(Method::POST)
            .header(header::HOST, "tada.example.org")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (response, body) = support::send(&self.router, request).await;
        assert_eq!(response.status(), StatusCode::OK, "{body}");
        assert_eq!(body["result"]["isError"], false, "{body}");
        body["result"]["structuredContent"]["changeset_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

/// A proposal that cites the passage `quote` of `SOURCE`.
fn proposal(operation: Value, quote: &str, reason: &str) -> Value {
    let start = SOURCE[..SOURCE.find(quote).unwrap()].chars().count();
    json!({
        "id": Uuid::now_v7(), "operation": operation, "reason": reason,
        "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
    })
}

#[tokio::test]
async fn the_lead_reviews_a_supplier_promise_and_the_condition_stays_until_made_firm() {
    let app = App::start().await;

    // 1. An owner creates the event and adds the event manager. The manager creates the workstream with the lead.
    let owner = app.member(OrganizationRole::Owner).await;
    let manager = app.member(OrganizationRole::Member).await;
    let lead = app.member(OrganizationRole::Member).await;
    let contributor = app.member(OrganizationRole::Member).await;
    let event = app
        .created(
            &owner,
            "/api/v1/events",
            &json!({"key": "TEST30", "name": "Open Day Testwil"}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (member, role) in [
        (&manager, "event-manager"),
        (&lead, "event-contributor"),
        (&contributor, "event-contributor"),
    ] {
        app.created(
            &owner,
            &format!("/api/v1/events/{event}/memberships"),
            &json!({"user_id": member.user.as_uuid(), "event_role": role}),
        )
        .await;
    }
    let workstream = app
        .created(
            &manager,
            &format!("/api/v1/events/{event}/workstreams"),
            &json!({"name": "Bodenbetrieb", "lead_user_id": lead.user.as_uuid()}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // 2. The contributor's AI client proposes the supplier and its conditional promise through MCP.
    let token = app.propose_token(&contributor).await;
    let (institution, commitment) = (Uuid::now_v7(), Uuid::now_v7());
    let new_institution = proposal(
        json!({"kind": "create-institution", "id": institution,
               "name": "Testwil Generatoren AG", "institution_kind": "company"}),
        SUPPLIER_QUOTE,
        "The message names the supplier.",
    );
    let mut promise = proposal(
        json!({
            "kind": "create-commitment", "id": commitment, "event_id": event,
            "text": "Generator delivery Friday 15:00", "condition": "subject to signed order",
            "promisor": {"institution": institution}, "owner": lead.user.as_uuid(),
            "workstream": workstream,
        }),
        PROMISE_QUOTE,
        "The supplier promises the delivery if the order is signed.",
    );
    promise["depends_on"] = json!([new_institution["id"]]);
    let changeset = app
        .mcp_propose(
            &token,
            &event,
            vec![new_institution.clone(), promise.clone()],
        )
        .await;

    // 3. The lead sees the changeset; the event manager does not before the proposals are overdue (3 days).
    assert_eq!(app.inbox(&lead).await, [changeset.as_str()]);
    assert_eq!(app.inbox(&manager).await, Vec::<String>::new());
    // Both open proposals are routed to the lead: the promise by its workstream, the new supplier by the promise.
    let work = app.get(&lead, "/api/v1/me/work").await;
    assert_eq!(work["review_count"], 2);
    assert_eq!(
        app.get(&manager, "/api/v1/me/work").await["review_count"],
        0
    );

    // 4. The lead applies both proposals in one request.
    let (status, applied) = app
        .post(
            &lead,
            &format!("/api/v1/changesets/{changeset}/apply"),
            &json!({"selected": [new_institution["id"], promise["id"]]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(
        applied["institutions"],
        json!([{"id": institution, "local_id": "INS-001"}])
    );
    assert_eq!(
        applied["commitments"],
        json!([{"id": commitment, "local_id": "COM-001"}])
    );
    let institutions: i64 = app.test.scalar("SELECT count(*) FROM institution").await;
    assert_eq!(institutions, 1, "the institution exists once");
    let path = format!("/api/v1/events/{event}/commitments/{commitment}");
    let read = app.get(&lead, &path).await;
    assert_eq!(read["local_id"], "COM-001");
    assert_eq!(read["status"], "conditional");
    assert_eq!(read["condition"], "subject to signed order");
    assert_eq!(read["promisor"]["local_id"], "INS-001");
    assert_eq!(read["firm_reason"], Value::Null);
    assert_eq!(read["evidence"][0]["quote"], PROMISE_QUOTE);
    assert_eq!(app.inbox(&lead).await, Vec::<String>::new());

    // 5. A change of the due date keeps the condition. The status `firm` needs "make firm".
    let (status, changed) = app
        .patch(
            &lead,
            &path,
            &json!({"due_date": "2030-06-07", "expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["status"], "conditional");
    let (status, problem) = app
        .patch(
            &lead,
            &path,
            &json!({"status": "firm", "expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "invalid-transition");

    // 6. A proposal of the AI client to make it firm changes nothing until the lead applies it.
    let to_firm = proposal(
        json!({
            "kind": "change-commitment-status", "event_id": event, "commitment_id": commitment,
            "status": "firm", "expected_version": 2,
        }),
        UNSIGNED_QUOTE,
        "The AI client guesses that the order is fine.",
    );
    let pending = app.mcp_propose(&token, &event, vec![to_firm]).await;
    assert_eq!(app.inbox(&lead).await, [pending.as_str()]);
    let read = app.get(&lead, &path).await;
    assert_eq!(read["status"], "conditional");
    assert_eq!(read["version"], 2);

    // The lead makes it firm with a reason. The condition text stays.
    let (status, made) = app
        .post(
            &lead,
            &format!("{path}/firm"),
            &json!({"reason": "The order is signed.", "expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["status"], "firm");
    assert_eq!(made["firm_reason"], "The order is signed.");
    assert_eq!(made["condition"], "subject to signed order");

    // The pending proposal expects version 2, so it conflicts now and changes nothing.
    let review = app
        .get(&lead, &format!("/api/v1/changesets/{pending}"))
        .await;
    let to_firm = &review["proposals"][0];
    let (status, problem) = app
        .post(
            &lead,
            &format!("/api/v1/changesets/{pending}/apply"),
            &json!({"selected": [to_firm["id"]]}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "record-version-conflict");
    let review = app
        .get(&lead, &format!("/api/v1/changesets/{pending}"))
        .await;
    assert_eq!(review["proposals"][0]["status"], "conflict");
    assert_eq!(
        app.get(&lead, &path).await["firm_reason"],
        "The order is signed."
    );

    // The audit log names the lead as the actor and holds no reason text (ADR 0068).
    let actors: Vec<Uuid> = firm_actors(&app).await;
    assert_eq!(actors, [lead.user.as_uuid()]);
    let leaked: i64 = app
        .test
        .scalar("SELECT count(*) FROM audit_event WHERE audit_event::text LIKE '%order is signed%'")
        .await;
    assert_eq!(leaked, 0, "the audit log holds no reason text");
}

/// The actors of the make-firm events of the audit log.
async fn firm_actors(app: &App) -> Vec<Uuid> {
    let actors: Value = app
        .test
        .scalar(
            "SELECT coalesce(jsonb_agg(actor_id ORDER BY occurred_at, id), '[]'::jsonb)
             FROM audit_event WHERE action = 'commitment.firm' AND actor_kind = 'member' AND channel = 'web'",
        )
        .await;
    serde_json::from_value(actors).unwrap()
}
