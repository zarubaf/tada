//! The housekeeping deletes technical rows after their periods (ADR 0071).
//! Each rule has a row just older and a row just younger than its period.

#![allow(clippy::unwrap_used)]

use jiff::{SignedDuration, Timestamp};
use tada_app::caller::{JobRunner, ServiceCaller};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::ids::UserId;
use tada_app::housekeeping::{HousekeepingReport, HousekeepingStore, LONG_KEEP, SHORT_KEEP};
use tada_app::session::{ABSOLUTE_TIMEOUT, IDLE_TIMEOUT};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const NOW: &str = "2031-06-30T12:00:00Z";
const SECOND: SignedDuration = SignedDuration::from_secs(1);

fn caller() -> ServiceCaller<JobRunner> {
    ServiceCaller::new()
}

fn now() -> Timestamp {
    NOW.parse().unwrap()
}

/// A time `age` before `NOW`.
fn ago(age: SignedDuration) -> Timestamp {
    now().checked_sub(age).unwrap()
}

fn older(period: SignedDuration) -> Timestamp {
    ago(period + SECOND)
}

fn younger(period: SignedDuration) -> Timestamp {
    ago(period - SECOND)
}

async fn run(test: &TestDatabase) -> HousekeepingReport {
    test.database.run(&caller(), now()).await.unwrap()
}

async fn user(test: &TestDatabase, name: &str, email: &str) -> UserId {
    test.create_user(
        &DisplayName::parse(name).unwrap(),
        &Email::parse(email).unwrap(),
    )
    .await
}

async fn count(test: &TestDatabase, table: &str) -> i64 {
    test.scalar(&format!("SELECT count(*) FROM {table}")).await
}

fn token_hash() -> Vec<u8> {
    Uuid::now_v7().as_bytes().repeat(2)
}

fn sql(time: Timestamp) -> jiff_sqlx::Timestamp {
    jiff_sqlx::ToSqlx::to_sqlx(time)
}

#[tokio::test]
async fn expired_magic_links_go_after_30_days() {
    let test = TestDatabase::start().await;
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    for expires_at in [
        older(SHORT_KEEP),
        younger(SHORT_KEEP),
        now().checked_add(SHORT_KEEP).unwrap(),
    ] {
        sqlx::query(
            "INSERT INTO magic_link (token_hash, user_id, expires_at, created_at) VALUES ($1, $2, $3, $3)",
        )
        .bind(token_hash())
        .bind(anna.as_uuid())
        .bind(sql(expires_at))
        .execute(test.pool())
        .await
        .unwrap();
    }

    let report = run(&test).await;

    assert_eq!(report.magic_links, 1);
    assert_eq!(count(&test, "magic_link").await, 2);
}

/// A sign-out or a revocation deletes the session row at once, so a session ends here by its timeouts only.
#[tokio::test]
async fn ended_sessions_go_after_30_days() {
    let test = TestDatabase::start().await;
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    let idle = SHORT_KEEP + IDLE_TIMEOUT;
    let absolute = SHORT_KEEP + ABSOLUTE_TIMEOUT;
    // (created, last used, deleted)
    let sessions = [
        (older(idle), older(idle), true),
        (younger(idle), younger(idle), false),
        (older(absolute), now(), true),
        (younger(absolute), now(), false),
        (ago(SECOND), ago(SECOND), false),
    ];
    for (created_at, last_used_at, _) in sessions {
        sqlx::query(
            "INSERT INTO session (token_hash, user_id, created_at, last_used_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash())
        .bind(anna.as_uuid())
        .bind(sql(created_at))
        .bind(sql(last_used_at))
        .execute(test.pool())
        .await
        .unwrap();
    }

    let report = run(&test).await;

    assert_eq!(report.sessions, 2);
    assert_eq!(count(&test, "session").await, 3);
}

async fn claim(
    test: &TestDatabase,
    organization: Uuid,
    user: UserId,
    expires_at: Timestamp,
    confirmed_at: Option<Timestamp>,
) {
    sqlx::query(
        "INSERT INTO telegram_link_code
             (id, organization_id, user_id, code_hash, expires_at, claimed_by, claimed_name, claimed_at,
              accepted_at, confirmed_at, created_at)
         VALUES ($1, $2, $3, $4, $5, 42, 'Anna', $6, $6, $7, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(organization)
    .bind(user.as_uuid())
    .bind(token_hash())
    .bind(sql(expires_at))
    .bind(sql(ago(LONG_KEEP * 2)))
    .bind(confirmed_at.map(sql))
    .execute(test.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn telegram_claims_go_after_30_days() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await.as_uuid();
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    let future = now().checked_add(SHORT_KEEP).unwrap();
    // An open claim ends when its code expires.
    claim(&test, organization, anna, older(SHORT_KEEP), None).await;
    claim(&test, organization, anna, younger(SHORT_KEEP), None).await;
    // A confirmed claim ends at its confirmation.
    claim(&test, organization, anna, future, Some(older(SHORT_KEEP))).await;
    claim(&test, organization, anna, future, Some(younger(SHORT_KEEP))).await;
    // A code that is still valid stays.
    claim(&test, organization, anna, future, None).await;

    let report = run(&test).await;

    assert_eq!(report.telegram_claims, 2);
    assert_eq!(count(&test, "telegram_link_code").await, 3);
}

async fn invitation(
    test: &TestDatabase,
    organization: Uuid,
    email: &str,
    status: &str,
    finished_at: Option<Timestamp>,
    with_token: bool,
    with_intent: bool,
) {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO invitation
             (id, organization_id, email, display_name, role, status, created_at, accepted_at, revoked_at)
         VALUES ($1, $2, $3, 'Gast', 'member', $4, $5,
                 CASE WHEN $4 = 'accepted' THEN $6 END, CASE WHEN $4 = 'revoked' THEN $6 END)",
    )
    .bind(id)
    .bind(organization)
    .bind(email)
    .bind(status)
    .bind(sql(ago(LONG_KEEP * 2)))
    .bind(finished_at.map(sql))
    .execute(test.pool())
    .await
    .unwrap();
    if with_token {
        sqlx::query(
            "INSERT INTO invitation_token (token_hash, organization_id, invitation_id, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash())
        .bind(organization)
        .bind(id)
        .bind(sql(now()))
        .execute(test.pool())
        .await
        .unwrap();
    }
    if with_intent {
        sqlx::query(
            "INSERT INTO outbound_intent
                 (id, organization_id, invitation_id, purpose, status, message_id, created_at)
             VALUES ($1, $2, $3, 'invitation', 'pending', $1::text, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(organization)
        .bind(id)
        .bind(sql(ago(LONG_KEEP * 2)))
        .execute(test.pool())
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn finished_invitations_go_after_90_days() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await.as_uuid();
    let one = |email| format!("{email}@example.org");
    // The tokens and the intents of an invitation go first, whatever their status.
    invitation(
        &test,
        organization,
        &one("a"),
        "accepted",
        Some(older(LONG_KEEP)),
        true,
        true,
    )
    .await;
    invitation(
        &test,
        organization,
        &one("b"),
        "revoked",
        Some(older(LONG_KEEP)),
        true,
        true,
    )
    .await;
    invitation(
        &test,
        organization,
        &one("c"),
        "accepted",
        Some(younger(LONG_KEEP)),
        true,
        true,
    )
    .await;
    invitation(
        &test,
        organization,
        &one("d"),
        "revoked",
        Some(younger(LONG_KEEP)),
        false,
        false,
    )
    .await;
    // A pending invitation has no end, however old.
    invitation(&test, organization, &one("e"), "pending", None, true, true).await;

    let report = run(&test).await;

    assert_eq!(report.invitations, 2);
    assert_eq!(count(&test, "invitation").await, 3);
    assert_eq!(count(&test, "invitation_token").await, 2);
    assert_eq!(count(&test, "outbound_intent").await, 2);
}

#[tokio::test]
async fn sent_intents_go_after_90_days() {
    let test = TestDatabase::start().await;
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    // (status, finished, deleted)
    let intents = [
        ("sent", Some(older(LONG_KEEP)), true),
        ("failed", Some(older(LONG_KEEP)), true),
        ("sent", Some(younger(LONG_KEEP)), false),
        ("failed", Some(younger(LONG_KEEP)), false),
        // An intent without a known outcome stays for the operator.
        ("unknown", Some(older(LONG_KEEP)), false),
        ("pending", None, false),
    ];
    for (status, finished_at, _) in intents {
        sqlx::query(
            "INSERT INTO outbound_intent (id, user_id, purpose, status, message_id, created_at, finished_at)
             VALUES ($1, $2, 'magic-link', $3, $1::text, $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(anna.as_uuid())
        .bind(status)
        .bind(sql(ago(LONG_KEEP * 2)))
        .bind(finished_at.map(sql))
        .execute(test.pool())
        .await
        .unwrap();
    }

    let report = run(&test).await;

    assert_eq!(report.outbound_intents, 2);
    assert_eq!(count(&test, "outbound_intent").await, 4);
}

/// A digest goes 90 days after its outcome with its items, also with an unknown outcome: it holds IDs only, and
/// no operator check needs it. A digest that is still pending 90 days after its plan goes too. A notice in `reminder_state` goes 90 days after the notification.
#[tokio::test]
async fn housekeeping_removes_old_digests() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    // (status, finished, deleted)
    let digests = [
        ("sent", Some(older(LONG_KEEP)), true),
        ("failed", Some(older(LONG_KEEP)), true),
        ("unknown", Some(older(LONG_KEEP)), true),
        ("canceled", Some(older(LONG_KEEP)), true),
        ("sent", Some(younger(LONG_KEEP)), false),
        ("unknown", Some(younger(LONG_KEEP)), false),
        ("canceled", Some(younger(LONG_KEEP)), false),
        // Planned long ago and never sent: its send job is gone.
        ("pending", None, true),
    ];
    for (status, finished_at, _) in digests {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO outbound_intent (id, organization_id, user_id, purpose, status, message_id, created_at,
                                          finished_at)
             VALUES ($1, $2, $3, 'telegram-digest', $4, $1::text, $5, $6)",
        )
        .bind(id)
        .bind(organization.as_uuid())
        .bind(anna.as_uuid())
        .bind(status)
        .bind(sql(ago(LONG_KEEP * 2)))
        .bind(finished_at.map(sql))
        .execute(test.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO digest_item (id, intent_id, organization_id, record_kind, record_id, record_version,
                                      reason, position)
             VALUES ($1, $2, $3, 'action', $1, 1, 'overdue', 0)",
        )
        .bind(Uuid::now_v7())
        .bind(id)
        .bind(organization.as_uuid())
        .execute(test.pool())
        .await
        .unwrap();
    }
    for notified_at in [older(LONG_KEEP), younger(LONG_KEEP)] {
        sqlx::query(
            "INSERT INTO reminder_state (organization_id, record_kind, record_id, user_id, notified_version,
                                         notified_at, response, response_version, response_at)
             VALUES ($1, 'action', $2, $3, 1, $4, 'on-track', 1, $4)",
        )
        .bind(organization.as_uuid())
        .bind(Uuid::now_v7())
        .bind(anna.as_uuid())
        .bind(sql(notified_at))
        .execute(test.pool())
        .await
        .unwrap();
    }

    let report = run(&test).await;

    assert_eq!(report.digests, 5);
    assert_eq!(report.reminder_states, 1);
    assert_eq!(
        report.outbound_intents, 0,
        "the mail rule leaves the digests to their own rule"
    );
    assert_eq!(count(&test, "outbound_intent").await, 3);
    assert_eq!(
        count(&test, "digest_item").await,
        3,
        "the items go with their digest"
    );
    assert_eq!(count(&test, "reminder_state").await, 1);
}

#[tokio::test]
async fn failed_jobs_go_after_90_days() {
    let test = TestDatabase::start().await;
    for failed_at in [Some(older(LONG_KEEP)), Some(younger(LONG_KEEP)), None] {
        sqlx::query(
            "INSERT INTO job (id, kind, version, payload, run_at, created_at, failed_at)
             VALUES ($1, 'send-outbound', 1, '{}', $2, $2, $3)",
        )
        .bind(Uuid::now_v7())
        .bind(sql(ago(LONG_KEEP * 2)))
        .bind(failed_at.map(sql))
        .execute(test.pool())
        .await
        .unwrap();
    }

    let report = run(&test).await;

    assert_eq!(report.failed_jobs, 1);
    assert_eq!(count(&test, "job").await, 2);
}

async fn end_membership(test: &TestDatabase, user: UserId, ended_at: Timestamp) {
    sqlx::query("UPDATE app_user SET membership_ended_at = $2 WHERE id = $1")
        .bind(user.as_uuid())
        .bind(sql(ended_at))
        .execute(test.pool())
        .await
        .unwrap();
}

/// Gives the account a row in each table that the anonymization empties, and an audit row.
async fn fill_account(test: &TestDatabase, organization: Uuid, user: UserId) {
    let id = user.as_uuid();
    sqlx::query(
        "UPDATE app_user SET reminders_opt_in_at = now(), time_zone = 'Asia/Tokyo' WHERE id = $1",
    )
    .bind(id)
    .execute(test.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at) VALUES ($1, $2, now())")
        .bind(i64::from(id.as_bytes()[15]) + 1000)
        .bind(id)
        .execute(test.pool())
        .await
        .unwrap();
    claim(
        test,
        organization,
        user,
        now().checked_add(SHORT_KEEP).unwrap(),
        None,
    )
    .await;
    sqlx::query("INSERT INTO session (token_hash, user_id, created_at, last_used_at) VALUES ($1, $2, $3, $3)")
        .bind(token_hash())
        .bind(id)
        .bind(sql(now()))
        .execute(test.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO magic_link (token_hash, user_id, expires_at, created_at) VALUES ($1, $2, $3, $3)")
        .bind(token_hash())
        .bind(id)
        .bind(sql(now()))
        .execute(test.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO audit_event (id, organization_id, occurred_at, actor_kind, actor_id, channel, action, record_kind, record_id)
         VALUES ($1, $2, $3, 'member', $4, 'web', 'member.remove', 'member', $4)",
    )
    .bind(Uuid::now_v7())
    .bind(organization)
    .bind(sql(ago(LONG_KEEP * 2)))
    .bind(id)
    .execute(test.pool())
    .await
    .unwrap();
}

/// A person record of the organization that links to `user`.
async fn person_of(test: &TestDatabase, organization: Uuid, number: i64, user: UserId) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO person (id, organization_id, local_number, name, user_id, version, created_at, updated_at)
         VALUES ($1, $2, $3, 'Kontakt', $4, 1, $5, $5)",
    )
    .bind(id)
    .bind(organization)
    .bind(number)
    .bind(user.as_uuid())
    .bind(sql(ago(LONG_KEEP * 2)))
    .execute(test.pool())
    .await
    .unwrap();
    id
}

async fn rows_of(test: &TestDatabase, user: UserId) -> i64 {
    let id = user.as_uuid();
    test.scalar(&format!(
        "SELECT (SELECT count(*) FROM email_identity WHERE user_id = '{id}')
              + (SELECT count(*) FROM telegram_identity WHERE user_id = '{id}')
              + (SELECT count(*) FROM telegram_link_code WHERE user_id = '{id}')
              + (SELECT count(*) FROM session WHERE user_id = '{id}')
              + (SELECT count(*) FROM magic_link WHERE user_id = '{id}')
              + (SELECT count(*) FROM api_token WHERE user_id = '{id}')"
    ))
    .await
}

#[tokio::test]
async fn account_without_membership_anonymized_after_90_days() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let gone = user(&test, "Anna Muster", "anna@example.org").await;
    let recent = user(&test, "Ben Beispiel", "ben@example.org").await;
    for user in [gone, recent] {
        fill_account(&test, organization.as_uuid(), user).await;
    }
    end_membership(&test, gone, older(LONG_KEEP)).await;
    end_membership(&test, recent, younger(LONG_KEEP)).await;
    let gone_person = person_of(&test, organization.as_uuid(), 1, gone).await;
    let recent_person = person_of(&test, organization.as_uuid(), 2, recent).await;

    let report = run(&test).await;

    assert_eq!(report.anonymized_accounts, 1);
    assert_eq!(rows_of(&test, gone).await, 0);
    let name: String = test
        .scalar(&format!(
            "SELECT display_name FROM app_user WHERE id = '{}'",
            gone.as_uuid()
        ))
        .await;
    assert_eq!(name, "entfernt");
    // The reminder settings go with the account; the member who stays keeps them.
    let settings = |user: UserId| {
        format!(
            "SELECT (reminders_opt_in_at IS NULL)::text || ' ' || time_zone FROM app_user WHERE id = '{}'",
            user.as_uuid()
        )
    };
    assert_eq!(
        test.scalar::<String>(&settings(gone)).await,
        "true Europe/Zurich"
    );
    assert_eq!(
        test.scalar::<String>(&settings(recent)).await,
        "false Asia/Tokyo"
    );
    let anonymized: bool = test
        .scalar(&format!(
            "SELECT anonymized_at = '{NOW}'::timestamptz FROM app_user WHERE id = '{}'",
            gone.as_uuid()
        ))
        .await;
    assert!(anonymized);
    // The audit log keeps the ID.
    let audit: i64 = test
        .scalar(&format!(
            "SELECT count(*) FROM audit_event WHERE actor_id = '{}'",
            gone.as_uuid()
        ))
        .await;
    assert_eq!(audit, 1);
    // One audit event for the anonymized account, by the job runner, and none for the other.
    let events: Vec<(String, String, String, Option<Uuid>, Uuid)> = sqlx::query_as(
        "SELECT action, actor_kind, channel, subject_user_id, record_id FROM audit_event
         WHERE action = 'app_user.anonymize'",
    )
    .fetch_all(test.pool())
    .await
    .unwrap();
    assert_eq!(
        events,
        [(
            "app_user.anonymize".to_owned(),
            "service".to_owned(),
            "job".to_owned(),
            Some(gone.as_uuid()),
            gone.as_uuid()
        )]
    );
    // The person record of the account loses the link and keeps its other values.
    let linked = |person: Uuid| format!("SELECT user_id FROM person WHERE id = '{person}'");
    assert_eq!(
        test.scalar::<Option<Uuid>>(&linked(gone_person)).await,
        None
    );
    assert_eq!(
        test.scalar::<Option<Uuid>>(&linked(recent_person)).await,
        Some(recent.as_uuid())
    );
    // The account that ended later keeps all its rows.
    assert_eq!(rows_of(&test, recent).await, 5);
    let kept: String = test
        .scalar(&format!(
            "SELECT display_name FROM app_user WHERE id = '{}'",
            recent.as_uuid()
        ))
        .await;
    assert_eq!(kept, "Ben Beispiel");

    // An anonymized account is not touched again.
    let again = run(&test).await;
    assert_eq!(again.anonymized_accounts, 0);
    assert_eq!(
        count(&test, "audit_event WHERE action = 'app_user.anonymize'").await,
        1
    );
    let still: bool = test
        .scalar(&format!(
            "SELECT anonymized_at = '{NOW}'::timestamptz FROM app_user WHERE id = '{}'",
            gone.as_uuid()
        ))
        .await;
    assert!(still);
}

#[tokio::test]
async fn account_with_membership_kept() {
    let test = TestDatabase::start().await;
    let organization = test.create_organization("testwil").await;
    let anna = user(&test, "Anna Muster", "anna@example.org").await;
    test.add_membership(
        organization,
        anna,
        tada_app::caller::OrganizationRole::Member,
    )
    .await;
    fill_account(&test, organization.as_uuid(), anna).await;
    // The mark must be gone with the membership; a stale mark must still not anonymize a member.
    end_membership(&test, anna, older(LONG_KEEP)).await;

    let report = run(&test).await;

    assert_eq!(report.anonymized_accounts, 0);
    assert_eq!(rows_of(&test, anna).await, 5);
    assert_eq!(
        count(&test, "audit_event WHERE action = 'app_user.anonymize'").await,
        0
    );
}
