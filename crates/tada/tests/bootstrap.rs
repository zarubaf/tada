//! `tada bootstrap`: the organization and the invitation of its first owner (ADR 0036).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use jiff::{SignedDuration, Timestamp};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use tada::bootstrap::{BootstrapCommand, PrintLinkRefused, execute, link_output};
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_app::bootstrap::BootstrapOutcome;
use tada_app::caller::{Bootstrap, ServiceIdentity};
use tada_app::clock::Clock;
use tada_app::domain::identity::{
    DisplayName, Email, OrganizationName, OrganizationRole, OrganizationSlug,
};
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SendOutbound;
use tada_app::public_url::PublicUrl;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const NOW: Timestamp = Timestamp::constant(1_800_000_000, 0);
const LINK: &str = "https://tada.example.org/invitation#token=";

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        NOW
    }
}

fn command() -> BootstrapCommand {
    BootstrapCommand {
        organization_slug: OrganizationSlug::parse("testwil").unwrap(),
        organization_name: OrganizationName::parse("Open Day Testwil").unwrap(),
        owner_email: Email::parse("owner@example.org").unwrap(),
        print_link: false,
    }
}

fn public_url() -> PublicUrl {
    PublicUrl::parse("https://tada.example.org").unwrap()
}

async fn run(test: &TestDatabase, link: Option<&mut dyn Write>) -> BootstrapOutcome {
    execute(&test.database, &public_url(), &FixedClock, command(), link)
        .await
        .unwrap()
}

async fn count(test: &TestDatabase, sql: &str) -> i64 {
    test.scalar(sql).await
}

/// The number of rows of each table that bootstrap writes.
async fn counts(test: &TestDatabase) -> [i64; 6] {
    [
        count(test, "SELECT count(*) FROM organization").await,
        count(test, "SELECT count(*) FROM invitation").await,
        count(
            test,
            "SELECT count(*) FROM invitation WHERE status = 'pending'",
        )
        .await,
        count(test, "SELECT count(*) FROM outbound_intent").await,
        count(test, "SELECT count(*) FROM job").await,
        count(test, "SELECT count(*) FROM audit_event").await,
    ]
}

#[tokio::test]
async fn a_new_slug_creates_the_organization_and_queues_one_owner_invitation() {
    let test = TestDatabase::start().await;
    let BootstrapOutcome::InvitationQueued {
        invitation_id,
        organization_id,
    } = run(&test, None).await
    else {
        panic!("no invitation was queued");
    };

    assert_eq!(counts(&test).await, [1, 1, 1, 1, 1, 2]);
    let organization: String = test
        .scalar(&format!(
            "SELECT slug || ' ' || name FROM organization WHERE id = '{organization_id}'"
        ))
        .await;
    assert_eq!(organization, "testwil Open Day Testwil");
    let invitation: String = test
        .scalar(&format!(
            "SELECT concat_ws(' ', email, display_name, role, status, invited_by)
             FROM invitation WHERE id = '{invitation_id}' AND organization_id = '{organization_id}'"
        ))
        .await;
    assert_eq!(invitation, "owner@example.org owner owner pending");
    let intent: Uuid = test
        .scalar("SELECT invitation_id FROM outbound_intent WHERE purpose = 'invitation'")
        .await;
    assert_eq!(intent, invitation_id.as_uuid());
    let audit: String = test
        .scalar(&format!(
            "SELECT string_agg(concat_ws(' ', actor_kind, actor_id, channel, action), ', ' ORDER BY action)
             FROM audit_event WHERE organization_id = '{organization_id}'"
        ))
        .await;
    assert_eq!(
        audit,
        format!(
            "service {id} cli invitation.create, service {id} cli organization.create",
            id = Bootstrap::ID
        )
    );
}

#[tokio::test]
async fn a_second_run_revokes_the_first_invitation_and_queues_a_new_one() {
    let test = TestDatabase::start().await;
    let BootstrapOutcome::InvitationQueued {
        invitation_id: first,
        ..
    } = run(&test, None).await
    else {
        panic!("no invitation was queued");
    };
    let BootstrapOutcome::InvitationQueued {
        invitation_id: second,
        ..
    } = run(&test, None).await
    else {
        panic!("no invitation was queued");
    };

    assert_ne!(first, second);
    assert_eq!(counts(&test).await, [1, 2, 1, 2, 2, 3]);
    let status: String = test
        .scalar(&format!(
            "SELECT status FROM invitation WHERE id = '{first}'"
        ))
        .await;
    assert_eq!(status, "revoked");
}

#[tokio::test]
async fn an_organization_with_an_owner_changes_nothing() {
    let test = TestDatabase::start().await;
    let BootstrapOutcome::InvitationQueued {
        organization_id, ..
    } = run(&test, None).await
    else {
        panic!("no invitation was queued");
    };
    let owner = test
        .create_user(
            &DisplayName::parse("Anna Muster").unwrap(),
            &Email::parse("anna@example.org").unwrap(),
        )
        .await;
    test.add_membership(organization_id, owner, OrganizationRole::Owner)
        .await;
    let before = counts(&test).await;

    assert_eq!(run(&test, None).await, BootstrapOutcome::OwnerExists);
    assert_eq!(counts(&test).await, before);
    let mut link = Vec::new();
    assert_eq!(
        run(&test, Some(&mut link as &mut dyn Write)).await,
        BootstrapOutcome::OwnerExists
    );
    assert!(link.is_empty(), "no link without an invitation");
}

#[test]
fn print_link_refuses_when_standard_error_is_not_a_terminal() {
    assert_eq!(link_output(true, false), Err(PrintLinkRefused));
    assert_eq!(link_output(true, true), Ok(true));
    assert_eq!(link_output(false, false), Ok(false));
    assert_eq!(link_output(false, true), Ok(false));
}

#[tokio::test]
async fn the_printed_link_expires_after_30_minutes() {
    let test = TestDatabase::start().await;
    let mut output = Vec::new();
    let BootstrapOutcome::InvitationQueued { invitation_id, .. } =
        run(&test, Some(&mut output as &mut dyn Write)).await
    else {
        panic!("no invitation was queued");
    };

    let output = String::from_utf8(output).unwrap();
    let token = output.trim_end().strip_prefix(LINK).unwrap();
    assert_eq!(token.len(), 43, "256 bits in Base64: {output:?}");
    test.assert_no_plaintext(token).await;
    let invitation: Uuid = test
        .scalar("SELECT invitation_id FROM invitation_token")
        .await;
    assert_eq!(invitation, invitation_id.as_uuid());
    let expires_at: i64 = test
        .scalar("SELECT extract(epoch FROM expires_at)::bigint FROM invitation_token")
        .await;
    assert_eq!(
        expires_at,
        (NOW + SignedDuration::from_mins(30)).as_second()
    );
}

#[tokio::test]
async fn the_worker_sends_the_invitation_of_the_first_owner_only() {
    let test = TestDatabase::start().await;
    run(&test, None).await;
    run(&test, None).await;
    let mailer = Arc::new(MemoryMailer::new());
    let handlers = Handlers::default().with(Arc::new(SendOutbound::new(
        Arc::new(test.database.clone()),
        mailer.clone(),
        Arc::new(FluentMailTexts::new().unwrap()),
        Arc::new(FixedClock),
        public_url(),
    )));
    let worker = Uuid::now_v7();
    let lease = Duration::from_secs(60);
    for _ in 0..2 {
        let ran = run_next(&test.database, &handlers, worker, lease).await;
        assert!(matches!(
            ran,
            Ok(Ran::Completed(_) | Ran::CompletedWithWarning(..))
        ));
    }

    let sent = mailer.sent();
    assert_eq!(sent.len(), 1, "the revoked invitation sends nothing");
    assert_eq!(sent[0].to, Email::parse("owner@example.org").unwrap());
    assert!(sent[0].text.contains(LINK));
}
