//! The person features of ADR 0073 through the API: the mentions of a person, the export of the data about a person,
//! dispute notes and the redaction request. Only an owner uses them.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::organization::{Client, Filled, fill, organization, router};
use tada_adapters::storage::testing::TestGarage;
use tada_app::caller::OrganizationRole;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// An organization with an owner and one event.
struct World {
    test: TestDatabase,
    garage: TestGarage,
    owner: Client,
    event: String,
}

impl World {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (_, _, owner) = organization(&test, &garage, "testwil").await;
        let event = owner.create_event("OPEN30", "Open Day Testwil").await;
        Self::with(test, garage, owner, event)
    }

    /// An organization that `fill` filled, with a Telegram link of the owner to `telegram_user`.
    async fn filled(telegram_user: i64) -> (Self, Filled) {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, _, owner) = organization(&test, &garage, "testwil").await;
        let filled = fill(&test, &owner, organization, telegram_user).await;
        let event = filled.event.clone();
        (Self::with(test, garage, owner, event), filled)
    }

    fn with(test: TestDatabase, garage: TestGarage, owner: Client, event: String) -> Self {
        Self {
            test,
            garage,
            owner,
            event,
        }
    }

    /// A client of a new member of the organization `slug` with the role `role`.
    async fn client(&self, slug: &str, role: OrganizationRole) -> Client {
        let (_, _, cookie) = self.test.member(slug, role).await;
        Client {
            router: router(&self.test, &self.garage),
            cookie,
        }
    }

    /// A changeset with the source text `text` and a date proposal whose evidence is `quote`.
    async fn changeset(&self, text: &str, quote: &str) -> Value {
        let start = text[..text.find(quote).unwrap()].chars().count();
        let operation = self.owner.date_operation(&self.event).await;
        let body = json!({
            "source_text": text,
            "proposals": [{
                "id": Uuid::now_v7(),
                "operation": operation,
                "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
                "reason": "The member wrote it.",
            }],
        });
        self.owner
            .post(&format!("/api/v1/events/{}/changesets", self.event), &body)
            .await
    }

    async fn source_item(&self, changeset: &Value) -> Uuid {
        let version = changeset["source_version_id"].as_str().unwrap();
        self.test
            .scalar::<Uuid>(&format!(
                "SELECT source_item_id FROM source_version WHERE id = '{version}'"
            ))
            .await
    }
}

/// A request of `client` with a JSON body.
async fn call(
    client: &Client,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let request = support::request(method, path).header(header::CONTENT_TYPE, "application/json");
    let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
    client.call(request, body).await
}

#[tokio::test]
async fn only_owner_reads_mentions_exports_redacts_disputes() {
    let (world, filled) = World::filled(4_200_002).await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;
    let redaction = json!({"targets": [{"kind": "source-item", "id": item}], "reason": "request"});
    let note =
        json!({"target_kind": "source-item", "target_id": item, "text": "Das stimmt nicht."});

    let callers = [
        (
            world.client("testwil", OrganizationRole::Member).await,
            "forbidden",
        ),
        (
            world.client("testwil", OrganizationRole::Admin).await,
            "forbidden",
        ),
        (
            world.client("musterhausen", OrganizationRole::Owner).await,
            "not-found",
        ),
        (
            world.client("musterhausen", OrganizationRole::Admin).await,
            "not-found",
        ),
    ];
    for (client, code) in &callers {
        for (method, path, body) in [
            (Method::POST, "/api/v1/redactions", Some(&redaction)),
            (Method::POST, "/api/v1/dispute-notes", Some(&note)),
        ] {
            let (status, problem) = call(client, method, path, body).await;
            assert_eq!(problem["code"], *code, "{path}: {status} {problem}");
        }
    }

    // The REST API takes no API token, and the MCP server has no tool for these features (ADR 0073).
    let token = filled.secrets[3].as_str();
    for (method, path, body) in [
        (Method::POST, "/api/v1/dispute-notes", Some(&note)),
        (Method::POST, "/api/v1/redactions", Some(&redaction)),
    ] {
        let request = support::request(method, path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .unwrap();
        let (response, problem) = support::send(&world.owner.router, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{problem}");
    }
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let request = support::request(Method::POST, "/mcp")
        .header(header::HOST, "tada.example.org")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .body(Body::from(list.to_string()))
        .unwrap();
    let (response, body) = support::send(&world.owner.router, request).await;
    assert_eq!(response.status(), StatusCode::OK, "{body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        for word in ["redact", "dispute"] {
            assert!(!name.contains(word), "{name}");
        }
    }

    assert_eq!(
        world
            .test
            .scalar::<i64>(
                "SELECT (SELECT count(*) FROM redaction) + (SELECT count(*) FROM dispute_note)"
            )
            .await,
        0
    );
}

#[tokio::test]
async fn dispute_note_visible_to_readers_of_target() {
    let world = World::start().await;
    let changeset = world
        .changeset("Das Open Day ist im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;
    let proposal = changeset["proposal_ids"][0].as_str().unwrap().to_owned();
    for (kind, id) in [
        ("source-item", item.to_string()),
        ("proposal", proposal.clone()),
    ] {
        let (status, note) = call(
            &world.owner,
            Method::POST,
            "/api/v1/dispute-notes",
            Some(&json!({"target_kind": kind, "target_id": id, "text": format!("Einwand zu {kind}.")})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{note}");
        assert_eq!(note["text"], format!("Einwand zu {kind}."));
    }
    world.owner.apply(&changeset).await;
    let fact_version = world
        .test
        .scalar::<Uuid>("SELECT id FROM fact_version")
        .await;
    world
        .owner
        .post(
            "/api/v1/dispute-notes",
            &json!({"target_kind": "fact-version", "target_id": fact_version, "text": "Der Monat ist falsch."}),
        )
        .await;

    // An admin reads the event, so it sees the notes of the evidence, the proposal and the fact.
    let admin = world.client("testwil", OrganizationRole::Admin).await;
    let review = admin
        .get(&format!(
            "/api/v1/changesets/{}",
            changeset["id"].as_str().unwrap()
        ))
        .await;
    let proposal = &review["proposals"][0];
    assert_eq!(
        proposal["dispute_notes"][0]["text"], "Einwand zu proposal.",
        "{proposal}"
    );
    assert_eq!(
        proposal["evidence"][0]["dispute_notes"][0]["text"],
        "Einwand zu source-item."
    );
    assert!(proposal["evidence"][0]["dispute_notes"][0]["created_at"].is_string());
    let profile = admin
        .get(&format!("/api/v1/events/{}/profile", world.event))
        .await;
    let fact = &profile["facts"][0];
    assert_eq!(
        fact["dispute_notes"][0]["text"], "Der Monat ist falsch.",
        "{fact}"
    );
    assert_eq!(
        fact["evidence"][0]["dispute_notes"][0]["text"],
        "Einwand zu source-item."
    );

    // A text over 1000 characters is refused.
    let (status, problem) = call(
        &world.owner,
        Method::POST,
        "/api/v1/dispute-notes",
        Some(&json!({"target_kind": "source-item", "target_id": item, "text": "x".repeat(1001)})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["errors"][0]["pointer"], "/text");
    assert_eq!(
        world
            .test
            .scalar::<i64>("SELECT count(*) FROM audit_event WHERE action = 'dispute_note.create'")
            .await,
        3
    );
}

#[tokio::test]
async fn redaction_api_rejects_event_kind() {
    let world = World::start().await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;

    // The kind `event` is for the retention job only.
    let (status, problem) = call(
        &world.owner,
        Method::POST,
        "/api/v1/redactions",
        Some(&json!({"targets": [{"kind": "event", "id": world.event}], "reason": "request"})),
    )
    .await;
    assert_eq!(problem["code"], "validation-failed", "{status} {problem}");
    // A request names the reason `request`.
    let (_, problem) = call(
        &world.owner,
        Method::POST,
        "/api/v1/redactions",
        Some(&json!({"targets": [{"kind": "source-item", "id": item}], "reason": "retention"})),
    )
    .await;
    assert_eq!(problem["code"], "validation-failed", "{problem}");
    assert_eq!(
        world
            .test
            .scalar::<i64>("SELECT count(*) FROM redaction")
            .await,
        0
    );

    let body = json!({"targets": [{"kind": "source-item", "id": item}], "reason": "request"});
    let redacted = world.owner.post("/api/v1/redactions", &body).await;
    assert_eq!(
        redacted["applied"].as_array().unwrap().len(),
        1,
        "{redacted}"
    );
    assert_eq!(redacted["already"], 0);
    let again = world.owner.post("/api/v1/redactions", &body).await;
    assert_eq!(again, json!({"applied": [], "already": 1}));
}
