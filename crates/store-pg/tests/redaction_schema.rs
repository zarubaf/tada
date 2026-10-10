//! The tombstone rule of a legal redaction (ADR 0072): the triggers of the immutable tables allow one shape of UPDATE.
//! It runs with `SET LOCAL tada.redaction = 'on'`, sets `redacted_at` on a row that is not redacted, and changes
//! the content columns to NULL only. Each other UPDATE, each DELETE and each TRUNCATE fails.

#![allow(clippy::unwrap_used)]

use sqlx::AssertSqlSafe;
use tada_store_pg::testing::{TestDatabase, sqlstate};

const RESTRICT_VIOLATION: &str = "23001";

/// The test database with the invented rows of the fixtures of 0015 and 0025.
async fn database() -> TestDatabase {
    let test = TestDatabase::start().await;
    sqlx::raw_sql(include_str!("fixtures/0015.sql"))
        .execute(test.pool())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("fixtures/0025.sql"))
        .execute(test.pool())
        .await
        .unwrap();
    test
}

/// Runs `statement` in its own transaction, with the redaction setting on if `redaction` is true.
async fn run(test: &TestDatabase, redaction: bool, statement: &str) -> Result<(), sqlx::Error> {
    let mut tx = test.pool().begin().await?;
    if redaction {
        sqlx::query("SET LOCAL tada.redaction = 'on'")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(AssertSqlSafe(statement.to_owned()))
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

async fn refused(test: &TestDatabase, redaction: bool, statement: &str) {
    let error = run(test, redaction, statement).await.expect_err(statement);
    assert_eq!(sqlstate(&error), RESTRICT_VIOLATION, "{statement}");
}

/// One immutable row and the SET clauses of the UPDATEs that the test tries on it.
struct Case {
    table: &'static str,
    id: &'static str,
    /// The tombstone: each content column to NULL (or to its allowed form) and `redacted_at` set.
    tombstone: &'static str,
    /// Other UPDATEs that must fail with the setting on.
    refused: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        table: "source_version",
        id: "0190a000-0000-7000-8000-000000000012",
        tombstone: "text = NULL, url = NULL, sha256 = NULL, redacted_at = now(), \
                    author_actor = jsonb_build_object('kind', author_actor -> 'kind', 'id', author_actor -> 'id')",
        refused: &[
            "text = NULL, sha256 = NULL, captured_at = now(), redacted_at = now()",
            "text = NULL, sha256 = NULL, channel = 'cli', redacted_at = now()",
            "text = 'x', sha256 = NULL, redacted_at = now()",
            "text = NULL, sha256 = NULL, author_actor = author_actor || '{\"channel\": \"cli\"}', redacted_at = now()",
            "text = NULL, sha256 = NULL, author_actor = '{\"kind\": \"member\", \"id\": \"0190a000-0000-7000-8000-000000000001\"}', redacted_at = now()",
            "text = NULL, sha256 = NULL",
            "text = NULL, url = NULL, redacted_at = now()",
            "text = NULL, url = NULL, sha256 = NULL, redacted_at = now()",
        ],
    },
    Case {
        table: "fact_version",
        id: "0190a000-0000-7000-8000-000000000052",
        tombstone: "value = NULL, redacted_at = now()",
        refused: &[
            "value = NULL, approximate = true, redacted_at = now()",
            "value = '{\"type\": \"text\", \"text\": \"x\"}', redacted_at = now()",
            "value = NULL",
        ],
    },
    Case {
        table: "proposal",
        id: "0190a000-0000-7000-8000-000000000044",
        tombstone: "operation = jsonb_build_object('kind', operation -> 'kind'), reason = NULL, manifest = NULL, \
                    lint_warnings = NULL, redacted_at = now()",
        refused: &[
            "operation = '{\"kind\": \"set_fact\"}', reason = NULL, manifest = NULL, lint_warnings = NULL, redacted_at = now()",
            "reason = NULL, manifest = NULL, lint_warnings = NULL, redacted_at = now()",
            "operation = operation - 'markdown', reason = NULL, manifest = NULL, lint_warnings = NULL, redacted_at = now()",
            "reason = NULL, created_at = now(), redacted_at = now()",
            "reason = NULL, target_id = gen_random_uuid(), redacted_at = now()",
            "reason = 'x', redacted_at = now()",
            "reason = NULL",
        ],
    },
    Case {
        table: "document_version",
        id: "0190a000-0000-7000-8000-000000000095",
        tombstone: "markdown = NULL, sha256 = NULL, redacted_at = now()",
        refused: &[
            "markdown = NULL, sha256 = NULL, status = 'archived', redacted_at = now()",
            "markdown = NULL, sha256 = NULL, approved_at = now(), redacted_at = now()",
            "markdown = NULL, redacted_at = now()",
            "markdown = 'x', redacted_at = now()",
            "markdown = NULL",
        ],
    },
    Case {
        table: "document_version",
        id: "0190a000-0000-7000-8000-000000000092",
        tombstone: "file_name = NULL, sha256 = NULL, redacted_at = now()",
        refused: &[
            "file_name = NULL, sha256 = NULL, blob_key = NULL, redacted_at = now()",
            "file_name = NULL, sha256 = '\\x00', redacted_at = now()",
            "file_name = NULL, redacted_at = now()",
            "file_name = 'x.pdf', redacted_at = now()",
        ],
    },
    Case {
        table: "record_evidence",
        id: "0190a000-0000-7000-8000-000000000131",
        tombstone: "quote = NULL, redacted_at = now()",
        refused: &[
            "quote = NULL, page = 2, redacted_at = now()",
            "quote = 'x', redacted_at = now()",
            "quote = NULL",
        ],
    },
    Case {
        table: "evidence_link",
        id: "0190a000-0000-7000-8000-000000000061",
        tombstone: "quote = NULL, redacted_at = now()",
        refused: &["quote = NULL, start_offset = 0, redacted_at = now()"],
    },
    Case {
        table: "proposal_evidence",
        id: "0190a000-0000-7000-8000-000000000071",
        tombstone: "quote = NULL, redacted_at = now()",
        refused: &["quote = NULL, end_offset = 60, redacted_at = now()"],
    },
    Case {
        table: "source_item",
        id: "0190a000-0000-7000-8000-000000000011",
        tombstone: "redacted_at = now()",
        refused: &["kind = 'upload', redacted_at = now()"],
    },
];

#[tokio::test]
async fn redaction_trigger_allows_only_tombstone_update() {
    let test = database().await;
    for case in CASES {
        let update =
            |set: &str| format!("UPDATE {} SET {set} WHERE id = '{}'", case.table, case.id);
        let delete = format!("DELETE FROM {} WHERE id = '{}'", case.table, case.id);

        // Without the setting, nothing changes the content, not even the tombstone.
        refused(&test, false, &update(case.tombstone)).await;
        refused(&test, false, &delete).await;

        // With the setting, only the tombstone passes.
        // A partial tombstone leaves content in place, so it fails too.
        if case.tombstone != "redacted_at = now()" {
            refused(&test, true, &update("redacted_at = now()")).await;
        }
        for set in case.refused {
            refused(&test, true, &update(set)).await;
        }
        refused(&test, true, &delete).await;
        run(&test, true, &update(case.tombstone))
            .await
            .unwrap_or_else(|error| panic!("{}: {error}", update(case.tombstone)));

        // A redacted row never changes again.
        refused(&test, true, &update("redacted_at = now()")).await;
        refused(&test, true, &update(case.tombstone)).await;
        refused(&test, true, &delete).await;
    }

    // The operation of a proposal keeps its kind: it never becomes NULL.
    refused(
        &test,
        true,
        "UPDATE proposal SET operation = NULL, reason = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000043'",
    )
    .await;

    for table in [
        "source_item",
        "source_version",
        "fact_version",
        "evidence_link",
        "proposal",
        "proposal_evidence",
        "record_evidence",
        "document_version",
    ] {
        refused(&test, true, &format!("TRUNCATE {table} CASCADE")).await;
    }

    // The tombstone keeps the actor kind and ID of the author, and the search finds no text.
    let (author, search_empty): (serde_json::Value, bool) = sqlx::query_as(
        "SELECT author_actor, search = ''::tsvector FROM source_version
         WHERE id = '0190a000-0000-7000-8000-000000000012'",
    )
    .fetch_one(test.pool())
    .await
    .unwrap();
    assert_eq!(
        author,
        serde_json::json!({"kind": "member", "id": "0190a000-0000-7000-8000-000000000002"})
    );
    assert!(search_empty);
    let operation: serde_json::Value = sqlx::query_scalar(
        "SELECT operation FROM proposal WHERE id = '0190a000-0000-7000-8000-000000000044'",
    )
    .fetch_one(test.pool())
    .await
    .unwrap();
    assert_eq!(
        operation,
        serde_json::json!({"kind": "create_document_draft"})
    );
}

/// The approval and the status of a draft change as before, and the content of a version stays guarded.
#[tokio::test]
async fn a_draft_status_changes_without_the_redaction_setting() {
    let test = database().await;
    run(
        &test,
        false,
        "UPDATE document_version SET status = 'archived' WHERE id = '0190a000-0000-7000-8000-000000000096'",
    )
    .await
    .unwrap();
    refused(
        &test,
        false,
        "UPDATE document_version SET redacted_at = now() WHERE id = '0190a000-0000-7000-8000-000000000096'",
    )
    .await;
    refused(
        &test,
        true,
        "UPDATE document_version SET status = 'superseded' WHERE id = '0190a000-0000-7000-8000-000000000096'",
    )
    .await;
}

/// A content column can be NULL only in a redacted row, also in the tables without a trigger.
#[tokio::test]
async fn content_is_null_only_in_a_redacted_row() {
    let test = database().await;
    let columns = [
        ("person", "0190a000-0000-7000-8000-000000000111", "name"),
        (
            "institution",
            "0190a000-0000-7000-8000-000000000112",
            "name",
        ),
        (
            "open_question",
            "0190a000-0000-7000-8000-000000000101",
            "text",
        ),
        ("workstream", "0190a000-0000-7000-8000-000000000121", "name"),
        ("action", "0190a000-0000-7000-8000-000000000122", "title"),
        ("commitment", "0190a000-0000-7000-8000-000000000123", "text"),
        (
            "commitment",
            "0190a000-0000-7000-8000-000000000123",
            "condition",
        ),
        ("document", "0190a000-0000-7000-8000-000000000094", "name"),
    ];
    for (table, id, column) in columns {
        let error = run(
            &test,
            false,
            &format!("UPDATE {table} SET {column} = NULL WHERE id = '{id}'"),
        )
        .await
        .unwrap_err();
        assert_eq!(sqlstate(&error), "23514", "{table}.{column}");
    }
    // A redacted row keeps each content column NULL, also in the tables that change.
    for (table, id, column) in columns {
        let error = run(
            &test,
            false,
            &format!("UPDATE {table} SET {column} = NULL, redacted_at = now() WHERE id = '{id}'"),
        )
        .await
        .err();
        // A table with more content columns refuses a tombstone that keeps one of them.
        if let Some(error) = error {
            assert_eq!(sqlstate(&error), "23514", "{table}.{column}");
        }
    }
    for statement in [
        "UPDATE person SET name = NULL, email = NULL, phone = NULL, user_id = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000111'",
        "UPDATE institution SET name = NULL, email = NULL, phone = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000112'",
        "UPDATE open_question SET text = NULL, redacted_at = now() WHERE id = '0190a000-0000-7000-8000-000000000101'",
        "UPDATE workstream SET name = NULL, redacted_at = now() WHERE id = '0190a000-0000-7000-8000-000000000121'",
        "UPDATE action SET title = NULL, description = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000122'",
        "UPDATE commitment SET text = NULL, condition = NULL, firm_reason = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000123'",
        "UPDATE document SET name = NULL, redacted_at = now() WHERE id = '0190a000-0000-7000-8000-000000000094'",
    ] {
        run(&test, false, statement)
            .await
            .unwrap_or_else(|error| panic!("{statement}: {error}"));
    }
}

/// Content never comes back into a redacted row of a table that changes; its structure still can change.
#[tokio::test]
async fn a_redacted_person_takes_no_content_back() {
    let test = database().await;
    let person = "0190a000-0000-7000-8000-000000000111";
    run(
        &test,
        false,
        &format!(
            "UPDATE person SET name = NULL, email = NULL, phone = NULL, user_id = NULL, redacted_at = now()
             WHERE id = '{person}'"
        ),
    )
    .await
    .unwrap();
    for set in [
        "name = 'Max Muster'",
        "email = 'max@example.org'",
        "phone = '+41 00 000 00 00'",
        "user_id = '0190a000-0000-7000-8000-000000000002'",
    ] {
        let error = run(
            &test,
            false,
            &format!("UPDATE person SET {set} WHERE id = '{person}'"),
        )
        .await
        .unwrap_err();
        assert_eq!(sqlstate(&error), "23514", "{set}");
    }
    run(
        &test,
        false,
        &format!("UPDATE person SET version = version + 1 WHERE id = '{person}'"),
    )
    .await
    .unwrap();
    // A tombstone that keeps a content column fails.
    let error = run(
        &test,
        false,
        "UPDATE commitment SET text = NULL, redacted_at = now()
         WHERE id = '0190a000-0000-7000-8000-000000000123'",
    )
    .await
    .unwrap_err();
    assert_eq!(sqlstate(&error), "23514");
}

/// The event status, the retention periods, the redaction record and the dispute note refuse invalid states.
#[tokio::test]
async fn the_new_tables_and_columns_refuse_invalid_states() {
    let test = database().await;
    let event = "0190a000-0000-7000-8000-000000000003";
    let organization = "0190a000-0000-7000-8000-000000000001";
    for statement in [
        format!("UPDATE event SET status = 'archived', closed_at = now() WHERE id = '{event}'"),
        format!("UPDATE event SET status = 'closed' WHERE id = '{event}'"),
        format!("UPDATE event SET closed_at = now() WHERE id = '{event}'"),
        format!("UPDATE event SET redacted_at = now() WHERE id = '{event}'"),
        format!("UPDATE organization SET retention_source_months = 0 WHERE id = '{organization}'"),
        format!("UPDATE organization SET retention_closed_event_months = 121 WHERE id = '{organization}'"),
        format!(
            "INSERT INTO redaction (id, organization_id, target_kind, target_id, reason, redacted_at)
             VALUES (gen_random_uuid(), '{organization}', 'workstream', gen_random_uuid(), 'request', now())"
        ),
        format!(
            "INSERT INTO redaction (id, organization_id, target_kind, target_id, reason, redacted_at)
             VALUES (gen_random_uuid(), '{organization}', 'person', gen_random_uuid(), 'mood', now())"
        ),
        format!(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES (gen_random_uuid(), '{organization}', 'person', gen_random_uuid(), 'Falsch',
                     '0190a000-0000-7000-8000-000000000002', now())"
        ),
        format!(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES (gen_random_uuid(), '{organization}', 'proposal', gen_random_uuid(), '',
                     '0190a000-0000-7000-8000-000000000002', now())"
        ),
        format!(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES (gen_random_uuid(), '{organization}', 'proposal', gen_random_uuid(), repeat('x', 1001),
                     '0190a000-0000-7000-8000-000000000002', now())"
        ),
        format!(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES (gen_random_uuid(), '{organization}', 'proposal', gen_random_uuid(), NULL,
                     '0190a000-0000-7000-8000-000000000002', now())"
        ),
    ] {
        let error = run(&test, false, &statement).await.expect_err(&statement);
        assert_eq!(sqlstate(&error), "23514", "{statement}");
    }

    // A target is redacted once.
    let insert = format!(
        "INSERT INTO redaction (id, organization_id, target_kind, target_id, reason, actor_id, redacted_at)
         VALUES (gen_random_uuid(), '{organization}', 'person', '0190a000-0000-7000-8000-000000000111',
                 'request', '0190a000-0000-7000-8000-000000000002', now())"
    );
    run(&test, false, &insert).await.unwrap();
    let error = run(&test, false, &insert).await.unwrap_err();
    assert_eq!(sqlstate(&error), "23505");

    // A closed event, a redacted closed event and a note with its maximum length are valid.
    run(
        &test,
        false,
        &format!("UPDATE event SET status = 'closed', closed_at = now(), redacted_at = now() WHERE id = '{event}'"),
    )
    .await
    .unwrap();
    run(
        &test,
        false,
        &format!(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES (gen_random_uuid(), '{organization}', 'source-item', gen_random_uuid(), repeat('ä', 1000),
                     '0190a000-0000-7000-8000-000000000002', now())"
        ),
    )
    .await
    .unwrap();
}
