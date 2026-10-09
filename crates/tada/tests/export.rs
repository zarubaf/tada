//! The structured export of one organization (ADR 0059): isolation, no secrets, blob hashes and
//! the rebuild of the data from the export.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::path::Path;

use jiff::Timestamp;
use serde_json::Value;
use sha2::{Digest, Sha256};
use support::export::{contains, export, files};
use support::files::pdf;
use support::organization::{Client, fill, organization, router};
use tada::export::{ExportCommand, execute};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_app::domain::identity::OrganizationSlug;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_store_pg::testing::TestDatabase;

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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    let filled = fill(&test, &client_a, a, 424_242).await;
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
