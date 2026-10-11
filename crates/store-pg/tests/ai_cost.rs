//! The prices, the usage and the monthly cap of the AI work (ADRs 0047 and 0077).

#![allow(clippy::unwrap_used)]

use tada_app::ai_cost::{AiCostStore, Chf, Month, Price, UsageRecord};
use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::caller::TestScope;
use tada_app::caller::{JobRunner, MemberCaller, OrgScope, ServiceCaller};
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::OrganizationRole;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::model::Usage;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

fn chf(text: &str) -> Chf {
    text.parse().unwrap()
}

fn scope(organization: OrganizationId) -> OrgScope {
    let owner = UserId::from_uuid(Uuid::now_v7());
    MemberCaller::new(owner, organization, OrganizationRole::Owner).scope()
}

fn audit(scope: OrgScope) -> AuditEvent {
    AuditEvent::new(
        ServiceCaller::<JobRunner>::new().actor(),
        AuditAction::OrganizationSetAiCap,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    )
}

fn usage(cost: &str, at: &str) -> UsageRecord {
    UsageRecord {
        event_id: None,
        task: "pm-digest",
        prompt_version: 1,
        model_id: "model-a".to_owned(),
        usage: Usage {
            input_tokens: 1_000,
            output_tokens: 200,
        },
        cost: chf(cost),
        at: at.parse().unwrap(),
    }
}

#[tokio::test]
async fn the_price_of_a_day_is_the_latest_that_started() {
    let test = TestDatabase::start().await;
    let db = test.connect();
    let testwil = scope(test.create_organization("testwil").await);
    test.execute(&[
        "INSERT INTO ai_price VALUES ('model-a', '2026-01-01', 0.8, 4)",
        "INSERT INTO ai_price VALUES ('model-a', '2026-10-01', 0.75, 3.5)",
        // A part of a millirappen rounds up, so that the cost is never too low.
        "INSERT INTO ai_price VALUES ('model-c', '2026-10-01', 0.000001, 0.123456)",
    ])
    .await;

    let price = |on: &str| {
        let db = db.clone();
        let on = on.parse().unwrap();
        async move { db.price(testwil, "model-a", on).await.unwrap() }
    };
    assert_eq!(price("2025-12-31").await, None);
    assert_eq!(
        price("2026-09-30").await,
        Some(Price {
            input_per_mtok: chf("0.8"),
            output_per_mtok: chf("4"),
        })
    );
    assert_eq!(
        price("2026-10-11").await,
        Some(Price {
            input_per_mtok: chf("0.75"),
            output_per_mtok: chf("3.5"),
        })
    );
    assert_eq!(
        db.price(testwil, "model-c", "2026-10-11".parse().unwrap())
            .await
            .unwrap(),
        Some(Price {
            input_per_mtok: chf("0.00001"),
            output_per_mtok: chf("0.12346"),
        })
    );
    assert_eq!(
        db.price(testwil, "model-b", "2026-10-11".parse().unwrap())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn the_month_usage_counts_only_the_organization_and_the_month() {
    let test = TestDatabase::start().await;
    let db = test.connect();
    let testwil = scope(test.create_organization("testwil").await);
    let other = scope(test.create_organization("andere").await);
    let month = Month::containing("2026-10-11T08:30:00Z".parse().unwrap());
    assert_eq!(db.month_usage(testwil, month).await.unwrap(), Chf::ZERO);

    for (scope, cost, at) in [
        (testwil, "0.00125", "2026-10-01T00:00:00Z"),
        (testwil, "0.01", "2026-10-31T23:59:59Z"),
        (testwil, "1", "2026-09-30T23:59:59Z"),
        (testwil, "1", "2026-11-01T00:00:00Z"),
        (other, "1", "2026-10-11T08:30:00Z"),
    ] {
        db.record(scope, &usage(cost, at)).await.unwrap();
    }
    assert_eq!(
        db.month_usage(testwil, month).await.unwrap(),
        chf("0.01125")
    );
    let row: String = test
        .scalar(
            "SELECT format('%s %s %s %s', task, input_tokens, output_tokens, cost_chf) FROM ai_usage
             WHERE at = '2026-10-01T00:00:00Z'",
        )
        .await;
    assert_eq!(row, "pm-digest 1000 200 0.00125");
}

#[tokio::test]
async fn the_first_cap_inserts_and_a_stale_version_conflicts() {
    let test = TestDatabase::start().await;
    let db = test.connect();
    let testwil = scope(test.create_organization("testwil").await);
    assert_eq!(db.cap(testwil).await.unwrap(), None);
    assert!(!db.ai_enabled(testwil).await.unwrap());

    let first = RecordVersion::FIRST;
    let two = db
        .set_cap(testwil, chf("40.5"), first, &audit(testwil))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(two.get(), 2);
    assert_eq!(db.cap(testwil).await.unwrap(), Some((chf("40.5"), two)));

    // The version 1 of the default is stale now.
    assert_eq!(
        db.set_cap(testwil, chf("10"), first, &audit(testwil))
            .await
            .unwrap(),
        None
    );
    let three = db
        .set_cap(testwil, chf("10"), two, &audit(testwil))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(db.cap(testwil).await.unwrap(), Some((chf("10"), three)));

    // A version that never existed conflicts also without a row.
    let other = scope(test.create_organization("andere").await);
    assert_eq!(
        db.set_cap(other, chf("10"), two, &audit(other))
            .await
            .unwrap(),
        None
    );
    let audits: i64 = test
        .scalar("SELECT count(*) FROM audit_event WHERE action = 'organization.set_ai_cap'")
        .await;
    assert_eq!(audits, 2);
}

#[tokio::test]
async fn ai_is_enabled_by_the_policy() {
    let test = TestDatabase::start().await;
    let db = test.connect();
    let organization = test.create_organization("testwil").await;
    test.execute(&[&format!(
        "INSERT INTO pm_policy (organization_id, local_time, time_zone, weekdays, quiet_start, quiet_end,
             cooldown_hours, follow_up_working_days, look_ahead_working_days, ai_enabled)
         VALUES ('{}', '08:30', 'Europe/Zurich', 31, '20:00', '08:00', 20, 2, 2, true)",
        organization.as_uuid()
    )])
    .await;
    assert!(db.ai_enabled(scope(organization)).await.unwrap());
}
