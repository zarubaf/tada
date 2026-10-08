//! The structured export of one organization (ADR 0059): isolation, no secrets, blob hashes and
//! the rebuild of the data from the export.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::Timestamp;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::SESSION_COOKIE;
use support::files::pdf;
use tada::export::{ExportCommand, execute};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_api::ApiState;
use tada_app::caller::{OrganizationRole, ServiceCaller, TelegramGateway};
use tada_app::domain::identity::{DisplayName, Email, OrganizationSlug};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::session::SessionAuthenticator;
use tada_app::telegram::{TelegramName, TelegramUserId, claim_link_code};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const SOURCE: &str = "Das Open Day findet im Mai 2030 auf dem Flugfeld statt.";

/// The API of one database on a shared Garage.
fn router(test: &TestDatabase, garage: &TestGarage) -> Router {
    let database = Arc::new(test.database.clone());
    let clock = Arc::new(SystemClock);
    let authenticator = Arc::new(SessionAuthenticator::new(
        database.clone(),
        database,
        clock.clone(),
    ));
    let state = ApiState {
        blobs: Arc::new(garage.storage.clone()),
        upload_max_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
        ..support::api_state(test, authenticator, clock)
    };
    tada_api::router(state, None)
}

/// A member of one organization who calls the API.
struct Client {
    router: Router,
    cookie: String,
}

impl Client {
    async fn call(&self, request: axum::http::request::Builder, body: Body) -> (StatusCode, Value) {
        let request = request
            .header(header::COOKIE, format!("{SESSION_COOKIE}={}", self.cookie))
            .body(body)
            .unwrap();
        let (response, value) = support::send(&self.router, request).await;
        (response.status(), value)
    }

    async fn get(&self, path: &str) -> Value {
        let (status, value) = self
            .call(support::request(Method::GET, path), Body::empty())
            .await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    async fn post(&self, path: &str, body: &Value) -> Value {
        let request =
            support::request(Method::POST, path).header(header::CONTENT_TYPE, "application/json");
        let (status, value) = self.call(request, Body::from(body.to_string())).await;
        assert!(status.is_success(), "{path}: {status} {value}");
        value
    }

    async fn upload(&self, path: &str, name: &str, content: Vec<u8>) -> Value {
        let request = support::request(Method::POST, path)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("x-file-name", name);
        let (status, value) = self.call(request, Body::from(content)).await;
        assert_eq!(status, StatusCode::CREATED, "{value}");
        value
    }

    async fn create_event(&self, key: &str, name: &str) -> String {
        let event = self
            .post("/api/v1/events", &json!({"key": key, "name": name}))
            .await;
        event["id"].as_str().unwrap().to_owned()
    }

    /// Proposes one operation with the passage `quote` of `SOURCE` as evidence. Returns the changeset.
    async fn propose(&self, event: &str, operation: Value, quote: &str) -> Value {
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
        self.post(&format!("/api/v1/events/{event}/changesets"), &body)
            .await
    }

    async fn apply(&self, changeset: &Value) {
        let path = format!(
            "/api/v1/changesets/{}/apply",
            changeset["id"].as_str().unwrap()
        );
        self.post(&path, &json!({"selected": changeset["proposal_ids"]}))
            .await;
    }

    /// A proposal that sets the date window of the event to May 2030.
    async fn date_operation(&self, event: &str) -> Value {
        let fields = self.get(&format!("/api/v1/events/{event}/fields")).await;
        let field = fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == "date_window")
            .unwrap()["id"]
            .clone();
        json!({
            "kind": "set-fact", "event_id": event, "field_id": field, "expected_version": null,
            "state": {"state": "accepted", "value": {"type": "date-window",
                "start": "2030-05-01", "end": "2030-05-31", "granularity": "month"}},
        })
    }
}

/// An organization with an owner who calls the API.
async fn organization(
    test: &TestDatabase,
    garage: &TestGarage,
    slug: &str,
) -> (OrganizationId, UserId, Client) {
    let (organization, user, cookie) = test.member(slug, OrganizationRole::Owner).await;
    let client = Client {
        router: router(test, garage),
        cookie,
    };
    (organization, user, client)
}

/// The data of organization A that the test compares after the import.
struct Filled {
    event: String,
    upload: String,
    draft: String,
    secrets: Vec<String>,
}

/// Fills organization A: two events, facts, an applied and an open changeset, an upload with two
/// versions, an approved draft, a Telegram link, an API token and an invitation.
async fn fill_a(test: &TestDatabase, client: &Client, organization: OrganizationId) -> Filled {
    let event = client.create_event("OPEN30", "Open Day Testwil").await;
    let second = client.create_event("FLY30", "Fly-in Testwil").await;

    let operation = client.date_operation(&event).await;
    let applied = client.propose(&event, operation, "im Mai 2030").await;
    client.apply(&applied).await;
    let operation = client.date_operation(&second).await;
    client.propose(&second, operation, "im Mai 2030").await;
    let profile = client.get(&format!("/api/v1/events/{event}/profile")).await;
    let fact = profile["facts"][0]["id"].as_str().unwrap().to_owned();

    let documents = format!("/api/v1/events/{event}/documents");
    let upload = client
        .upload(&documents, "Programm.pdf", pdf("Programm Testwil eins"))
        .await;
    let upload = upload["id"].as_str().unwrap().to_owned();
    client
        .upload(
            &format!("/api/v1/documents/{upload}/versions"),
            "Programm.pdf",
            pdf("Programm Testwil zwei"),
        )
        .await;

    let draft = Uuid::now_v7();
    let operation = json!({
        "kind": "create-document-draft", "event_id": event,
        "document": {"new": {"id": draft, "name": "Konzept Testwil"}},
        "markdown": format!("Das Open Day ist am [](tada:fact/{fact}?v=1).\n"),
    });
    let changeset = client.propose(&event, operation, "Das Open Day").await;
    client.apply(&changeset).await;
    let document = client.get(&format!("/api/v1/documents/{draft}")).await;
    let version = document["newest_version"]["id"].as_str().unwrap();
    client
        .post(
            &format!("/api/v1/document-versions/{version}/approve"),
            &json!({"expected_version": 1}),
        )
        .await;

    let code = client.post("/api/v1/telegram/link-codes", &json!({})).await;
    let code = code["code"].as_str().unwrap().to_owned();
    assert!(
        claim_link_code(
            &ServiceCaller::<TelegramGateway>::new(),
            &code,
            TelegramUserId(424_242),
            &TelegramName("Testperson Testwil".to_owned()),
            &test.database,
            &SystemClock,
        )
        .await
        .unwrap()
    );
    let requests = client.get("/api/v1/telegram/link-requests").await;
    let request = requests["items"][0]["id"].as_str().unwrap();
    client
        .post(
            &format!("/api/v1/telegram/link-requests/{request}/confirm"),
            &json!({}),
        )
        .await;
    let open_code = client.post("/api/v1/telegram/link-codes", &json!({})).await;

    let expires = Timestamp::now() + jiff::SignedDuration::from_hours(24);
    let token = client
        .post(
            "/api/v1/tokens",
            &json!({"name": "Claude Code", "scope": "read", "expires_at": expires.to_string(),
                    "notice_version_confirmed": 1}),
        )
        .await;

    test.queue_invitation(
        organization,
        &Email::parse("invited@example.org").unwrap(),
        &DisplayName::parse("Invited Testwil").unwrap(),
        OrganizationRole::Member,
    )
    .await;

    Filled {
        event,
        upload,
        draft: draft.to_string(),
        secrets: vec![
            client.cookie.clone(),
            code,
            open_code["code"].as_str().unwrap().to_owned(),
            token["secret"].as_str().unwrap().to_owned(),
        ],
    }
}

/// Adds a row to each secret table that the API does not fill in this test.
async fn add_secret_rows(test: &TestDatabase, organization: OrganizationId, user: UserId) {
    let inserts = [
        format!(
            "WITH i AS (INSERT INTO magic_link (token_hash, user_id, expires_at, created_at)
             VALUES (sha256('magic'), '{user}', now(), now()) RETURNING 1) SELECT count(*) FROM i"
        ),
        "WITH i AS (INSERT INTO rate_limit_counter (key, window_start, count)
         VALUES (sha256('counter'), now(), 1) RETURNING 1) SELECT count(*) FROM i"
            .to_owned(),
        format!(
            "WITH i AS (INSERT INTO invitation_token (token_hash, organization_id, invitation_id, expires_at)
             SELECT sha256('invitation'), organization_id, id, now() FROM invitation
             WHERE organization_id = '{organization}' RETURNING 1) SELECT count(*) FROM i"
        ),
    ];
    for insert in inserts {
        assert_eq!(test.scalar::<i64>(&insert).await, 1, "{insert}");
    }
}

/// The hex of each hash in the secret tables of the source database.
async fn secret_hashes(test: &TestDatabase) -> Vec<String> {
    let sql = "SELECT coalesce(string_agg(encode(hash, 'hex'), ','), '') FROM (
                   SELECT token_hash AS hash FROM session
                   UNION ALL SELECT token_hash FROM magic_link
                   UNION ALL SELECT token_hash FROM invitation_token
                   UNION ALL SELECT code_hash FROM telegram_link_code
                   UNION ALL SELECT key FROM rate_limit_counter
                   UNION ALL SELECT token_hash FROM api_token
               ) AS hashes";
    let hashes: String = test.scalar(sql).await;
    let hashes: Vec<String> = hashes.split(',').map(str::to_owned).collect();
    assert!(hashes.len() >= 8, "{hashes:?}");
    hashes
}

/// All files of the export by their path relative to the export.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                files.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    files
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn export(
    test: &TestDatabase,
    garage: &TestGarage,
    slug: &str,
    output: &Path,
) -> tada_app::export::ExportSummary {
    execute(
        &test.database,
        &garage.storage,
        &SystemClock,
        ExportCommand {
            organization_slug: OrganizationSlug::parse(slug).unwrap(),
            output: output.to_owned(),
        },
    )
    .await
    .unwrap()
}

/// The results of queries of the facts, the document versions, the draft manifests and the approvals of the organization.
async fn compared_rows(test: &TestDatabase, organization: OrganizationId) -> Vec<String> {
    let queries = [
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id)::text FROM fact_version r",
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY r.id)::text FROM document_version r",
        "SELECT jsonb_agg(to_jsonb(r) ORDER BY r.document_version_id, r.fact_id)::text FROM document_manifest_fact r",
        "SELECT jsonb_agg(jsonb_build_array(id, status, approved_by, approved_at) ORDER BY id)::text
         FROM document_version r",
    ];
    let mut rows = Vec::new();
    for query in queries {
        let query = format!("{query} WHERE r.organization_id = '{organization}'");
        rows.push(test.scalar::<String>(&query).await);
    }
    rows
}

/// Acceptance (10) and (1): the export of A holds no row of B and no secret, its blobs have their
/// hashes, and an empty database rebuilt from it holds the same data.
#[tokio::test]
async fn an_export_holds_one_organization_without_secrets_and_rebuilds_its_data() {
    let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
    let (a, owner_a, client_a) = organization(&test, &garage, "testwil").await;
    let filled = fill_a(&test, &client_a, a).await;
    add_secret_rows(&test, a, owner_a).await;
    let (b, owner_b, client_b) = organization(&test, &garage, "musterhausen").await;
    let event_b = client_b.create_event("FLY31", "Fly-in Musterhausen").await;
    client_b
        .upload(
            &format!("/api/v1/events/{event_b}/documents"),
            "Hangar.pdf",
            pdf("Hangar Musterhausen"),
        )
        .await;
    let operation = client_b.date_operation(&event_b).await;
    let changeset = client_b.propose(&event_b, operation, "im Mai 2030").await;
    client_b.apply(&changeset).await;

    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("export");
    let summary = export(&test, &garage, "testwil", &output).await;
    assert_eq!(summary.organization_id, a);
    assert_eq!(summary.blobs, 2, "the two versions of the upload");
    let exported = files(&output);

    // Isolation: no row and no text of B.
    let texts_of_b = [
        b.to_string(),
        owner_b.to_string(),
        owner_b.as_uuid().simple().to_string(),
        event_b.clone(),
        "usterhausen".to_owned(),
    ];
    for (path, content) in &exported {
        for text in &texts_of_b {
            assert!(!contains(content, text), "{path} holds {text} of B");
        }
    }

    // No secret: no secret table, no hash column, no token and no hash of the fixture.
    for table in [
        "session",
        "magic_link",
        "invitation_token",
        "telegram_link_code",
        "rate_limit_counter",
    ] {
        assert!(!exported.contains_key(&format!("tables/{table}.jsonl")));
        assert!(!exported.contains_key(&format!("tables/{table}.csv")));
    }
    let mut secrets = filled.secrets.clone();
    secrets.extend(secret_hashes(&test).await);
    secrets.extend(["token_hash".to_owned(), "code_hash".to_owned()]);
    for (path, content) in &exported {
        for secret in &secrets {
            assert!(!contains(content, secret), "{path} holds a secret");
        }
    }

    // The manifest names each file with its hash, and each blob has the hash of its name.
    let manifest: Value = serde_json::from_slice(&exported["manifest.json"]).unwrap();
    assert_eq!(manifest["format_version"], 1);
    assert_eq!(manifest["organization_id"], a.to_string());
    let listed = manifest["files"].as_array().unwrap();
    assert_eq!(
        listed.len(),
        exported.len() - 1,
        "each file except the manifest"
    );
    for file in listed {
        let content = &exported[file["path"].as_str().unwrap()];
        assert_eq!(file["sha256"], hex(&Sha256::digest(content)));
        assert_eq!(file["size_bytes"], content.len());
    }
    let blobs: Vec<_> = exported
        .iter()
        .filter(|(path, _)| path.starts_with("blobs/"))
        .collect();
    assert_eq!(blobs.len(), 2);
    for (path, content) in blobs {
        assert_eq!(*path, format!("blobs/{}", hex(&Sha256::digest(content))));
    }
    let row_count = |name: &str| {
        manifest["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == name)
            .unwrap()["row_count"]
            .as_u64()
            .unwrap()
    };
    for (name, rows) in [
        ("organization", 1),
        ("event", 2),
        ("changeset", 3),
        ("document", 2),
        ("document_version", 3),
        ("document_manifest_fact", 1),
        ("telegram_identity", 1),
        ("api_token", 1),
        ("invitation", 1),
    ] {
        assert_eq!(row_count(name), rows, "{name}");
    }
    for name in [
        "fact_version",
        "evidence_link",
        "review_result",
        "audit_event",
    ] {
        assert!(row_count(name) > 0, "{name}");
    }
    for table in manifest["tables"].as_array().unwrap() {
        let name = table["name"].as_str().unwrap();
        let lines = exported[&format!("tables/{name}.jsonl")]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        assert_eq!(table["row_count"], lines, "{name}");
        let columns: Vec<&str> = table["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|column| column.as_str().unwrap())
            .collect();
        let header = format!("{}\r\n", columns.join(","));
        assert!(
            exported[&format!("tables/{name}.csv")].starts_with(header.as_bytes()),
            "{name}"
        );
    }

    // The rebuild: an empty database gets the same data.
    let fresh = TestDatabase::start().await;
    fresh.import_export(&output).await;
    assert_eq!(
        compared_rows(&fresh, a).await,
        compared_rows(&test, a).await
    );
    let again = directory.path().join("again");
    export(&fresh, &garage, "testwil", &again).await;
    let again = files(&again);
    for (path, content) in &exported {
        if path != "manifest.json" {
            assert_eq!(again.get(path), Some(content), "{path}");
        }
    }
    let token_hashes = "SELECT string_agg(encode(token_hash, 'hex'), ',') FROM api_token";
    assert_ne!(
        fresh.scalar::<String>(token_hashes).await,
        test.scalar::<String>(token_hashes).await,
        "an exported token never works again"
    );

    let rebuilt = Client {
        router: router(&fresh, &garage),
        cookie: fresh.sign_in(owner_a, Some(a), Timestamp::now()).await,
    };
    for path in [
        format!("/api/v1/events/{}/profile", filled.event),
        format!("/api/v1/documents/{}/versions", filled.upload),
        format!("/api/v1/documents/{}/versions", filled.draft),
        format!("/api/v1/documents/{}", filled.draft),
    ] {
        assert_eq!(
            rebuilt.get(&path).await,
            client_a.get(&path).await,
            "{path}"
        );
    }
}

#[tokio::test]
async fn an_export_refuses_an_unknown_slug_and_a_directory_that_is_not_empty() {
    let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
    test.create_organization("testwil").await;
    let directory = tempfile::tempdir().unwrap();
    let command = |slug: &str, output: &Path| ExportCommand {
        organization_slug: OrganizationSlug::parse(slug).unwrap(),
        output: output.to_owned(),
    };
    let error = execute(
        &test.database,
        &garage.storage,
        &SystemClock,
        command("musterhausen", &directory.path().join("unknown")),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("no organization has this slug"),
        "{error:#}"
    );

    std::fs::write(directory.path().join("other.txt"), "other").unwrap();
    let error = execute(
        &test.database,
        &garage.storage,
        &SystemClock,
        command("testwil", directory.path()),
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("not empty"), "{error:#}");
    assert!(!directory.path().join("manifest.json").exists());
}
