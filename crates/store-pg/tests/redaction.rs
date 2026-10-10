//! The redaction store (ADR 0072): one transaction writes the tombstones of the targets and the content that
//! copies them, a `redaction` row, an audit event and a journal job for each target.
//! The rows come from the fixtures of 0015 and 0025.

#![allow(clippy::unwrap_used)]

use tada_app::caller::{MemberCaller, OrgScope, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::domain::redaction::{RedactionReason, RedactionTarget, RedactionTargetKind};
use tada_app::redaction::{JOURNAL_JOB, RedactionOutcome, RedactionStore};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const ORGANIZATION: &str = "0190a000-0000-7000-8000-000000000001";
const OWNER: &str = "0190a000-0000-7000-8000-000000000002";
const EVENT: &str = "0190a000-0000-7000-8000-000000000003";
const MEMBER_TEXT: &str = "0190a000-0000-7000-8000-000000000011";
const UPLOAD_VERSION: &str = "0190a000-0000-7000-8000-000000000092";
const UPLOAD_SOURCE: &str = "0190a000-0000-7000-8000-000000000015";
const PERSON: &str = "0190a000-0000-7000-8000-000000000111";

fn uuid(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}

fn owner() -> MemberCaller {
    MemberCaller::new(
        UserId::from_uuid(uuid(OWNER)),
        OrganizationId::from_uuid(uuid(ORGANIZATION)),
        OrganizationRole::Owner,
    )
}

fn scope() -> OrgScope {
    owner().scope()
}

fn target(kind: RedactionTargetKind, id: &str) -> RedactionTarget {
    RedactionTarget { kind, id: uuid(id) }
}

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

async fn redact(
    test: &TestDatabase,
    targets: &[RedactionTarget],
    reason: RedactionReason,
) -> RedactionOutcome {
    test.database
        .redact(scope(), targets, reason, &owner().actor())
        .await
        .unwrap()
        .unwrap()
}

async fn count(test: &TestDatabase, sql: &str) -> i64 {
    test.scalar::<i64>(sql).await
}

#[tokio::test]
async fn redact_source_item_clears_text_hash_and_quotes() {
    let test = database().await;
    let outcome = redact(
        &test,
        &[target(RedactionTargetKind::SourceItem, MEMBER_TEXT)],
        RedactionReason::Request,
    )
    .await;
    assert_eq!(outcome.applied.len(), 1);
    assert_eq!(outcome.already, 0);
    assert!(outcome.blob_keys.is_empty());

    let item = format!("source_item_id = '{MEMBER_TEXT}'");
    assert_eq!(
        count(
            &test,
            &format!(
                "SELECT count(*) FROM source_version WHERE {item} AND redacted_at IS NOT NULL
               AND text IS NULL AND url IS NULL AND sha256 IS NULL AND search = ''::tsvector
               AND author_actor = jsonb_build_object('kind', 'member', 'id', '{OWNER}')"
            )
        )
        .await,
        2,
        "both versions are tombstones with the actor kind and ID only"
    );
    assert_eq!(
        count(&test, &format!("SELECT count(*) FROM source_item WHERE id = '{MEMBER_TEXT}' AND redacted_at IS NOT NULL")).await,
        1
    );
    // The quotes that copy the text: evidence links, proposal evidence and record evidence.
    for table in ["evidence_link", "proposal_evidence", "record_evidence"] {
        assert_eq!(
            count(
                &test,
                &format!("SELECT count(*) FROM {table} WHERE quote IS NOT NULL")
            )
            .await,
            0,
            "{table}"
        );
        assert_eq!(
            count(
                &test,
                &format!("SELECT count(*) FROM {table} WHERE redacted_at IS NULL")
            )
            .await,
            0,
            "{table}"
        );
    }
    // The proposals and fact versions are separate targets; the source redaction keeps them.
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM proposal WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM fact_version WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
    // The upload item is another item and keeps its text and hash.
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM source_version WHERE redacted_at IS NULL AND sha256 IS NOT NULL"
        )
        .await,
        2
    );
    // One redaction row, one audit event with the reason, and one journal job.
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM redaction WHERE target_kind = 'source-item' AND target_id = '{MEMBER_TEXT}'
               AND reason = 'request' AND actor_id = '{OWNER}' AND id = '{}'",
            outcome.applied[0]
        ))
        .await,
        1
    );
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM audit_event WHERE action = 'source_item.redact' AND record_kind = 'source_item'
               AND record_id = '{MEMBER_TEXT}' AND detail = '{{\"reason\": \"request\"}}'::jsonb"
        ))
        .await,
        1
    );
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM job WHERE kind = '{JOURNAL_JOB}' AND version = 1
               AND organization_id = '{ORGANIZATION}' AND payload = jsonb_build_object('redaction_id', '{}')",
            outcome.applied[0]
        ))
        .await,
        1
    );
    assert!(
        test.database
            .is_redacted(
                scope(),
                target(RedactionTargetKind::SourceItem, MEMBER_TEXT)
            )
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn redact_upload_version_deletes_its_object_and_its_text() {
    let test = database().await;
    let outcome = redact(
        &test,
        &[target(RedactionTargetKind::DocumentVersion, UPLOAD_VERSION)],
        RedactionReason::Request,
    )
    .await;
    assert_eq!(
        outcome
            .blob_keys
            .iter()
            .map(|key| key.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec![format!(
            "{ORGANIZATION}/0190a000-0000-7000-8000-000000000192"
        )]
    );
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM document_version WHERE id = '{UPLOAD_VERSION}' AND redacted_at IS NOT NULL
               AND file_name IS NULL AND sha256 IS NULL AND markdown IS NULL AND blob_key IS NOT NULL"
        ))
        .await,
        1
    );
    // The source version of the upload holds the text and the hash of the same file.
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM source_version WHERE id = '{UPLOAD_SOURCE}' AND redacted_at IS NOT NULL
               AND sha256 IS NULL"
        ))
        .await,
        1
    );
    // The other version of the upload stays.
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM document_version WHERE redacted_at IS NULL"
        )
        .await,
        3
    );
}

#[tokio::test]
async fn redact_event_clears_all_event_content_keeps_ids() {
    let test = database().await;
    sqlx::query(
        "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
         VALUES ($1, $2, 'fact-version', '0190a000-0000-7000-8000-000000000052', 'Das Motto war anders.', $3, now())",
    )
    .bind(Uuid::now_v7())
    .bind(uuid(ORGANIZATION))
    .bind(uuid(OWNER))
    .execute(test.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE event SET status = 'closed', closed_at = now() WHERE id = $1")
        .bind(uuid(EVENT))
        .execute(test.pool())
        .await
        .unwrap();
    let tables = [
        "source_item",
        "source_version",
        "fact_version",
        "evidence_link",
        "proposal",
        "proposal_evidence",
        "record_evidence",
        "document",
        "document_version",
        "open_question",
        "workstream",
        "action",
        "commitment",
        "dispute_note",
        "review_result",
        "field_definition",
    ];
    let mut before = Vec::new();
    for table in tables {
        before.push(count(&test, &format!("SELECT count(*) FROM {table}")).await);
    }

    let outcome = redact(
        &test,
        &[target(RedactionTargetKind::Event, EVENT)],
        RedactionReason::Retention,
    )
    .await;
    assert_eq!(outcome.applied.len(), 1);
    let mut keys: Vec<_> = outcome
        .blob_keys
        .iter()
        .map(|key| key.as_str().to_owned())
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            format!("{ORGANIZATION}/0190a000-0000-7000-8000-000000000192"),
            format!("{ORGANIZATION}/0190a000-0000-7000-8000-000000000193"),
        ]
    );

    // Each row stays; only its content goes.
    for (table, rows) in tables.iter().zip(before) {
        assert_eq!(
            count(&test, &format!("SELECT count(*) FROM {table}")).await,
            rows,
            "{table}"
        );
    }
    for (table, content) in [
        (
            "source_version",
            "text IS NULL AND url IS NULL AND sha256 IS NULL",
        ),
        ("fact_version", "value IS NULL"),
        ("evidence_link", "quote IS NULL"),
        (
            "proposal",
            "operation = jsonb_build_object('kind', operation -> 'kind') AND reason IS NULL
             AND manifest IS NULL AND lint_warnings IS NULL",
        ),
        ("proposal_evidence", "quote IS NULL"),
        ("record_evidence", "quote IS NULL"),
        ("document", "name IS NULL"),
        (
            "document_version",
            "markdown IS NULL AND file_name IS NULL AND sha256 IS NULL",
        ),
        ("open_question", "text IS NULL"),
        ("workstream", "name IS NULL"),
        ("action", "title IS NULL AND description IS NULL"),
        (
            "commitment",
            "text IS NULL AND condition IS NULL AND firm_reason IS NULL",
        ),
        ("dispute_note", "text IS NULL"),
    ] {
        assert_eq!(
            count(
                &test,
                &format!(
                    "SELECT count(*) FROM {table} WHERE redacted_at IS NULL OR NOT ({content})"
                )
            )
            .await,
            0,
            "{table}"
        );
    }
    // The item of the event and the upload item of its document.
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM source_item WHERE redacted_at IS NULL"
        )
        .await,
        1
    );
    // The name of the event, the field labels, statuses, dates, numbers and IDs stay.
    assert_eq!(
        test.scalar::<String>(&format!(
            "SELECT name FROM event WHERE id = '{EVENT}' AND redacted_at IS NOT NULL"
        ))
        .await,
        "Open Day Testwil"
    );
    assert_eq!(
        test.scalar::<String>(
            "SELECT label_text FROM field_definition WHERE id = '0190a000-0000-7000-8000-000000000021'"
        )
        .await,
        "Motto"
    );
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM fact_version WHERE state = 'accepted' AND number IN (1, 2)"
        )
        .await,
        2
    );
    assert_eq!(
        count(&test, "SELECT count(*) FROM action WHERE status = 'open' AND local_number = 1 AND due_date = '2030-06-01'").await,
        1
    );
    // The persons and institutions are the directory of the organization; they stay.
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM person WHERE name = 'Max Muster'"
        )
        .await,
        1
    );
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM audit_event WHERE action = 'event.redact' AND record_id = '{EVENT}'
               AND detail = '{{\"reason\": \"retention\"}}'::jsonb"
        ))
        .await,
        1
    );
}

#[tokio::test]
async fn redact_person_clears_contact_and_link() {
    let test = database().await;
    sqlx::query("UPDATE person SET user_id = $1 WHERE id = $2")
        .bind(uuid(OWNER))
        .bind(uuid(PERSON))
        .execute(test.pool())
        .await
        .unwrap();
    redact(
        &test,
        &[target(RedactionTargetKind::Person, PERSON)],
        RedactionReason::Request,
    )
    .await;
    assert_eq!(
        count(
            &test,
            &format!(
                "SELECT count(*) FROM person WHERE id = '{PERSON}' AND redacted_at IS NOT NULL
               AND name IS NULL AND email IS NULL AND phone IS NULL AND user_id IS NULL
               AND local_number = 1 AND version = 2"
            )
        )
        .await,
        1
    );
    // The commitment keeps its promisor by ID.
    assert_eq!(
        count(
            &test,
            &format!(
                "SELECT count(*) FROM commitment WHERE person_id = '{PERSON}' AND text IS NOT NULL"
            )
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM audit_event WHERE action = 'person.redact'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn redact_twice_is_noop() {
    let test = database().await;
    let targets = [
        target(RedactionTargetKind::SourceItem, MEMBER_TEXT),
        target(RedactionTargetKind::Person, PERSON),
    ];
    let first = redact(&test, &targets, RedactionReason::Request).await;
    assert_eq!((first.applied.len(), first.already), (2, 0));
    let second = redact(&test, &targets, RedactionReason::Request).await;
    assert_eq!(
        second,
        RedactionOutcome {
            applied: Vec::new(),
            already: 2,
            blob_keys: Vec::new()
        }
    );
    assert_eq!(count(&test, "SELECT count(*) FROM redaction").await, 2);
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM audit_event WHERE action LIKE '%.redact'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &test,
            &format!("SELECT count(*) FROM job WHERE kind = '{JOURNAL_JOB}'")
        )
        .await,
        2
    );
}

#[tokio::test]
async fn redacting_an_open_proposal_rejects_it() {
    let test = database().await;
    let changeset = Uuid::now_v7();
    let proposal = Uuid::now_v7();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
         SELECT '{changeset}', organization_id, event_id, author, source_version_id, now()
         FROM changeset WHERE id = '0190a000-0000-7000-8000-000000000032';
         INSERT INTO proposal
             (id, organization_id, changeset_id, event_id, operation, operation_version, target_kind, target_id,
              expected_version, reason, created_at)
         SELECT '{proposal}', organization_id, '{changeset}', event_id, operation, operation_version, target_kind,
                target_id, 2, 'Max Muster nennt ein Motto.', now()
         FROM proposal WHERE id = '0190a000-0000-7000-8000-000000000043';"
    )))
    .execute(test.pool())
    .await
    .unwrap();

    redact(
        &test,
        &[RedactionTarget {
            kind: RedactionTargetKind::Proposal,
            id: proposal,
        }],
        RedactionReason::Request,
    )
    .await;
    assert_eq!(
        count(
            &test,
            &format!(
                "SELECT count(*) FROM proposal WHERE id = '{proposal}' AND redacted_at IS NOT NULL
               AND operation = '{{\"kind\": \"set_fact\"}}'::jsonb AND reason IS NULL"
            )
        )
        .await,
        1
    );
    // An emptied proposal does not stay open: the redaction rejects it with the actor as reviewer.
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM review_result WHERE proposal_id = '{proposal}' AND result = 'rejected'
               AND reviewer ->> 'id' = '{OWNER}'"
        ))
        .await,
        1
    );
    assert_eq!(
        count(&test, &format!(
            "SELECT count(*) FROM audit_event WHERE action = 'proposal.reject' AND record_id = '{proposal}'"
        ))
        .await,
        1
    );
    // An accepted proposal keeps its one review result.
    redact(
        &test,
        &[target(
            RedactionTargetKind::Proposal,
            "0190a000-0000-7000-8000-000000000042",
        )],
        RedactionReason::Request,
    )
    .await;
    assert_eq!(
        count(&test, "SELECT count(*) FROM review_result WHERE proposal_id = '0190a000-0000-7000-8000-000000000042'").await,
        1
    );
}

#[tokio::test]
async fn redaction_row_and_audit_hold_no_content() {
    let test = database().await;
    redact(
        &test,
        &[
            target(RedactionTargetKind::SourceItem, MEMBER_TEXT),
            target(RedactionTargetKind::Person, PERSON),
            target(
                RedactionTargetKind::FactVersion,
                "0190a000-0000-7000-8000-000000000052",
            ),
        ],
        RedactionReason::Request,
    )
    .await;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(r)::text FROM redaction r
         UNION ALL SELECT row_to_json(a)::text FROM audit_event a
         UNION ALL SELECT row_to_json(j)::text FROM job j",
    )
    .fetch_all(test.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 9, "{rows:?}");
    let hashes: Vec<String> = sqlx::query_scalar(
        "SELECT encode(sha256(convert_to(t, 'UTF8')), 'hex') FROM unnest($1::text[]) AS t",
    )
    .bind(vec![
        "Das Motto des Open Day Testwil ist Flieg mit Testwil.",
        "Neu: Das Motto ist Flieg mit uns.",
        "Max Muster",
    ])
    .fetch_all(test.pool())
    .await
    .unwrap();
    for row in &rows {
        let lower = row.to_lowercase();
        for content in [
            "flieg mit",
            "motto",
            "max muster",
            "max@example.org",
            "+41 00",
        ] {
            assert!(!lower.contains(content), "{row} holds {content}");
        }
        for hash in &hashes {
            assert!(!lower.contains(hash), "{row} holds a hash of the content");
        }
    }
}

#[tokio::test]
async fn a_target_of_another_organization_is_not_found() {
    let test = database().await;
    let other = test.create_organization("musterhausen").await;
    let caller = MemberCaller::new(
        UserId::from_uuid(uuid(OWNER)),
        other,
        OrganizationRole::Owner,
    );
    let outcome = test
        .database
        .redact(
            caller.scope(),
            &[target(RedactionTargetKind::Person, PERSON)],
            RedactionReason::Request,
            &caller.actor(),
        )
        .await
        .unwrap();
    assert_eq!(outcome, None);
    // A missing target among others changes nothing.
    let outcome = test
        .database
        .redact(
            scope(),
            &[
                target(RedactionTargetKind::SourceItem, MEMBER_TEXT),
                RedactionTarget {
                    kind: RedactionTargetKind::Person,
                    id: Uuid::now_v7(),
                },
            ],
            RedactionReason::Request,
            &owner().actor(),
        )
        .await
        .unwrap();
    assert_eq!(outcome, None);
    assert_eq!(count(&test, "SELECT count(*) FROM redaction").await, 0);
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM source_version WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM person WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
}
