//! The person features of ADR 0073 through the API: the mentions of a person, the export of the data about a person,
//! dispute notes and the redaction request. Only an owner uses them.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::SESSION_COOKIE;
use support::files::pdf;
use support::organization::{Client, Filled, fill, organization, router};
use tada_adapters::storage::testing::TestGarage;
use tada_app::caller::OrganizationRole;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// An organization with an owner and one event.
struct World {
    test: TestDatabase,
    garage: TestGarage,
    organization: OrganizationId,
    owner_id: UserId,
    owner: Client,
    event: String,
}

impl World {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, owner_id, owner) = organization(&test, &garage, "testwil").await;
        let event = owner.create_event("OPEN30", "Open Day Testwil").await;
        Self::with(test, garage, organization, owner_id, owner, event)
    }

    /// An organization that `fill` filled, with a Telegram link of the owner to `telegram_user`.
    async fn filled(telegram_user: i64) -> (Self, Filled) {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, owner_id, owner) = organization(&test, &garage, "testwil").await;
        let filled = fill(&test, &owner, organization, telegram_user).await;
        let event = filled.event.clone();
        (
            Self::with(test, garage, organization, owner_id, owner, event),
            filled,
        )
    }

    fn with(
        test: TestDatabase,
        garage: TestGarage,
        organization: OrganizationId,
        owner_id: UserId,
        owner: Client,
        event: String,
    ) -> Self {
        Self {
            test,
            garage,
            organization,
            owner_id,
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

    async fn person(&self, body: Value) -> String {
        let person = self.owner.post("/api/v1/persons", &body).await;
        person["id"].as_str().unwrap().to_owned()
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

    async fn action(&self, title: &str) -> String {
        self.action_with(json!({"title": title})).await
    }

    async fn action_with(&self, mut body: Value) -> String {
        body["owner_user_id"] = json!(self.owner_id.as_uuid());
        let action = self
            .owner
            .post(&format!("/api/v1/events/{}/actions", self.event), &body)
            .await;
        action["id"].as_str().unwrap().to_owned()
    }

    async fn mentions(&self, person: &str) -> Vec<Value> {
        let mentions = self
            .owner
            .get(&format!("/api/v1/persons/{person}/mentions"))
            .await;
        mentions["items"].as_array().unwrap().clone()
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

fn kinds(mentions: &[Value]) -> Vec<&str> {
    let mut kinds: Vec<&str> = mentions
        .iter()
        .map(|mention| mention["kind"].as_str().unwrap())
        .collect();
    kinds.sort_unstable();
    kinds
}

/// The `structuredContent` of an MCP tool call with the API token `token`.
async fn mcp_tool(world: &World, token: &str, name: &str, arguments: Value) -> Value {
    let call = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": {"name": name, "arguments": arguments}});
    let request = support::request(Method::POST, "/mcp")
        .header(header::HOST, "tada.example.org")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .body(Body::from(call.to_string()))
        .unwrap();
    let (response, body) = support::send(&world.owner.router, request).await;
    assert_eq!(response.status(), StatusCode::OK, "{body}");
    body["result"]["structuredContent"].clone()
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

/// Each key of a JSON value, also in nested objects and arrays.
fn keys(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                into.push(key.clone());
                keys(value, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| keys(item, into)),
        _ => {}
    }
}

#[tokio::test]
async fn mentions_find_name_email_phone_case_insensitive() {
    let world = World::start().await;
    let person = world
        .person(json!({"name": "Beat Muster", "email": "beat.muster@example.org", "phone": "079 555 01 02"}))
        .await;
    let changeset = world
        .changeset(
            "Kontakt: BEAT MUSTER, Tel 079 555 01 02. Das Open Day ist im Mai 2030.",
            "im Mai 2030",
        )
        .await;
    let action = world
        .action("Mail an Beat.Muster@Example.org schicken")
        .await;
    world.action("Zelt aufstellen").await;
    let padding = "Lange Notiz zum Gelände und zur Zufahrt. ".repeat(10);
    let long = world
        .action_with(json!({"title": "Rückruf", "description": format!("{padding}Rückruf von beat muster. {padding}")}))
        .await;
    let commitment = world
        .owner
        .post(
            &format!("/api/v1/events/{}/commitments", world.event),
            &json!({"text": "Generator liefern", "promisor": {"kind": "person", "id": person},
                    "owner_user_id": world.owner_id.as_uuid()}),
        )
        .await;

    let mentions = world.mentions(&person).await;
    assert_eq!(
        kinds(&mentions),
        ["action", "action", "promisor", "source-item"],
        "{mentions:?}"
    );
    let source = mentions
        .iter()
        .find(|m| m["kind"] == "source-item")
        .unwrap();
    assert_eq!(
        source["id"],
        world.source_item(&changeset).await.to_string()
    );
    assert_eq!(source["event_id"], world.event.as_str());
    assert_eq!(source["redacted"], false);
    assert_eq!(source["disputed"], false);
    assert!(
        source["excerpt"].as_str().unwrap().contains("BEAT MUSTER"),
        "{source}"
    );
    let mail = mentions
        .iter()
        .find(|m| m["id"] == action.as_str())
        .unwrap();
    assert!(
        mail["excerpt"]
            .as_str()
            .unwrap()
            .contains("Beat.Muster@Example.org")
    );
    let promisor = mentions.iter().find(|m| m["kind"] == "promisor").unwrap();
    assert_eq!(promisor["id"], commitment["id"]);
    assert_eq!(promisor["excerpt"], "Generator liefern");
    // The excerpt of a long text has at most 200 characters around the match.
    let long = mentions.iter().find(|m| m["id"] == long.as_str()).unwrap();
    let excerpt = long["excerpt"].as_str().unwrap();
    assert!(excerpt.chars().count() <= 200, "{excerpt}");
    assert!(excerpt.contains("beat muster"), "{excerpt}");
}

/// A name in the file name of an upload or in the name of a document is a mention of the document version.
#[tokio::test]
async fn mentions_find_the_file_name_and_the_document_name() {
    let world = World::start().await;
    let person = world.person(json!({"name": "Beat Muster"})).await;
    let documents = format!("/api/v1/events/{}/documents", world.event);
    let offer = world
        .owner
        .upload(&documents, "Offerte Beat Muster.pdf", pdf("Angebot"))
        .await;
    let contract = world
        .owner
        .upload(&documents, "Vertrag.pdf", pdf("Vertrag"))
        .await;
    world
        .owner
        .upload(&documents, "Programm.pdf", pdf("Programm"))
        .await;
    let contract_id = contract["id"].as_str().unwrap();
    world
        .test
        .execute(&[&format!(
            "UPDATE document SET name = 'Vertrag mit Beat Muster' WHERE id = '{contract_id}'"
        )])
        .await;
    let version_of = |document: &str| {
        format!("SELECT id FROM document_version WHERE document_id = '{document}'")
    };
    let offer_version = world
        .test
        .scalar::<Uuid>(&version_of(offer["id"].as_str().unwrap()))
        .await;
    let contract_version = world.test.scalar::<Uuid>(&version_of(contract_id)).await;

    let mentions = world.mentions(&person).await;
    assert_eq!(
        kinds(&mentions),
        ["document-version", "document-version"],
        "{mentions:?}"
    );
    for version in [offer_version, contract_version] {
        let mention = mentions
            .iter()
            .find(|m| m["id"] == version.to_string())
            .unwrap();
        assert!(
            mention["excerpt"].as_str().unwrap().contains("Beat Muster"),
            "{mention}"
        );
        assert_eq!(mention["event_id"], world.event.as_str());
    }
}

/// The export and a redaction cannot be undone, so a stolen session older than 15 minutes cannot make them.
/// The mentions stay readable.
#[tokio::test]
async fn export_and_redaction_need_a_recent_sign_in() {
    let world = World::start().await;
    let person = world.person(json!({"name": "Beat Muster"})).await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let source = world.source_item(&changeset).await;
    let cookie = world
        .test
        .sign_in(
            world.owner_id,
            Some(world.organization),
            jiff::Timestamp::now() - jiff::SignedDuration::from_mins(16),
        )
        .await;
    let stale = Client {
        router: router(&world.test, &world.garage),
        cookie,
    };

    let (status, problem) = call(
        &stale,
        Method::GET,
        &format!("/api/v1/persons/{person}/export"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "recent-sign-in-required");
    let body = json!({"targets": [{"kind": "source-item", "id": source}], "reason": "request"});
    let (status, problem) = call(&stale, Method::POST, "/api/v1/redactions", Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "recent-sign-in-required");
    assert_eq!(
        world
            .test
            .scalar::<i64>("SELECT count(*) FROM redaction")
            .await,
        0
    );
    assert_eq!(
        world
            .test
            .scalar::<i64>("SELECT count(*) FROM audit_event WHERE action = 'person.export'")
            .await,
        0
    );
    let (status, _) = call(
        &stale,
        Method::GET,
        &format!("/api/v1/persons/{person}/mentions"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn mentions_escape_like_wildcards() {
    let world = World::start().await;
    let person = world.person(json!({"name": "100%_x"})).await;
    let literal = world.action("Gast 100%_x kommt").await;
    // With `%` and `_` as wildcards, the name would match these texts too.
    world.action("Gast 100 Tische und 1x Bühne").await;
    world.action("Gast 100%ax kommt").await;

    let mentions = world.mentions(&person).await;
    assert_eq!(mentions.len(), 1, "{mentions:?}");
    assert_eq!(mentions[0]["id"], literal.as_str());
}

#[tokio::test]
async fn short_terms_ignored() {
    let world = World::start().await;
    let person = world.person(json!({"name": "Al", "phone": "12"})).await;
    world.action("Alpenfest in der Halle 12").await;

    assert_eq!(world.mentions(&person).await, Vec::<Value>::new());
}

#[tokio::test]
async fn export_contains_account_data_when_linked_and_no_secrets() {
    let (world, filled) = World::filled(4_200_001).await;
    let email: String = world
        .test
        .scalar(&format!(
            "SELECT email FROM email_identity WHERE user_id = '{}'",
            world.owner_id
        ))
        .await;
    let linked = world
        .person(json!({"name": "Owner Testwil", "user_id": world.owner_id.as_uuid()}))
        .await;
    let unlinked = world.person(json!({"name": "Beat Muster"})).await;
    world.action("Anruf bei Owner Testwil").await;

    let request = support::request(Method::GET, &format!("/api/v1/persons/{linked}/export"))
        .header(
            header::COOKIE,
            format!("{SESSION_COOKIE}={}", world.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let response = world.owner.router.clone();
    let response = tower::ServiceExt::oneshot(response, request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"person-PER-001.json\""
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    let export: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(export["format_version"], 1);
    assert_eq!(export["organization"]["id"], world.organization.to_string());
    assert_eq!(export["person"]["id"], linked.as_str());
    assert_eq!(export["person"]["local_id"], "PER-001");
    assert_eq!(export["mentions"].as_array().unwrap().len(), 1, "{export}");
    assert_eq!(export["mentions"][0]["text"], "Anruf bei Owner Testwil");
    let account = &export["account"];
    assert_eq!(account["user_id"], world.owner_id.to_string());
    assert_eq!(account["email"], email.as_str());
    assert_eq!(account["organization_role"], "owner");
    assert_eq!(account["telegram"]["telegram_user_id"], 4_200_001);
    assert!(
        !account["sessions"].as_array().unwrap().is_empty(),
        "{account}"
    );
    assert_eq!(account["api_tokens"][0]["name"], "Claude Code");
    assert!(!account["audit_events"].as_array().unwrap().is_empty());
    assert!(account["event_memberships"].is_array());
    assert!(account["invitations"].is_array());

    // No secret and no hash of a secret.
    let mut names = Vec::new();
    keys(&export, &mut names);
    for name in &names {
        assert!(!name.contains("hash") && !name.contains("secret"), "{name}");
    }
    for secret in &filled.secrets {
        assert!(!text.contains(secret.as_str()), "the export holds a secret");
    }
    let hashes: String = world
        .test
        .scalar(
            "SELECT string_agg(encode(token_hash, 'hex'), ',') FROM (
                 SELECT token_hash FROM session UNION ALL SELECT token_hash FROM api_token) AS hashes",
        )
        .await;
    for hash in hashes.split(',') {
        assert!(!text.contains(hash), "the export holds a token hash");
    }
    assert_eq!(
        world
            .test
            .scalar::<i64>(&format!(
                "SELECT count(*) FROM audit_event WHERE action = 'person.export' AND record_id = '{linked}'"
            ))
            .await,
        1
    );

    // A person without an account has no account data.
    let export = world
        .owner
        .get(&format!("/api/v1/persons/{unlinked}/export"))
        .await;
    assert_eq!(export["account"], Value::Null, "{export}");
}

#[tokio::test]
async fn only_owner_reads_mentions_exports_redacts_disputes() {
    let (world, filled) = World::filled(4_200_002).await;
    let person = world.person(json!({"name": "Beat Muster"})).await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;
    let mentions = format!("/api/v1/persons/{person}/mentions");
    let export = format!("/api/v1/persons/{person}/export");
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
            (Method::GET, mentions.as_str(), None),
            (Method::GET, export.as_str(), None),
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
        (Method::GET, mentions.as_str(), None),
        (Method::GET, export.as_str(), None),
        (Method::POST, "/api/v1/redactions", Some(&redaction)),
        (Method::POST, "/api/v1/dispute-notes", Some(&note)),
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
        for word in ["mention", "export", "redact", "dispute"] {
            assert!(!name.contains(word), "{name}");
        }
    }

    assert_eq!(
        world
            .test
            .scalar::<i64>(
                "SELECT (SELECT count(*) FROM redaction) + (SELECT count(*) FROM dispute_note)
                 + (SELECT count(*) FROM audit_event WHERE action = 'person.export')"
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

    // The note marks the mention as disputed. A text over 1000 characters is refused.
    let person = world.person(json!({"name": "Open Day"})).await;
    let mentions = world.mentions(&person).await;
    let source = mentions
        .iter()
        .find(|m| m["kind"] == "source-item")
        .unwrap();
    assert_eq!(source["disputed"], true, "{mentions:?}");
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
    let person = world.person(json!({"name": "Beat Muster"})).await;
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
    assert!(world.mentions(&person).await.is_empty());

    // A redacted row that refers to the person by ID shows as redacted, without an excerpt.
    let commitment = world
        .owner
        .post(
            &format!("/api/v1/events/{}/commitments", world.event),
            &json!({"text": "Generator liefern", "promisor": {"kind": "person", "id": person},
                    "owner_user_id": world.owner_id.as_uuid()}),
        )
        .await;
    let id = commitment["id"].as_str().unwrap();
    world
        .test
        .redact(&[&format!(
            "UPDATE commitment SET text = NULL, condition = NULL, firm_reason = NULL, redacted_at = now()
             WHERE id = '{id}'"
        )])
        .await;
    let mentions = world.mentions(&person).await;
    assert_eq!(
        mentions,
        [
            json!({"kind": "promisor", "id": id, "event_id": world.event, "redacted": true, "disputed": false})
        ]
    );
}

#[tokio::test]
async fn dispute_notes_reach_mcp_readers_and_the_current_fact_of_a_proposal() {
    let world = World::start().await;
    let changeset = world
        .changeset("Das Open Day ist im Mai 2030.", "im Mai 2030")
        .await;
    world.owner.apply(&changeset).await;
    let item = world.source_item(&changeset).await;
    let fact_version = world
        .test
        .scalar::<Uuid>("SELECT id FROM fact_version")
        .await;
    for (kind, id, text) in [
        ("source-item", item, "Die Quelle ist falsch."),
        ("fact-version", fact_version, "Der Monat ist falsch."),
    ] {
        world
            .owner
            .post(
                "/api/v1/dispute-notes",
                &json!({"target_kind": kind, "target_id": id, "text": text}),
            )
            .await;
    }

    // An AI client of a member reads the profile through MCP and sees both notes.
    let expires = jiff::Timestamp::now() + jiff::SignedDuration::from_hours(24);
    let token = world
        .owner
        .post(
            "/api/v1/tokens",
            &json!({"name": "Claude Code", "scope": "read", "expires_at": expires.to_string(),
                    "notice_version_confirmed": 1}),
        )
        .await;
    let token = token["secret"].as_str().unwrap();
    let profile = mcp_tool(
        &world,
        token,
        "get_event_profile",
        json!({"event_key": "OPEN30"}),
    )
    .await;
    let fact = &profile["accepted"][0];
    assert_eq!(
        fact["dispute_notes"][0]["text"], "Der Monat ist falsch.",
        "{profile}"
    );
    assert_eq!(
        fact["evidence"][0]["dispute_notes"][0]["text"],
        "Die Quelle ist falsch."
    );

    // A second proposal for the fact shows the notes of the current fact version to its reviewer.
    let second = world
        .changeset("Das Open Day ist doch im Mai 2030.", "im Mai 2030")
        .await;
    let review = world
        .owner
        .get(&format!(
            "/api/v1/changesets/{}",
            second["id"].as_str().unwrap()
        ))
        .await;
    let current = &review["proposals"][0]["current"];
    assert_eq!(
        current["dispute_notes"][0]["text"], "Der Monat ist falsch.",
        "{review}"
    );
}

#[tokio::test]
async fn a_redaction_of_a_source_item_redacts_its_dispute_notes() {
    let world = World::start().await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;
    world
        .owner
        .post(
            "/api/v1/dispute-notes",
            &json!({"target_kind": "source-item", "target_id": item, "text": "Beat Muster bestreitet es."}),
        )
        .await;
    world
        .owner
        .post(
            "/api/v1/redactions",
            &json!({"targets": [{"kind": "source-item", "id": item}], "reason": "request"}),
        )
        .await;

    assert_eq!(
        world
            .test
            .scalar::<i64>(
                "SELECT count(*) FROM dispute_note WHERE text IS NULL AND redacted_at IS NOT NULL"
            )
            .await,
        1
    );
    let review = world
        .owner
        .get(&format!(
            "/api/v1/changesets/{}",
            changeset["id"].as_str().unwrap()
        ))
        .await;
    assert!(!review.to_string().contains("bestreitet"), "{review}");
}

/// A decision that the person took is a reference by ID; a decision whose wording names the person is a mention
/// (ADR 0081). The export lists both.
#[tokio::test]
async fn person_export_lists_the_decisions_of_a_person() {
    let world = World::start().await;
    let person = world.person(json!({"name": "Beat Muster"})).await;
    let other = world.person(json!({"name": "Gerda Gemeinde"})).await;
    let decisions = format!("/api/v1/events/{}/decisions", world.event);
    let decide = |party: &str, wording: &str| {
        json!({"wording": wording, "decided_on": "2030-03-01",
               "decided_by": [{"kind": "person", "id": party}],
               "owner_user_id": world.owner_id.as_uuid()})
    };
    let taken = world
        .owner
        .post(
            &decisions,
            &decide(&person, "Das Fest findet statt.\nBei Regen im Zelt."),
        )
        .await;
    let named = world
        .owner
        .post(
            &decisions,
            &decide(&other, "Beat Muster leitet den Aufbau."),
        )
        .await;
    world
        .owner
        .post(&decisions, &decide(&other, "Der Zaun bleibt."))
        .await;

    let mentions = world.mentions(&person).await;
    assert_eq!(
        kinds(&mentions),
        ["decision", "decision-party"],
        "{mentions:?}"
    );
    let party = mentions
        .iter()
        .find(|m| m["kind"] == "decision-party")
        .unwrap();
    assert_eq!(party["id"], taken["id"]);
    assert_eq!(
        party["excerpt"],
        "Das Fest findet statt.\nBei Regen im Zelt."
    );
    let mention = mentions.iter().find(|m| m["kind"] == "decision").unwrap();
    assert_eq!(mention["id"], named["id"]);

    let request = support::request(Method::GET, &format!("/api/v1/persons/{person}/export"))
        .header(
            header::COOKIE,
            format!("{SESSION_COOKIE}={}", world.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(world.owner.router.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let export: Value = serde_json::from_slice(&body).unwrap();
    let references = export["references"].as_array().unwrap();
    assert_eq!(references.len(), 1, "{export}");
    assert_eq!(references[0]["kind"], "decision-party");
    assert_eq!(references[0]["id"], taken["id"]);
    let mentions = export["mentions"].as_array().unwrap();
    assert_eq!(mentions.len(), 1, "{export}");
    assert_eq!(mentions[0]["kind"], "decision");
    assert_eq!(mentions[0]["text"], "Beat Muster leitet den Aufbau.");
}

/// A risk whose title, description or mitigation names a person is a mention (ADR 0081). The export lists it.
#[tokio::test]
async fn person_export_lists_the_risks_that_name_a_person() {
    let world = World::start().await;
    let person = world.person(json!({"name": "Beat Muster"})).await;
    let risks = format!("/api/v1/events/{}/risks", world.event);
    let risk = |title: &str, description: Option<&str>, mitigation: Option<&str>| {
        json!({"title": title, "description": description, "mitigation": mitigation,
               "likelihood": "medium", "impact": "high",
               "owner_user_id": world.owner_id.as_uuid()})
    };
    let in_title = world
        .owner
        .post(&risks, &risk("Beat Muster fällt aus", None, None))
        .await;
    let in_description = world
        .owner
        .post(
            &risks,
            &risk("Der Aufbau", Some("Ohne beat muster fehlt der Plan."), None),
        )
        .await;
    let in_mitigation = world
        .owner
        .post(
            &risks,
            &risk("Der Ort", None, Some("Frage bei Beat Muster nach.")),
        )
        .await;
    world
        .owner
        .post(
            &risks,
            &risk("Starkregen", Some("Das Zelt."), Some("Kies.")),
        )
        .await;

    let mentions = world.mentions(&person).await;
    assert_eq!(kinds(&mentions), ["risk", "risk", "risk"], "{mentions:?}");
    let ids: Vec<&Value> = mentions.iter().map(|m| &m["id"]).collect();
    for found in [&in_title, &in_description, &in_mitigation] {
        assert!(ids.contains(&&found["id"]), "{found} not in {ids:?}");
    }
    // A risk is a text that names the person, not a reference by ID.
    let request = support::request(Method::GET, &format!("/api/v1/persons/{person}/export"))
        .header(
            header::COOKIE,
            format!("{SESSION_COOKIE}={}", world.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(world.owner.router.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let export: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        export["references"].as_array().unwrap().len(),
        0,
        "{export}"
    );
    assert_eq!(export["mentions"].as_array().unwrap().len(), 3, "{export}");
}
