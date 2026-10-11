//! The daily check of the AI-PM (ADR 0076): it plans one digest for each eligible owner, with the items of all
//! events of the organization, and queues its send at the end of the quiet hours of the recipient.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use jiff::Timestamp;
use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::caller::{MemberCaller, OrganizationRole, TestScope};
use tada_app::clock::Clock;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{EventId, OrganizationId, UserId};
use tada_app::domain::reminders::PmPolicy;
use tada_app::jobs::{Job, JobHandler};
use tada_app::pm::check::PmCheck;
use tada_app::pm::policy::PmPolicyStore;
use tada_app::pm::{PM_CHECK_JOB, SEND_DIGEST_JOB};
use tada_store_pg::Database;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// Wednesday 14 October 2026, 10:30 in Zurich: a working day, outside the quiet hours.
const WEDNESDAY: &str = "2026-10-14T08:30:00Z";

#[derive(Debug)]
struct At(Timestamp);

impl Clock for At {
    fn now(&self) -> Timestamp {
        self.0
    }
}

fn ts(text: &str) -> Timestamp {
    text.parse().unwrap()
}

/// A new handler on `database`, as a new worker process makes it.
fn check(database: &Database, now: Timestamp) -> PmCheck {
    PmCheck::new(
        Arc::new(database.clone()),
        Arc::new(database.clone()),
        Arc::new(database.clone()),
        Arc::new(At(now)),
    )
}

fn job(organization: OrganizationId) -> Job {
    Job {
        id: Uuid::now_v7(),
        kind: PM_CHECK_JOB.to_owned(),
        version: 1,
        payload: serde_json::json!({}),
        organization_id: Some(organization),
        request_id: None,
        attempt: 1,
        max_attempts: 5,
    }
}

async fn run(database: &Database, now: Timestamp, organization: OrganizationId) {
    let warning = check(database, now).run(&job(organization)).await.unwrap();
    assert_eq!(warning, None);
}

/// A member of `organization` who opted in, with a confirmed Telegram link and the time zone `zone`.
async fn recipient(test: &TestDatabase, organization: OrganizationId, zone: &str) -> UserId {
    let user = member(test, organization).await;
    test.execute(&[
        &format!(
            "UPDATE app_user SET reminders_opt_in_at = now(), time_zone = '{zone}' WHERE id = '{user}'"
        ),
        &format!(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at)
             VALUES ((SELECT coalesce(max(telegram_user_id), 0) + 1 FROM telegram_identity), '{user}', now())"
        ),
    ])
    .await;
    user
}

async fn member(test: &TestDatabase, organization: OrganizationId) -> UserId {
    let number = Uuid::now_v7().simple();
    let user = test
        .create_user(
            &tada_app::domain::identity::DisplayName::parse(&format!("Member {number}")).unwrap(),
            &tada_app::domain::identity::Email::parse(&format!("member-{number}@example.org"))
                .unwrap(),
        )
        .await;
    test.add_membership(organization, user, OrganizationRole::Member)
        .await;
    user
}

/// Gives `user` the role event viewer in `event`, so the event is in the reach of a plain member (ADR 0052).
async fn view(test: &TestDatabase, organization: OrganizationId, event: EventId, user: UserId) {
    test.execute(&[&format!(
        "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, created_at)
         VALUES ('{organization}', '{event}', '{user}', 'event-viewer', now()) ON CONFLICT DO NOTHING"
    )])
    .await;
}

/// Adds an action of `owner`, who can read the event, and returns its ID.
async fn action(
    test: &TestDatabase,
    organization: OrganizationId,
    event: EventId,
    owner: UserId,
    due: Option<&str>,
    status: &str,
) -> Uuid {
    view(test, organization, event, owner).await;
    let id = Uuid::now_v7();
    let due = due.map_or("NULL".to_owned(), |due| format!("'{due}'"));
    test.execute(&[&format!(
        "INSERT INTO action (id, organization_id, event_id, local_number, title, owner_user_id, due_date, status,
                             version, created_at, updated_at)
         VALUES ('{id}', '{organization}', '{event}',
                 (SELECT coalesce(max(local_number), 0) + 1 FROM action WHERE event_id = '{event}'),
                 'Festzelt aufstellen', '{owner}', {due}, '{status}', 1, now(), now())"
    )])
    .await;
    id
}

/// Adds a firm commitment of `owner`, who can read the event, with a new promisor and returns its ID.
async fn commitment(
    test: &TestDatabase,
    organization: OrganizationId,
    event: EventId,
    owner: UserId,
    due: &str,
) -> Uuid {
    view(test, organization, event, owner).await;
    let id = Uuid::now_v7();
    let person = Uuid::now_v7();
    test.execute(&[
        &format!(
            "INSERT INTO person (id, organization_id, local_number, name, version, created_at, updated_at)
             VALUES ('{person}', '{organization}',
                     (SELECT coalesce(max(local_number), 0) + 1 FROM person WHERE organization_id = '{organization}'),
                     'Erika Muster', 1, now(), now())"
        ),
        &format!(
            "INSERT INTO commitment (id, organization_id, event_id, local_number, text, person_id, owner_user_id,
                                     due_date, status, version, created_at, updated_at)
             VALUES ('{id}', '{organization}', '{event}',
                     (SELECT coalesce(max(local_number), 0) + 1 FROM commitment WHERE event_id = '{event}'),
                     'Liefert die Bänke', '{person}', '{owner}', '{due}', 'firm', 1, now(), now())"
        ),
    ])
    .await;
    id
}

/// The digests of `user`: the intent ID for each, in the order of creation.
async fn digests(test: &TestDatabase, organization: OrganizationId, user: UserId) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM outbound_intent
         WHERE purpose = 'telegram-digest' AND status = 'pending' AND organization_id = $1 AND user_id = $2
         ORDER BY id",
    )
    .bind(organization.as_uuid())
    .bind(user.as_uuid())
    .fetch_all(test.pool())
    .await
    .unwrap()
}

/// The items of a digest: kind, record, version and reason, in the order of their position.
async fn items(test: &TestDatabase, intent: Uuid) -> Vec<(String, Uuid, i64, String)> {
    sqlx::query_as(
        "SELECT record_kind, record_id, record_version, reason FROM digest_item
         WHERE intent_id = $1 ORDER BY position",
    )
    .bind(intent)
    .fetch_all(test.pool())
    .await
    .unwrap()
}

/// The send job of a digest: its organization, its payload version and its start.
async fn send_job(test: &TestDatabase, intent: Uuid) -> (Uuid, i32, Timestamp) {
    let row: (Uuid, i32, jiff_sqlx::Timestamp) = sqlx::query_as(
        "SELECT organization_id, version, run_at FROM job
         WHERE kind = $1 AND payload->>'intent_id' = $2::text",
    )
    .bind(SEND_DIGEST_JOB)
    .bind(intent)
    .fetch_one(test.pool())
    .await
    .unwrap();
    (row.0, row.1, row.2.to_jiff())
}

async fn count(test: &TestDatabase, sql: &str) -> i64 {
    test.scalar(sql).await
}

#[tokio::test]
async fn check_plans_one_digest_per_owner_across_events() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let fest = test.create_event(organization, "FEST").await;
    let markt = test.create_event(organization, "MARKT").await;
    let anna = recipient(&test, organization, "Europe/Zurich").await;
    let ben = recipient(&test, organization, "Europe/Zurich").await;

    let overdue = action(&test, organization, fest, anna, Some("2026-10-12"), "open").await;
    let blocked = action(&test, organization, markt, anna, None, "blocked").await;
    let due_soon = commitment(&test, organization, markt, anna, "2026-10-16").await;
    // Outside the look-ahead of 2 working days, closed, or reminded yesterday without an answer.
    action(&test, organization, fest, anna, Some("2026-10-19"), "open").await;
    action(&test, organization, fest, anna, Some("2026-10-12"), "done").await;
    let reminded = action(
        &test,
        organization,
        fest,
        anna,
        Some("2026-10-15"),
        "in-progress",
    )
    .await;
    test.execute(&[&format!(
        "INSERT INTO reminder_state (organization_id, record_kind, record_id, user_id, notified_version, notified_at)
         VALUES ('{organization}', 'action', '{reminded}', '{anna}', 1, '2026-10-13T06:30:00Z')"
    )])
    .await;
    let bens = action(&test, organization, markt, ben, Some("2026-10-14"), "open").await;
    // The action of a closed event is not work any more.
    let closed = test.create_event(organization, "ALT").await;
    action(&test, organization, closed, ben, Some("2026-10-01"), "open").await;
    test.execute(&[&format!(
        "UPDATE event SET status = 'closed', closed_at = now() WHERE id = '{closed}'"
    )])
    .await;

    run(&test.database, ts(WEDNESDAY), organization).await;

    let anna_digests = digests(&test, organization, anna).await;
    assert_eq!(anna_digests.len(), 1, "one digest across both events");
    assert_eq!(
        items(&test, anna_digests[0]).await,
        vec![
            ("action".to_owned(), overdue, 1, "overdue".to_owned()),
            ("commitment".to_owned(), due_soon, 1, "due-soon".to_owned()),
            ("action".to_owned(), blocked, 1, "blocked".to_owned()),
        ]
    );
    let ben_digests = digests(&test, organization, ben).await;
    assert_eq!(ben_digests.len(), 1);
    assert_eq!(
        items(&test, ben_digests[0]).await,
        vec![("action".to_owned(), bens, 1, "due-soon".to_owned())]
    );
    for intent in [anna_digests[0], ben_digests[0]] {
        assert_eq!(
            send_job(&test, intent).await,
            (organization.as_uuid(), 1, ts(WEDNESDAY)),
            "outside the quiet hours the send starts now"
        );
        let message_id: String = test
            .scalar(&format!(
                "SELECT message_id FROM outbound_intent WHERE id = '{intent}'"
            ))
            .await;
        assert_eq!(message_id, intent.simple().to_string());
    }
}

#[tokio::test]
async fn check_skips_not_opted_in_unlinked_blocked_and_removed() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "FEST").await;
    let eligible = recipient(&test, organization, "Europe/Zurich").await;
    let not_opted_in = recipient(&test, organization, "Europe/Zurich").await;
    let unlinked = recipient(&test, organization, "Europe/Zurich").await;
    let blocked = recipient(&test, organization, "Europe/Zurich").await;
    let removed = recipient(&test, organization, "Europe/Zurich").await;
    for user in [eligible, not_opted_in, unlinked, blocked, removed] {
        action(&test, organization, event, user, Some("2026-10-13"), "open").await;
    }
    test.execute(&[
        &format!("UPDATE app_user SET reminders_opt_in_at = NULL WHERE id = '{not_opted_in}'"),
        &format!("DELETE FROM telegram_identity WHERE user_id = '{unlinked}'"),
        &format!("UPDATE telegram_identity SET blocked_at = now() WHERE user_id = '{blocked}'"),
        &format!("DELETE FROM organization_membership WHERE user_id = '{removed}'"),
    ])
    .await;

    run(&test.database, ts(WEDNESDAY), organization).await;

    assert_eq!(digests(&test, organization, eligible).await.len(), 1);
    assert_eq!(
        count(
            &test,
            "SELECT count(*) FROM outbound_intent WHERE purpose = 'telegram-digest'"
        )
        .await,
        1,
        "only the eligible recipient gets a digest"
    );
}

#[tokio::test]
async fn check_respects_cooldown_after_restart() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "FEST").await;
    let anna = recipient(&test, organization, "Europe/Zurich").await;
    let overdue = action(&test, organization, event, anna, Some("2026-10-12"), "open").await;

    run(&test.database, ts(WEDNESDAY), organization).await;
    let first = digests(&test, organization, anna).await;
    assert_eq!(first.len(), 1);
    // A retry of the same check plans nothing new: the digest waits for its send.
    run(&test.database, ts(WEDNESDAY), organization).await;
    assert_eq!(digests(&test, organization, anna).await, first);
    // The digest went out; then the record changed, so the rules would remind again.
    test.execute(&[
        &format!(
            "UPDATE outbound_intent SET status = 'sent', finished_at = '{WEDNESDAY}' WHERE id = '{}'",
            first[0]
        ),
        &format!("UPDATE action SET version = 2 WHERE id = '{overdue}'"),
    ])
    .await;

    // A new worker process with a new pool, 19 hours later: still inside the cooldown of 20 hours.
    let restarted = test.connect();
    let later = ts(WEDNESDAY) + jiff::SignedDuration::from_hours(19);
    run(&restarted, later, organization).await;
    let planned: i64 = count(
        &test,
        "SELECT count(*) FROM outbound_intent WHERE purpose = 'telegram-digest'",
    )
    .await;
    assert_eq!(planned, 1, "no digest inside the cooldown");

    // After the cooldown the changed record is in a new digest.
    let after = ts(WEDNESDAY) + jiff::SignedDuration::from_hours(21);
    run(&restarted, after, organization).await;
    let second = digests(&test, organization, anna).await;
    assert_eq!(second.len(), 1);
    assert_eq!(
        items(&test, second[0]).await,
        vec![("action".to_owned(), overdue, 2, "overdue".to_owned())]
    );
}

#[tokio::test]
async fn cooldown_starts_at_the_planning_time_not_at_the_send() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "FEST").await;
    let new_york = recipient(&test, organization, "America/New_York").await;
    action(
        &test,
        organization,
        event,
        new_york,
        Some("2026-10-13"),
        "open",
    )
    .await;
    // 02:30 in New York: the send waits for 08:00 there, 12:00Z.
    let wednesday = ts("2026-10-14T06:30:00Z");
    run(&test.database, wednesday, organization).await;
    let first = digests(&test, organization, new_york).await;
    test.execute(&[&format!(
        "UPDATE outbound_intent SET status = 'sent', finished_at = '2026-10-14T12:00:00Z' WHERE id = '{}'",
        first[0]
    )])
    .await;

    // The check of the next day is 18.5 hours after the send, but 24 hours after the planning.
    run(&test.database, ts("2026-10-15T06:30:00Z"), organization).await;

    assert_eq!(
        digests(&test, organization, new_york).await.len(),
        1,
        "the recipient gets the digest of each working day"
    );
}

#[tokio::test]
async fn owner_without_access_to_the_event_gets_no_item_of_it() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let fest = test.create_event(organization, "FEST").await;
    let markt = test.create_event(organization, "MARKT").await;
    let anna = recipient(&test, organization, "Europe/Zurich").await;
    action(&test, organization, fest, anna, Some("2026-10-12"), "open").await;
    let kept = action(&test, organization, markt, anna, Some("2026-10-12"), "open").await;
    // Anna left FEST but is still a member of the organization; she keeps the ownership (ADR 0063).
    test.execute(&[&format!(
        "DELETE FROM event_membership WHERE event_id = '{fest}' AND user_id = '{anna}'"
    )])
    .await;
    // An admin reads each event without an event role.
    let admin = recipient(&test, organization, "Europe/Zurich").await;
    let admins = action(&test, organization, fest, admin, Some("2026-10-12"), "open").await;
    test.execute(&[
        &format!("UPDATE organization_membership SET role = 'admin' WHERE user_id = '{admin}'"),
        &format!("DELETE FROM event_membership WHERE user_id = '{admin}'"),
    ])
    .await;

    run(&test.database, ts(WEDNESDAY), organization).await;

    let planned = digests(&test, organization, anna).await;
    assert_eq!(
        items(&test, planned[0]).await,
        vec![("action".to_owned(), kept, 1, "overdue".to_owned())],
        "no item of an event that the owner cannot read"
    );
    let planned = digests(&test, organization, admin).await;
    assert_eq!(
        items(&test, planned[0]).await,
        vec![("action".to_owned(), admins, 1, "overdue".to_owned())]
    );
}

#[tokio::test]
async fn check_sets_send_at_to_quiet_end() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "FEST").await;
    let zurich = recipient(&test, organization, "Europe/Zurich").await;
    let new_york = recipient(&test, organization, "America/New_York").await;
    for user in [zurich, new_york] {
        action(&test, organization, event, user, Some("2026-10-13"), "open").await;
    }
    // 08:30 in Zurich is 02:30 in New York: inside the quiet hours of 20:00 to 08:00 there.
    let check_time = ts("2026-10-14T06:30:00Z");

    run(&test.database, check_time, organization).await;

    let zurich_digest = digests(&test, organization, zurich).await;
    assert_eq!(send_job(&test, zurich_digest[0]).await.2, check_time);
    let new_york_digest = digests(&test, organization, new_york).await;
    assert_eq!(
        send_job(&test, new_york_digest[0]).await.2,
        ts("2026-10-14T12:00:00Z"),
        "08:00 in New York"
    );
}

#[tokio::test]
async fn paused_policy_plans_nothing() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "FEST").await;
    let anna = recipient(&test, organization, "Europe/Zurich").await;
    action(&test, organization, event, anna, Some("2026-10-12"), "open").await;
    let admin = MemberCaller::new(
        UserId::from_uuid(Uuid::now_v7()),
        organization,
        OrganizationRole::Admin,
    );
    let paused = PmPolicy {
        paused: true,
        ..PmPolicy::default()
    };
    test.database
        .set(
            admin.scope(),
            &paused,
            RecordVersion::FIRST,
            &AuditEvent::new(
                admin.actor(),
                AuditAction::OrganizationSetPmPolicy,
                Some(organization.as_uuid()),
                Some(admin.scope()),
            ),
        )
        .await
        .unwrap()
        .unwrap();

    run(&test.database, ts(WEDNESDAY), organization).await;

    assert_eq!(
        count(&test, "SELECT count(*) FROM outbound_intent").await,
        0
    );
    assert_eq!(
        count(
            &test,
            &format!("SELECT count(*) FROM job WHERE kind = '{SEND_DIGEST_JOB}'")
        )
        .await,
        0
    );
}

#[tokio::test]
async fn other_org_items_never_appear() {
    let test = TestDatabase::start().await;
    let testwil = test.create_organization("testwil").await;
    let musterhausen = test.create_organization("musterhausen").await;
    let fest = test.create_event(testwil, "FEST").await;
    let markt = test.create_event(musterhausen, "MARKT").await;
    // One user in both organizations, with work in both.
    let anna = recipient(&test, testwil, "Europe/Zurich").await;
    test.add_membership(musterhausen, anna, OrganizationRole::Member)
        .await;
    let own = action(&test, testwil, fest, anna, Some("2026-10-12"), "open").await;
    let other = action(&test, musterhausen, markt, anna, Some("2026-10-12"), "open").await;

    run(&test.database, ts(WEDNESDAY), testwil).await;

    let planned = digests(&test, testwil, anna).await;
    assert_eq!(planned.len(), 1);
    assert_eq!(
        items(&test, planned[0]).await,
        vec![("action".to_owned(), own, 1, "overdue".to_owned())]
    );
    assert!(digests(&test, musterhausen, anna).await.is_empty());
    assert_eq!(
        count(
            &test,
            &format!(
                "SELECT count(*) FROM digest_item WHERE record_id = '{other}' OR organization_id <> '{testwil}'"
            )
        )
        .await,
        0,
        "the check of one organization never reads the work of another"
    );
}
