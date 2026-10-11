//! The durable schedules and the reminder policy (ADR 0076): the scheduler step starts each run once,
//! and a policy change moves the schedule of the daily check in its own transaction.

#![allow(clippy::unwrap_used)]

use jiff::Timestamp;
use jiff::civil::{Time, Weekday};
use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::caller::TestScope;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::domain::reminders::PmPolicy;
use tada_app::jobs::{ScheduleRunner, ScheduleStep};
use tada_app::pm::PM_CHECK_JOB;
use tada_app::pm::policy::PmPolicyStore;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

fn owner(organization: OrganizationId) -> MemberCaller {
    MemberCaller::new(
        UserId::from_uuid(Uuid::now_v7()),
        organization,
        OrganizationRole::Owner,
    )
}

fn audit(caller: &MemberCaller) -> AuditEvent {
    let scope = caller.scope();
    AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationSetPmPolicy,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    )
}

/// The defaults, with another time, zone and days.
fn saturday_evening() -> PmPolicy {
    PmPolicy::new(
        Time::constant(18, 15, 0, 0),
        "America/New_York",
        1 << 5,
        Time::constant(20, 0, 0, 0),
        Time::constant(8, 0, 0, 0),
        20,
        2,
        2,
        false,
        false,
    )
    .unwrap()
}

/// An organization whose policy has the defaults and is stored, so it has a schedule.
async fn organization_with_schedule(test: &TestDatabase, slug: &str) -> OrganizationId {
    let organization = test.create_organization(slug).await;
    let caller = owner(organization);
    let version = test
        .database
        .set(
            caller.scope(),
            &PmPolicy::default(),
            RecordVersion::FIRST,
            &audit(&caller),
        )
        .await
        .unwrap();
    assert_eq!(version, RecordVersion::new(2));
    organization
}

/// The schedule of the daily check: local time, time zone, weekdays, next run and last run.
async fn schedule(
    test: &TestDatabase,
    organization: OrganizationId,
) -> (Time, String, i16, Timestamp, Option<Timestamp>) {
    let row: (
        jiff_sqlx::Time,
        String,
        i16,
        jiff_sqlx::Timestamp,
        Option<jiff_sqlx::Timestamp>,
    ) = sqlx::query_as(
        "SELECT local_time, time_zone, weekdays, next_run_at, last_run_at
         FROM schedule WHERE organization_id = $1 AND kind = $2",
    )
    .bind(organization.as_uuid())
    .bind(PM_CHECK_JOB)
    .fetch_one(test.pool())
    .await
    .unwrap();
    (
        row.0.to_jiff(),
        row.1,
        row.2,
        row.3.to_jiff(),
        row.4.map(|at| at.to_jiff()),
    )
}

async fn make_due(test: &TestDatabase, interval: &str) {
    test.execute(&[&format!(
        "UPDATE schedule SET next_run_at = now() - interval '{interval}'"
    )])
    .await;
}

async fn jobs(test: &TestDatabase) -> i64 {
    test.scalar(&format!(
        "SELECT count(*) FROM job WHERE kind = '{PM_CHECK_JOB}'"
    ))
    .await
}

#[tokio::test]
async fn two_runners_never_start_the_same_run() {
    let test = TestDatabase::start().await;
    let mut organizations = Vec::new();
    for number in 0..12 {
        organizations.push(organization_with_schedule(&test, &format!("club-{number}")).await);
    }
    make_due(&test, "1 minute").await;
    let (first, second) = (test.connect(), test.connect());
    let (a, b) = tokio::join!(first.start_due(), second.start_due());
    assert_eq!(a.unwrap().started + b.unwrap().started, 12);
    // Each organization got one job, and the job names it.
    let per_organization: Vec<(Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT organization_id, count(*) FROM job WHERE kind = $1 GROUP BY organization_id",
    )
    .bind(PM_CHECK_JOB)
    .fetch_all(test.pool())
    .await
    .unwrap();
    assert_eq!(per_organization.len(), 12);
    for (organization, count) in per_organization {
        assert_eq!(count, 1);
        assert!(organizations.contains(&OrganizationId::from_uuid(organization.unwrap())));
    }
    // The runs moved on, so another step starts nothing.
    assert_eq!(first.start_due().await.unwrap(), ScheduleStep::default());
    assert_eq!(jobs(&test).await, 12);
}

#[tokio::test]
async fn missed_runs_start_once() {
    let test = TestDatabase::start().await;
    let organization = organization_with_schedule(&test, "testwil").await;
    let (_, _, _, planned, last) = schedule(&test, organization).await;
    assert!(planned > Timestamp::now());
    assert_eq!(last, None);
    // The worker was down for three weeks.
    make_due(&test, "21 days").await;
    assert_eq!(test.database.start_due().await.unwrap().started, 1);
    let now = Timestamp::now();
    assert_eq!(jobs(&test).await, 1);
    let (_, _, _, next, last) = schedule(&test, organization).await;
    assert!(next > now, "{next} must be after {now}");
    assert!(last.is_some_and(|last| last <= now));
    assert_eq!(test.database.start_due().await.unwrap().started, 0);
    assert_eq!(jobs(&test).await, 1);
}

#[tokio::test]
async fn policy_change_moves_the_schedule() {
    let test = TestDatabase::start().await;
    let organization = organization_with_schedule(&test, "testwil").await;
    let (time, zone, weekdays, _, _) = schedule(&test, organization).await;
    assert_eq!(
        (time, zone.as_str(), weekdays),
        (Time::constant(8, 30, 0, 0), "Europe/Zurich", 0b001_1111)
    );
    let caller = owner(organization);
    let before = Timestamp::now();
    let version = test
        .database
        .set(
            caller.scope(),
            &saturday_evening(),
            RecordVersion::new(2).unwrap(),
            &audit(&caller),
        )
        .await
        .unwrap();
    assert_eq!(version, RecordVersion::new(3));
    let (time, zone, weekdays, next, _) = schedule(&test, organization).await;
    assert_eq!(
        (time, zone.as_str(), weekdays),
        (Time::constant(18, 15, 0, 0), "America/New_York", 1 << 5)
    );
    let local = next.in_tz("America/New_York").unwrap();
    assert_eq!(local.weekday(), Weekday::Saturday);
    assert_eq!(local.time(), Time::constant(18, 15, 0, 0));
    assert!(next > before);
    assert!(next < before + jiff::SignedDuration::from_hours(24 * 8));
    // The policy reads back with its new version, and there is still one schedule.
    let (policy, read) = test.database.get(caller.scope()).await.unwrap();
    assert_eq!((policy, read), (saturday_evening(), version.unwrap()));
    let count: i64 = test.scalar("SELECT count(*) FROM schedule").await;
    assert_eq!(count, 1);
    let audits: i64 = test
        .scalar("SELECT count(*) FROM audit_event WHERE action = 'organization.set_pm_policy'")
        .await;
    assert_eq!(audits, 2);
}

#[tokio::test]
async fn rolled_back_policy_leaves_schedule() {
    let test = TestDatabase::start().await;
    let organization = organization_with_schedule(&test, "testwil").await;
    let before = schedule(&test, organization).await;
    let caller = owner(organization);

    // A stale version changes nothing.
    let stale = test
        .database
        .set(
            caller.scope(),
            &saturday_evening(),
            RecordVersion::FIRST,
            &audit(&caller),
        )
        .await
        .unwrap();
    assert_eq!(stale, None);
    assert_eq!(schedule(&test, organization).await, before);

    // An audit row that cannot be written rolls back the policy and the schedule with it.
    let elsewhere = owner(OrganizationId::from_uuid(Uuid::now_v7()));
    let failed = test
        .database
        .set(
            caller.scope(),
            &saturday_evening(),
            RecordVersion::new(2).unwrap(),
            &audit(&elsewhere),
        )
        .await;
    assert!(failed.is_err());
    assert_eq!(schedule(&test, organization).await, before);
    let (policy, version) = test.database.get(caller.scope()).await.unwrap();
    assert_eq!(
        (policy, version),
        (PmPolicy::default(), RecordVersion::new(2).unwrap())
    );
}

#[tokio::test]
async fn an_organization_without_a_policy_gets_the_default_schedule() {
    let test = TestDatabase::start().await;
    let saved = organization_with_schedule(&test, "saved").await;
    let caller = owner(saved);
    test.database
        .set(
            caller.scope(),
            &saturday_evening(),
            RecordVersion::new(2).unwrap(),
            &audit(&caller),
        )
        .await
        .unwrap()
        .unwrap();
    let saved_schedule = schedule(&test, saved).await;
    let fresh = test.create_organization("testwil").await;
    let (policy, version) = test.database.get(owner(fresh).scope()).await.unwrap();
    assert_eq!(
        (policy, version),
        (PmPolicy::default(), RecordVersion::FIRST)
    );

    assert_eq!(test.database.ensure_default_schedules().await.unwrap(), 1);
    let (time, zone, weekdays, next, last) = schedule(&test, fresh).await;
    assert_eq!(
        (time, zone.as_str(), weekdays, last),
        (
            Time::constant(8, 30, 0, 0),
            "Europe/Zurich",
            0b001_1111,
            None
        )
    );
    assert!(next > Timestamp::now() - jiff::SignedDuration::from_mins(1));
    let limit: i16 = test
        .scalar(&format!(
            "SELECT missed_run_limit FROM schedule WHERE organization_id = '{fresh}'"
        ))
        .await;
    assert_eq!(limit, 1);
    // The saved policy keeps its schedule, and a second call adds nothing.
    assert_eq!(schedule(&test, saved).await, saved_schedule);
    assert_eq!(test.database.ensure_default_schedules().await.unwrap(), 0);
}

#[tokio::test]
async fn a_bad_row_does_not_block_the_others() {
    let test = TestDatabase::start().await;
    let good = organization_with_schedule(&test, "good").await;
    let bad = organization_with_schedule(&test, "bad").await;
    // The bad row has a kind that this binary does not know, and it comes first.
    test.execute(&[
        &format!(
            "UPDATE schedule SET next_run_at = now() - interval '2 hours', kind = 'later-kind'
             WHERE organization_id = '{bad}'"
        ),
        &format!(
            "UPDATE schedule SET next_run_at = now() - interval '1 hour' WHERE organization_id = '{good}'"
        ),
    ])
    .await;
    let bad_id: Uuid = test
        .scalar(&format!(
            "SELECT id FROM schedule WHERE organization_id = '{bad}'"
        ))
        .await;
    let step = test.database.start_due().await.unwrap();
    assert_eq!(step.started, 1);
    assert_eq!(step.skipped, [(bad_id, "later-kind".to_owned())]);
    let job_organization: Uuid = test
        .scalar(&format!(
            "SELECT organization_id FROM job WHERE kind = '{PM_CHECK_JOB}'"
        ))
        .await;
    assert_eq!(job_organization, good.as_uuid());
    // The bad row moved one day forward and starts nothing now.
    let moved: bool = test
        .scalar(&format!(
            "SELECT next_run_at > now() + interval '23 hours' AND last_run_at IS NULL
             FROM schedule WHERE organization_id = '{bad}'"
        ))
        .await;
    assert!(moved);
    assert_eq!(
        test.database.start_due().await.unwrap(),
        ScheduleStep::default()
    );
}

#[tokio::test]
async fn a_save_with_the_same_time_keeps_the_next_run() {
    let test = TestDatabase::start().await;
    let organization = organization_with_schedule(&test, "testwil").await;
    make_due(&test, "1 minute").await;
    let before = schedule(&test, organization).await;
    let caller = owner(organization);
    let paused = PmPolicy {
        paused: true,
        ..PmPolicy::default()
    };
    test.database
        .set(
            caller.scope(),
            &paused,
            RecordVersion::new(2).unwrap(),
            &audit(&caller),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(schedule(&test, organization).await, before);
}
