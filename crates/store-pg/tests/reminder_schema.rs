//! The schema of reminders, schedules and AI usage (migration 0027): the CHECK constraints and the cascade.

#![allow(clippy::unwrap_used)]

use sqlx::AssertSqlSafe;
use tada_store_pg::testing::{TestDatabase, sqlstate};

const CHECK_VIOLATION: &str = "23514";

const ORG: &str = "0190a000-0000-7000-8000-0000000000a1";
const USER: &str = "0190a000-0000-7000-8000-0000000000b1";

/// A test database with an invented organization and user.
async fn database() -> TestDatabase {
    let test = TestDatabase::start().await;
    run(
        &test,
        &format!(
            "INSERT INTO organization (id, slug, name, created_at)
             VALUES ('{ORG}', 'testwil', 'Testwil Club', now());
             INSERT INTO app_user (id, display_name, created_at) VALUES ('{USER}', 'Anna Muster', now());"
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

async fn check_refused(test: &TestDatabase, statement: &str) {
    let error = run(test, statement).await.expect_err(statement);
    assert_eq!(sqlstate(&error), CHECK_VIOLATION, "{statement}");
}

fn intent(id: &str, purpose: &str, status: &str, org: &str, user: &str) -> String {
    format!(
        "INSERT INTO outbound_intent (id, organization_id, user_id, purpose, status, message_id, created_at)
         VALUES ('{id}', {org}, {user}, '{purpose}', '{status}', '{id}', now())"
    )
}

#[tokio::test]
async fn digest_intent_needs_org_and_user() {
    let test = database().await;
    let id = "0190a000-0000-7000-8000-0000000000c1";
    let org = format!("'{ORG}'");
    let user = format!("'{USER}'");
    check_refused(
        &test,
        &intent(id, "telegram-digest", "pending", "NULL", &user),
    )
    .await;
    check_refused(
        &test,
        &intent(id, "telegram-digest", "pending", &org, "NULL"),
    )
    .await;
    run(
        &test,
        &intent(id, "telegram-digest", "pending", &org, &user),
    )
    .await
    .unwrap();
    // The old rules stay: a magic link has a user, and an invitation intent has no user.
    check_refused(
        &test,
        &intent(
            "0190a000-0000-7000-8000-0000000000c2",
            "magic-link",
            "pending",
            "NULL",
            "NULL",
        ),
    )
    .await;
    check_refused(
        &test,
        &intent(
            "0190a000-0000-7000-8000-0000000000c3",
            "invitation",
            "pending",
            &org,
            &user,
        ),
    )
    .await;
}

#[tokio::test]
async fn canceled_is_a_status() {
    let test = database().await;
    let org = format!("'{ORG}'");
    let user = format!("'{USER}'");
    run(
        &test,
        &intent(
            "0190a000-0000-7000-8000-0000000000c1",
            "telegram-digest",
            "canceled",
            &org,
            &user,
        ),
    )
    .await
    .unwrap();
    check_refused(
        &test,
        &intent(
            "0190a000-0000-7000-8000-0000000000c2",
            "telegram-digest",
            "paused",
            &org,
            &user,
        ),
    )
    .await;
}

#[tokio::test]
async fn policy_ranges_are_checked() {
    let test = database().await;
    let policy = |column: &str, value: &str| {
        let mut values = [
            ("weekdays", "31"),
            ("cooldown_hours", "20"),
            ("follow_up_working_days", "2"),
            ("look_ahead_working_days", "2"),
        ];
        for entry in &mut values {
            if entry.0 == column {
                entry.1 = value;
            }
        }
        format!(
            "INSERT INTO pm_policy (organization_id, local_time, time_zone, weekdays, quiet_start, quiet_end,
                 cooldown_hours, follow_up_working_days, look_ahead_working_days)
             VALUES ('{ORG}', '08:30', 'Europe/Zurich', {}, '20:00', '08:00', {}, {}, {})",
            values[0].1, values[1].1, values[2].1, values[3].1
        )
    };
    for (column, value) in [
        ("weekdays", "0"),
        ("weekdays", "128"),
        ("cooldown_hours", "0"),
        ("cooldown_hours", "169"),
        ("follow_up_working_days", "0"),
        ("follow_up_working_days", "11"),
        ("look_ahead_working_days", "-1"),
        ("look_ahead_working_days", "11"),
    ] {
        check_refused(&test, &policy(column, value)).await;
    }
    run(&test, &policy("look_ahead_working_days", "0"))
        .await
        .unwrap();
    let ai_enabled: bool =
        sqlx::query_scalar("SELECT ai_enabled FROM pm_policy WHERE organization_id = $1::uuid")
            .bind(ORG)
            .fetch_one(test.pool())
            .await
            .unwrap();
    assert!(!ai_enabled);
}

#[tokio::test]
async fn digest_items_go_with_their_intent() {
    let test = database().await;
    let id = "0190a000-0000-7000-8000-0000000000c1";
    run(
        &test,
        &intent(
            id,
            "telegram-digest",
            "pending",
            &format!("'{ORG}'"),
            &format!("'{USER}'"),
        ),
    )
    .await
    .unwrap();
    run(
        &test,
        &format!(
            "INSERT INTO digest_item (id, intent_id, organization_id, record_kind, record_id, record_version,
                 reason, position)
             VALUES ('0190a000-0000-7000-8000-0000000000d1', '{id}', '{ORG}', 'action',
                 '0190a000-0000-7000-8000-0000000000e1', 1, 'overdue', 0)"
        ),
    )
    .await
    .unwrap();
    check_refused(
        &test,
        &format!(
            "INSERT INTO digest_item (id, intent_id, organization_id, record_kind, record_id, record_version,
                 reason, position)
             VALUES ('0190a000-0000-7000-8000-0000000000d2', '{id}', '{ORG}', 'action',
                 '0190a000-0000-7000-8000-0000000000e1', 1, 'whenever', 1)"
        ),
    )
    .await;
    run(
        &test,
        &format!("DELETE FROM outbound_intent WHERE id = '{id}'"),
    )
    .await
    .unwrap();
    let items: i64 = sqlx::query_scalar("SELECT count(*) FROM digest_item")
        .fetch_one(test.pool())
        .await
        .unwrap();
    assert_eq!(items, 0);
}

#[tokio::test]
async fn usage_cannot_point_at_an_event_of_another_organization() {
    let test = database().await;
    let other = "0190a000-0000-7000-8000-0000000000a2";
    let event = "0190a000-0000-7000-8000-0000000000f1";
    run(
        &test,
        &format!(
            "INSERT INTO organization (id, slug, name, created_at) VALUES ('{other}', 'andere', 'Anderer Club', now());
             INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ('{event}', '{other}', 'OPEN', 'Open Day', 'Europe/Zurich', 1, now());"
        ),
    )
    .await
    .unwrap();
    let usage = |organization: &str| {
        format!(
            "INSERT INTO ai_usage (id, organization_id, event_id, task, prompt_version, model_id, input_tokens,
                 output_tokens, cost_chf, at)
             VALUES ('0190a000-0000-7000-8000-0000000000d1', '{organization}', '{event}', 'pm-digest', 1,
                 'model-a', 10, 5, 0.01, now())"
        )
    };
    let error = run(&test, &usage(ORG)).await.expect_err("foreign event");
    assert_eq!(sqlstate(&error), "23503");
    run(&test, &usage(other)).await.unwrap();
}
