//! The store of the digest send (ADR 0076): the claim, the outcome with its notices, the cancel and the list. Each
//! statement stays in the organization of its scope.

#![allow(clippy::unwrap_used)]

use jiff::Timestamp;
use tada_app::caller::{MemberCaller, OrgScope, OrganizationRole, TestScope};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::ids::{DigestItemId, OrganizationId, UserId};
use tada_app::pm::digests::{DigestLog, DigestStatus};
use tada_app::pm::send::{DigestOutcome, DigestStore};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const AT: &str = "2026-09-16T08:30:00Z";

fn at() -> Timestamp {
    AT.parse().unwrap()
}

fn scope(organization: OrganizationId) -> OrgScope {
    MemberCaller::new(
        UserId::from_uuid(Uuid::now_v7()),
        organization,
        OrganizationRole::Admin,
    )
    .scope()
}

struct World {
    test: TestDatabase,
    organization: OrganizationId,
    anna: UserId,
}

impl World {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let anna = test
            .create_user(
                &DisplayName::parse("Anna Beispiel").unwrap(),
                &Email::parse("anna@example.org").unwrap(),
            )
            .await;
        test.add_membership(organization, anna, OrganizationRole::Member)
            .await;
        test.execute(&[&format!(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at) VALUES (4711, '{anna}', now())"
        )])
        .await;
        Self {
            test,
            organization,
            anna,
        }
    }

    /// A pending digest to Anna with one item for each record, at version 3. Returns the intent and the items.
    async fn digest(&self, records: &[Uuid]) -> (Uuid, Vec<Uuid>) {
        let intent = Uuid::now_v7();
        let (organization, anna) = (self.organization, self.anna);
        self.test
            .execute(&[&format!(
                "INSERT INTO outbound_intent (id, organization_id, user_id, purpose, message_id, created_at)
                 VALUES ('{intent}', '{organization}', '{anna}', 'telegram-digest', '{intent}', now())"
            )])
            .await;
        let mut items = Vec::new();
        for (position, record) in records.iter().enumerate() {
            let item = Uuid::now_v7();
            self.test
                .execute(&[&format!(
                    "INSERT INTO digest_item (id, intent_id, organization_id, record_kind, record_id, record_version,
                                              reason, position)
                     VALUES ('{item}', '{intent}', '{organization}', 'action', '{record}', 3, 'overdue', {position})"
                )])
                .await;
            items.push(item);
        }
        (intent, items)
    }

    async fn status(&self, intent: Uuid) -> String {
        self.test
            .scalar(&format!(
                "SELECT status FROM outbound_intent WHERE id = '{intent}'"
            ))
            .await
    }

    async fn count(&self, sql: &str) -> i64 {
        self.test.scalar(sql).await
    }
}

fn ids(items: &[Uuid]) -> Vec<DigestItemId> {
    items
        .iter()
        .map(|id| DigestItemId::from_uuid(*id))
        .collect()
}

#[tokio::test]
async fn load_and_claim_stay_in_the_organization_and_claim_once() {
    let world = World::start().await;
    let other = world.test.create_organization("musterhausen").await;
    let (intent, items) = world.digest(&[Uuid::now_v7()]).await;
    let database = &world.test.database;
    let own = scope(world.organization);

    assert!(database.load(scope(other), intent).await.unwrap().is_none());
    assert!(!database.claim(scope(other), intent, at()).await.unwrap());
    let pending = database.load(own, intent).await.unwrap().unwrap();
    assert_eq!(pending.user_id, world.anna);
    assert_eq!(pending.locale, "de-CH");
    assert_eq!(
        pending
            .items
            .iter()
            .map(|item| item.id.as_uuid())
            .collect::<Vec<_>>(),
        items
    );

    assert!(database.claim(own, intent, at()).await.unwrap());
    assert!(
        !database.claim(own, intent, at()).await.unwrap(),
        "a second claim"
    );
    assert!(database.load(own, intent).await.unwrap().is_none());
    assert_eq!(world.status(intent).await, "unknown");
    // A release after a send that delivered nothing makes it pending again, and only in its organization.
    database.release(scope(other), intent).await.unwrap();
    assert_eq!(world.status(intent).await, "unknown");
    database.release(own, intent).await.unwrap();
    assert!(database.load(own, intent).await.unwrap().is_some());
    assert!(database.claim(own, intent, at()).await.unwrap());
}

#[tokio::test]
async fn finish_keeps_the_shown_items_and_writes_their_notices() {
    let world = World::start().await;
    let (shown, hidden) = (Uuid::now_v7(), Uuid::now_v7());
    let (intent, items) = world.digest(&[shown, hidden]).await;
    let (organization, anna) = (world.organization, world.anna);
    // An older notice of the shown record, with an answer to that version.
    world
        .test
        .execute(&[&format!(
            "INSERT INTO reminder_state (organization_id, record_kind, record_id, user_id, notified_version,
                                         notified_at, response, response_version, response_at)
             VALUES ('{organization}', 'action', '{shown}', '{anna}', 1, now(), 'on-track', 1, now())"
        )])
        .await;
    let database = &world.test.database;
    let own = scope(organization);
    assert!(database.claim(own, intent, at()).await.unwrap());

    database
        .finish(own, intent, DigestOutcome::Sent, &ids(&items[..1]), at())
        .await
        .unwrap();
    // A second finish of the same digest changes nothing.
    database
        .finish(own, intent, DigestOutcome::Failed, &[], at())
        .await
        .unwrap();

    assert_eq!(world.status(intent).await, "sent");
    assert_eq!(
        world
            .count(&format!(
                "SELECT count(*) FROM digest_item WHERE intent_id = '{intent}'"
            ))
            .await,
        1
    );
    assert_eq!(
        world
            .count(&format!(
                "SELECT count(*) FROM reminder_state WHERE record_id = '{shown}' AND notified_version = 3
                   AND notified_at = '{AT}' AND response = 'on-track' AND response_version = 1"
            ))
            .await,
        1,
        "the new notice keeps the answer to the old version"
    );
    assert_eq!(
        world
            .count(&format!(
                "SELECT count(*) FROM reminder_state WHERE record_id = '{hidden}'"
            ))
            .await,
        0
    );
}

#[tokio::test]
async fn blocked_finish_marks_the_link_writes_no_notice_and_cancel_needs_a_pending_digest() {
    let world = World::start().await;
    let database = &world.test.database;
    let own = scope(world.organization);
    let (failed, items) = world.digest(&[Uuid::now_v7()]).await;
    assert!(database.claim(own, failed, at()).await.unwrap());
    database
        .finish(own, failed, DigestOutcome::Blocked, &ids(&items), at())
        .await
        .unwrap();
    assert_eq!(world.status(failed).await, "failed");
    assert_eq!(world.count("SELECT count(*) FROM reminder_state").await, 0);
    assert_eq!(
        world
            .count("SELECT count(*) FROM telegram_identity WHERE blocked_at IS NOT NULL")
            .await,
        1
    );

    // A cancel changes a pending digest only.
    database.cancel(own, failed, at()).await.unwrap();
    assert_eq!(world.status(failed).await, "failed");
    let (pending, _) = world.digest(&[Uuid::now_v7()]).await;
    database.cancel(own, pending, at()).await.unwrap();
    assert_eq!(world.status(pending).await, "canceled");
}

#[tokio::test]
async fn recent_lists_the_newest_digests_of_the_organization() {
    let world = World::start().await;
    let other = world.test.create_organization("musterhausen").await;
    let own = scope(world.organization);
    let database = &world.test.database;
    let (first, items) = world.digest(&[Uuid::now_v7(), Uuid::now_v7()]).await;
    assert!(database.claim(own, first, at()).await.unwrap());
    database
        .finish(own, first, DigestOutcome::Sent, &ids(&items), at())
        .await
        .unwrap();
    let (second, _) = world.digest(&[Uuid::now_v7()]).await;
    world
        .test
        .execute(&[&format!(
            "UPDATE outbound_intent SET created_at = created_at + interval '1 day' WHERE id = '{second}'"
        )])
        .await;

    let recent = database.recent(own, 50).await.unwrap();
    assert_eq!(
        recent
            .iter()
            .map(|digest| (digest.id, digest.status, digest.item_count))
            .collect::<Vec<_>>(),
        vec![
            (second, DigestStatus::Pending, 1),
            (first, DigestStatus::Sent, 2),
        ]
    );
    assert_eq!(recent[1].sent_at, Some(at()));
    assert_eq!(recent[1].user_display_name, "Anna Beispiel");
    assert_eq!(database.recent(own, 1).await.unwrap().len(), 1);
    assert!(database.recent(scope(other), 50).await.unwrap().is_empty());
}
