//! The tables of work records and parties (migration 0024) refuse the states that the domain forbids.

#![allow(clippy::unwrap_used)]

use sqlx::types::Uuid;
use tada_store_pg::testing::{TestDatabase, sqlstate};

const CHECK_VIOLATION: &str = "23514";
const FOREIGN_KEY_VIOLATION: &str = "23503";

/// The fixed rows that a commitment refers to.
struct Rows {
    organization: Uuid,
    event: Uuid,
    owner: Uuid,
}

async fn insert_commitment(
    test: &TestDatabase,
    rows: &Rows,
    number: i64,
    parties: (Option<Uuid>, Option<Uuid>),
    status_and_condition: (&str, Option<&str>),
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO commitment
             (id, organization_id, event_id, local_number, text, condition, person_id, institution_id,
              owner_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, 'Deliver the tables', $5, $6, $7, $8, $9, 1, now(), now())",
    )
    .bind(Uuid::now_v7())
    .bind(rows.organization)
    .bind(rows.event)
    .bind(number)
    .bind(status_and_condition.1)
    .bind(parties.0)
    .bind(parties.1)
    .bind(rows.owner)
    .bind(status_and_condition.0)
    .execute(test.pool())
    .await
    .map(|_| ())
}

#[tokio::test]
async fn the_work_record_tables_enforce_one_promisor_and_a_condition_for_conditional() {
    let test = TestDatabase::start().await;
    let organization_id = test.create_organization("work-club").await;
    let organization = organization_id.as_uuid();
    let event = test.create_event(organization_id, "OPEN1").await.as_uuid();
    let owner = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO app_user (id, display_name, created_at) VALUES ($1, 'Ada Test', now())",
    )
    .bind(owner)
    .execute(test.pool())
    .await
    .unwrap();
    let rows = Rows {
        organization,
        event,
        owner,
    };
    let person = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO person (id, organization_id, local_number, name, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Max Muster', 1, now(), now())",
    )
    .bind(person)
    .bind(organization)
    .execute(test.pool())
    .await
    .unwrap();
    let institution = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO institution (id, organization_id, local_number, name, kind, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Testwil Town Office', 'authority', 1, now(), now())",
    )
    .bind(institution)
    .bind(organization)
    .execute(test.pool())
    .await
    .unwrap();

    let both = insert_commitment(
        &test,
        &rows,
        1,
        (Some(person), Some(institution)),
        ("firm", None),
    )
    .await
    .unwrap_err();
    assert_eq!(sqlstate(&both), CHECK_VIOLATION);

    let none = insert_commitment(&test, &rows, 2, (None, None), ("firm", None))
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&none), CHECK_VIOLATION);

    let no_condition =
        insert_commitment(&test, &rows, 3, (Some(person), None), ("conditional", None))
            .await
            .unwrap_err();
    assert_eq!(sqlstate(&no_condition), CHECK_VIOLATION);

    insert_commitment(
        &test,
        &rows,
        4,
        (Some(person), None),
        ("conditional", Some("If the permit arrives")),
    )
    .await
    .unwrap();
    insert_commitment(&test, &rows, 5, (None, Some(institution)), ("firm", None))
        .await
        .unwrap();

    // A workstream of another event is no valid workstream of an action (composite foreign key).
    let other_event = test.create_event(organization_id, "OTHER2").await.as_uuid();
    let workstream = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO workstream (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, 'Stage', $4, 'active', 1, now(), now())",
    )
    .bind(workstream)
    .bind(organization)
    .bind(other_event)
    .bind(owner)
    .execute(test.pool())
    .await
    .unwrap();
    let foreign = sqlx::query(
        "INSERT INTO action (id, organization_id, event_id, local_number, title, owner_user_id, workstream_id,
                             status, version, created_at, updated_at)
         VALUES ($1, $2, $3, 1, 'Book the stage', $4, $5, 'open', 1, now(), now())",
    )
    .bind(Uuid::now_v7())
    .bind(organization)
    .bind(event)
    .bind(owner)
    .bind(workstream)
    .execute(test.pool())
    .await
    .unwrap_err();
    assert_eq!(sqlstate(&foreign), FOREIGN_KEY_VIOLATION);
}
