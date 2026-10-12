//! The schema of milestones, decisions, risks and requirements (migration 0029): keys and CHECK constraints.

#![allow(clippy::unwrap_used)]

use sqlx::AssertSqlSafe;
use tada_store_pg::testing::{TestDatabase, sqlstate};

const CHECK_VIOLATION: &str = "23514";
const FOREIGN_KEY_VIOLATION: &str = "23503";
const UNIQUE_VIOLATION: &str = "23505";

const ORG: &str = "0190a000-0000-7000-8000-0000000000a1";
const USER: &str = "0190a000-0000-7000-8000-0000000000b1";
const EVENT: &str = "0190a000-0000-7000-8000-0000000000e1";
const OTHER_EVENT: &str = "0190a000-0000-7000-8000-0000000000e2";
const PERSON: &str = "0190a000-0000-7000-8000-0000000000f1";
const INSTITUTION: &str = "0190a000-0000-7000-8000-0000000000f2";

/// A test database with an invented organization, user, two events, a person and an institution.
async fn database() -> TestDatabase {
    let test = TestDatabase::start().await;
    run(
        &test,
        &format!(
            "INSERT INTO organization (id, slug, name, created_at)
             VALUES ('{ORG}', 'testwil', 'Testwil Club', now());
             INSERT INTO app_user (id, display_name, created_at) VALUES ('{USER}', 'Anna Muster', now());
             INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ('{EVENT}', '{ORG}', 'OPEN', 'Open Day', 'Europe/Zurich', 1, now()),
                    ('{OTHER_EVENT}', '{ORG}', 'FEST', 'Festival', 'Europe/Zurich', 1, now());
             INSERT INTO person (id, organization_id, local_number, name, version, created_at, updated_at)
             VALUES ('{PERSON}', '{ORG}', 1, 'Berta Beispiel', 1, now(), now());
             INSERT INTO institution (id, organization_id, local_number, name, kind, version, created_at, updated_at)
             VALUES ('{INSTITUTION}', '{ORG}', 1, 'Testwil Gemeinde', 'authority', 1, now(), now());"
        ),
    )
    .await
    .unwrap();
    test
}

async fn run(test: &TestDatabase, statement: &str) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(AssertSqlSafe(statement.to_owned()))
        .execute(test.pool())
        .await
        .map(|_| ())
}

async fn refused(test: &TestDatabase, statement: &str, code: &str) {
    let error = run(test, statement).await.expect_err(statement);
    assert_eq!(sqlstate(&error), code, "{statement}");
}

fn id(n: u32) -> String {
    format!("0190a000-0000-7000-8000-{n:012}")
}

fn milestone(n: u32, event: &str) -> String {
    format!(
        "INSERT INTO milestone (id, organization_id, event_id, local_number, title, date, owner_user_id,
             status, version, created_at, updated_at)
         VALUES ('{}', '{ORG}', '{event}', {n}, 'Doors open', '2027-05-01', '{USER}', 'open', 1, now(), now())",
        id(100 + n)
    )
}

/// A decision in `status`, with a successor and a reason as given (`NULL` or a quoted ID or text).
fn decision(n: u32, status: &str, superseded_by: &str, reason: &str) -> String {
    format!(
        "INSERT INTO decision (id, organization_id, event_id, local_number, wording, decided_on, owner_user_id,
             status, superseded_by, revoked_reason, version, created_at, updated_at)
         VALUES ('{}', '{ORG}', '{EVENT}', {n}, 'Entry is free', '2027-01-10', '{USER}', '{status}',
             {superseded_by}, {reason}, 1, now(), now())",
        id(200 + n)
    )
}

fn requirement(n: u32, status: &str, reason: &str) -> String {
    format!(
        "INSERT INTO requirement (id, organization_id, event_id, local_number, text, owner_user_id, status,
             waived_reason, version, created_at, updated_at)
         VALUES ('{}', '{ORG}', '{EVENT}', {n}, 'Fire safety plan', '{USER}', '{status}', {reason}, 1, now(), now())",
        id(300 + n)
    )
}

fn action(n: u32, start: &str, due: &str, milestone: &str) -> String {
    format!(
        "INSERT INTO action (id, organization_id, event_id, local_number, title, owner_user_id, start_date,
             due_date, milestone_id, status, version, created_at, updated_at)
         VALUES ('{}', '{ORG}', '{EVENT}', {n}, 'Print flyers', '{USER}', {start}, {due}, {milestone}, 'open',
             1, now(), now())",
        id(400 + n)
    )
}

#[tokio::test]
async fn local_numbers_are_unique_in_an_event() {
    let test = database().await;
    run(&test, &milestone(1, EVENT)).await.unwrap();
    let again = milestone(1, EVENT).replace(&id(101), &id(150));
    refused(&test, &again, UNIQUE_VIOLATION).await;
    // The same number in another event is fine.
    let other = milestone(1, OTHER_EVENT).replace(&id(101), &id(151));
    run(&test, &other).await.unwrap();
}

#[tokio::test]
async fn superseded_needs_a_successor() {
    let test = database().await;
    run(&test, &decision(1, "in-force", "NULL", "NULL"))
        .await
        .unwrap();
    let successor = format!("'{}'", id(201));
    // A superseded decision names its successor, and only such a decision does.
    refused(
        &test,
        &decision(2, "superseded", "NULL", "NULL"),
        CHECK_VIOLATION,
    )
    .await;
    refused(
        &test,
        &decision(2, "in-force", &successor, "NULL"),
        CHECK_VIOLATION,
    )
    .await;
    run(&test, &decision(2, "superseded", &successor, "NULL"))
        .await
        .unwrap();
    // A decision is not its own successor.
    let own = format!("'{}'", id(203));
    refused(
        &test,
        &decision(3, "superseded", &own, "NULL"),
        CHECK_VIOLATION,
    )
    .await;
    // The successor belongs to the same event.
    let foreign = decision(4, "superseded", &successor, "NULL")
        .replace(&format!("'{EVENT}', 4"), &format!("'{OTHER_EVENT}', 4"));
    refused(&test, &foreign, FOREIGN_KEY_VIOLATION).await;
}

#[tokio::test]
async fn revoked_needs_a_reason() {
    let test = database().await;
    refused(
        &test,
        &decision(1, "revoked", "NULL", "NULL"),
        CHECK_VIOLATION,
    )
    .await;
    refused(
        &test,
        &decision(1, "in-force", "NULL", "'No longer needed'"),
        CHECK_VIOLATION,
    )
    .await;
    run(&test, &decision(1, "revoked", "NULL", "'No longer needed'"))
        .await
        .unwrap();
}

#[tokio::test]
async fn waived_needs_a_reason() {
    let test = database().await;
    refused(&test, &requirement(1, "waived", "NULL"), CHECK_VIOLATION).await;
    refused(
        &test,
        &requirement(1, "open", "'Not applicable'"),
        CHECK_VIOLATION,
    )
    .await;
    run(&test, &requirement(1, "waived", "'Not applicable'"))
        .await
        .unwrap();
    // A redacted requirement has no text and no reason, whatever its status.
    run(
        &test,
        &format!(
            "INSERT INTO requirement (id, organization_id, event_id, local_number, owner_user_id, status,
                 version, created_at, updated_at, redacted_at)
             VALUES ('{}', '{ORG}', '{EVENT}', 2, '{USER}', 'waived', 1, now(), now(), now())",
            id(302)
        ),
    )
    .await
    .unwrap();
    refused(
        &test,
        &format!(
            "UPDATE requirement SET text = NULL WHERE id = '{}'",
            id(301)
        ),
        CHECK_VIOLATION,
    )
    .await;
}

#[tokio::test]
async fn requirement_names_at_most_one_party() {
    let test = database().await;
    let both = requirement(1, "open", "NULL").replace(
        "owner_user_id,",
        "person_id, institution_id, owner_user_id,",
    );
    let both = both.replace(
        &format!("'Fire safety plan', '{USER}'"),
        &format!("'Fire safety plan', '{PERSON}', '{INSTITUTION}', '{USER}'"),
    );
    refused(&test, &both, CHECK_VIOLATION).await;
    run(&test, &requirement(1, "open", "NULL")).await.unwrap();
}

#[tokio::test]
async fn decision_party_names_exactly_one_party() {
    let test = database().await;
    run(&test, &decision(1, "in-force", "NULL", "NULL"))
        .await
        .unwrap();
    let party = |position: u32, person: &str, institution: &str| {
        format!(
            "INSERT INTO decision_party (organization_id, decision_id, position, person_id, institution_id)
             VALUES ('{ORG}', '{}', {position}, {person}, {institution})",
            id(201)
        )
    };
    let person = format!("'{PERSON}'");
    let institution = format!("'{INSTITUTION}'");
    refused(&test, &party(0, "NULL", "NULL"), CHECK_VIOLATION).await;
    refused(&test, &party(0, &person, &institution), CHECK_VIOLATION).await;
    run(&test, &party(0, &person, "NULL")).await.unwrap();
    run(&test, &party(1, "NULL", &institution)).await.unwrap();
    // A position holds one party.
    refused(&test, &party(1, &person, "NULL"), UNIQUE_VIOLATION).await;
}

#[tokio::test]
async fn action_start_must_not_follow_due() {
    let test = database().await;
    refused(
        &test,
        &action(1, "'2027-03-02'", "'2027-03-01'", "NULL"),
        CHECK_VIOLATION,
    )
    .await;
    run(&test, &action(1, "'2027-03-01'", "'2027-03-01'", "NULL"))
        .await
        .unwrap();
    // A missing date never conflicts.
    run(&test, &action(2, "'2027-03-02'", "NULL", "NULL"))
        .await
        .unwrap();
    run(&test, &action(3, "NULL", "'2027-03-01'", "NULL"))
        .await
        .unwrap();
}

#[tokio::test]
async fn action_milestone_must_be_of_the_same_event() {
    let test = database().await;
    run(&test, &milestone(1, OTHER_EVENT)).await.unwrap();
    run(&test, &milestone(2, EVENT)).await.unwrap();
    refused(
        &test,
        &action(1, "NULL", "NULL", &format!("'{}'", id(101))),
        FOREIGN_KEY_VIOLATION,
    )
    .await;
    run(&test, &action(1, "NULL", "NULL", &format!("'{}'", id(102))))
        .await
        .unwrap();
}

#[tokio::test]
async fn evidence_names_exactly_one_record() {
    let test = database().await;
    let evidence = |columns: &str| {
        let values = columns
            .split(", ")
            .filter(|column| !column.is_empty())
            .map(|_| format!("'{}'", id(999)))
            .collect::<Vec<_>>()
            .join(", ");
        let (columns, values) = if columns.is_empty() {
            (String::new(), String::new())
        } else {
            (format!("{columns}, "), format!("{values}, "))
        };
        format!(
            "INSERT INTO record_evidence (id, organization_id, {columns}record_version, proposal_id,
                 source_version_id, start_offset, end_offset, quote)
             VALUES ('{}', '{ORG}', {values}1, '{}', '{}', 0, 5, 'quote')",
            id(500),
            id(501),
            id(502)
        )
    };
    refused(&test, &evidence(""), CHECK_VIOLATION).await;
    refused(
        &test,
        &evidence("milestone_id, decision_id"),
        CHECK_VIOLATION,
    )
    .await;
    refused(&test, &evidence("risk_id, requirement_id"), CHECK_VIOLATION).await;
    refused(
        &test,
        &evidence("action_id, requirement_id"),
        CHECK_VIOLATION,
    )
    .await;
    // One record column passes the CHECK. The record does not exist, so the foreign key refuses it.
    for column in [
        "action_id",
        "milestone_id",
        "decision_id",
        "risk_id",
        "requirement_id",
    ] {
        refused(&test, &evidence(column), FOREIGN_KEY_VIOLATION).await;
    }
}

#[tokio::test]
async fn proposal_can_target_the_new_kinds() {
    let test = database().await;
    for kind in ["milestone", "decision", "risk", "requirement"] {
        let error = run(
            &test,
            &format!(
                "INSERT INTO proposal (id, organization_id, changeset_id, event_id, operation, operation_version,
                     target_kind, target_id, reason, created_at)
                 VALUES ('{}', '{ORG}', '{}', '{EVENT}', '{{}}', 1, '{kind}', '{}', 'why', now())",
                id(600),
                id(601),
                id(602)
            ),
        )
        .await
        .expect_err(kind);
        // The kind passes the CHECK. The changeset does not exist, so the foreign key refuses it.
        assert_eq!(sqlstate(&error), FOREIGN_KEY_VIOLATION, "{kind}");
    }
}
