//! The redaction store (ADR 0072): one transaction writes the tombstones of the targets and the content that
//! copies them, a `redaction` row, an audit event and a journal job for each target.
//! The rows come from the fixtures of 0015 and 0025.

#![allow(clippy::unwrap_used)]

use tada_app::caller::{MemberCaller, OrgScope, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::domain::redaction::{RedactionReason, RedactionTarget, RedactionTargetKind};
use tada_app::redaction::{JOURNAL_JOB, RedactionOutcome, RedactionStore, Refused};
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
    assert_eq!(
        test.database
            .redacted(
                scope(),
                target(RedactionTargetKind::SourceItem, MEMBER_TEXT)
            )
            .await
            .unwrap(),
        Some(true)
    );
    assert_eq!(
        test.database
            .redacted(
                scope(),
                target(RedactionTargetKind::SourceItem, &Uuid::now_v7().to_string())
            )
            .await
            .unwrap(),
        None
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

/// The proposals that created a person or an institution copy its name and contact details.
/// A redaction of the party redacts them too, so no copy stays at the level of the organization.
#[tokio::test]
async fn redact_person_and_institution_clears_the_proposals_that_created_them() {
    let test = database().await;
    let changeset = "0190a000-0000-7000-8000-0000000001c1";
    let create_person = "0190a000-0000-7000-8000-0000000001c2";
    let create_institution = "0190a000-0000-7000-8000-0000000001c3";
    const INSTITUTION: &str = "0190a000-0000-7000-8000-000000000112";
    test.execute(&[
        &format!(
            "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
             VALUES ('{changeset}', '{ORGANIZATION}', NULL,
                     '{{\"kind\": \"member\", \"id\": \"{OWNER}\", \"principal\": null, \"channel\": \"web\", \"request_id\": null}}',
                     '0190a000-0000-7000-8000-000000000012', '2030-05-06T07:00:00Z')"
        ),
        &format!(
            "INSERT INTO proposal
                 (id, organization_id, changeset_id, event_id, operation, operation_version, target_kind, target_id,
                  expected_version, reason, created_at)
             VALUES
                 ('{create_person}', '{ORGANIZATION}', '{changeset}', NULL,
                  '{{\"kind\": \"create_person\", \"id\": \"{PERSON}\", \"name\": \"Max Muster\",
                    \"email\": \"max@example.org\", \"phone\": \"+41 00 000 00 00\"}}',
                  1, 'person', '{PERSON}', NULL, 'Max Muster ist der Kontakt.', '2030-05-06T07:00:00Z'),
                 ('{create_institution}', '{ORGANIZATION}', '{changeset}', NULL,
                  '{{\"kind\": \"create_institution\", \"id\": \"{INSTITUTION}\", \"name\": \"Gemeinde Testwil\",
                    \"institution_kind\": \"authority\", \"email\": \"gemeinde@example.org\", \"phone\": null}}',
                  1, 'institution', '{INSTITUTION}', NULL, 'Die Gemeinde Testwil bewilligt.', '2030-05-06T07:00:00Z')"
        ),
        &format!(
            "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
             SELECT gen_random_uuid(), '{ORGANIZATION}', p, 'accepted',
                    '{{\"kind\": \"member\", \"id\": \"{OWNER}\", \"principal\": null, \"channel\": \"web\", \"request_id\": null}}',
                    '2030-05-06T08:00:00Z'
             FROM unnest(ARRAY['{create_person}', '{create_institution}']::uuid[]) AS p"
        ),
    ])
    .await;

    redact(
        &test,
        &[
            target(RedactionTargetKind::Person, PERSON),
            target(RedactionTargetKind::Institution, INSTITUTION),
        ],
        RedactionReason::Request,
    )
    .await;

    for (proposal, kind) in [
        (create_person, "create_person"),
        (create_institution, "create_institution"),
    ] {
        assert_eq!(
            test.scalar::<serde_json::Value>(&format!(
                "SELECT jsonb_build_object('operation', operation, 'reason', reason,
                                           'redacted', redacted_at IS NOT NULL)
                 FROM proposal WHERE id = '{proposal}'"
            ))
            .await,
            serde_json::json!({"operation": {"kind": kind}, "reason": null, "redacted": true}),
        );
    }
    for text in [
        "Max Muster",
        "max@example.org",
        "+41 00 000 00 00",
        "Testwil",
        "gemeinde@",
    ] {
        assert_eq!(
            count(
                &test,
                &format!(
                    "SELECT count(*) FROM proposal
                     WHERE target_kind IN ('person', 'institution')
                       AND (operation::text LIKE '%{text}%' OR reason LIKE '%{text}%')"
                )
            )
            .await,
            0,
            "{text} is gone from the proposals"
        );
    }
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
    assert_eq!(outcome, Err(Refused::NotFound));
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
    assert_eq!(outcome, Err(Refused::NotFound));
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

#[tokio::test]
async fn redacting_an_open_event_is_refused() {
    let test = database().await;
    let outcome = test
        .database
        .redact(
            scope(),
            &[
                target(RedactionTargetKind::Person, PERSON),
                target(RedactionTargetKind::Event, EVENT),
            ],
            RedactionReason::Retention,
            &owner().actor(),
        )
        .await
        .unwrap();
    assert_eq!(outcome, Err(Refused::EventNotClosed));
    assert_eq!(count(&test, "SELECT count(*) FROM redaction").await, 0);
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM person WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM source_version WHERE redacted_at IS NOT NULL"
        )
        .await,
        0
    );
}

/// A writer that cites a source version holds `FOR SHARE` on it until its commit. A redaction that starts then
/// waits, and its cascade covers the row that the writer added (ADR 0072).
#[tokio::test]
async fn a_redaction_waits_for_a_citing_writer_and_covers_its_row() {
    let test = database().await;
    let mut writer = test.pool().begin().await.unwrap();
    let redacted: Vec<bool> = sqlx::query_scalar(
        "SELECT redacted_at IS NOT NULL FROM source_version WHERE source_item_id = $1 ORDER BY id FOR SHARE",
    )
    .bind(uuid(MEMBER_TEXT))
    .fetch_all(&mut *writer)
    .await
    .unwrap();
    assert_eq!(redacted, [false, false]);
    sqlx::query(
        "INSERT INTO proposal_evidence (id, organization_id, proposal_id, source_version_id, start_offset, end_offset, quote)
         VALUES ($1, $2, '0190a000-0000-7000-8000-000000000043', '0190a000-0000-7000-8000-000000000012', 35, 52,
                 'Flieg mit Testwil')",
    )
    .bind(Uuid::now_v7())
    .bind(uuid(ORGANIZATION))
    .execute(&mut *writer)
    .await
    .unwrap();

    let database = test.database.clone();
    let redacting = tokio::spawn(async move {
        database
            .redact(
                scope(),
                &[target(RedactionTargetKind::SourceItem, MEMBER_TEXT)],
                RedactionReason::Request,
                &owner().actor(),
            )
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !redacting.is_finished(),
        "the redaction waits for the writer"
    );
    writer.commit().await.unwrap();
    redacting.await.unwrap().unwrap().unwrap();

    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM proposal_evidence WHERE quote IS NOT NULL"
        )
        .await,
        0,
        "the row of the writer is a tombstone too"
    );
    assert_eq!(
        count(&test, "SELECT count(*) FROM proposal_evidence").await,
        6
    );
}
