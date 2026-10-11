//! The answer buttons of the AI PM digest (ADR 0076): the gateway gets a press as a `callback_query` update in
//! webhook mode, checks it and changes the record with the capability of the member, or changes nothing.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use tada_adapters::clock::SystemClock;
use tada_app::caller::OrganizationRole;
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::ids::{DigestItemId, EventId, OrganizationId, UserId};
use tada_app::domain::reminders::Answer;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

use support::telegram::{Webhook, private};

/// The invented Telegram accounts: Anna owns the records, Bert is another linked member.
const ANNA: i64 = 4711;
const BERT: i64 = 4712;
/// One organization with one event; Anna and Bert are members and contributors of the event, with a link.
struct World {
    test: TestDatabase,
    organization: OrganizationId,
    event: EventId,
    anna: UserId,
    webhook: Webhook,
}

impl World {
    async fn start() -> Self {
        support::logs::install();
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let event = test.create_event(organization, "FEST").await;
        let anna = Self::linked(&test, organization, event, "Anna Beispiel", ANNA).await;
        Self::linked(&test, organization, event, "Bert Muster", BERT).await;
        let webhook = Webhook::start(&test, Arc::new(SystemClock)).await;
        Self {
            test,
            organization,
            event,
            anna,
            webhook,
        }
    }

    /// Adds a member with the event role `event-contributor` and a confirmed Telegram link.
    async fn linked(
        test: &TestDatabase,
        organization: OrganizationId,
        event: EventId,
        name: &str,
        account: i64,
    ) -> UserId {
        let email = format!(
            "{}@example.org",
            name.split(' ').next().unwrap().to_lowercase()
        );
        let user = test
            .create_user(
                &DisplayName::parse(name).unwrap(),
                &Email::parse(&email).unwrap(),
            )
            .await;
        test.add_membership(organization, user, OrganizationRole::Member)
            .await;
        test.execute(&[
            &format!(
                "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at)
                 VALUES ({account}, '{user}', now())"
            ),
            &format!(
                "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, created_at)
                 VALUES ('{organization}', '{event}', '{user}', 'event-contributor', now())"
            ),
        ])
        .await;
        user
    }

    /// Adds an action of Anna with the version 1 and returns its ID.
    async fn action(&self, status: &str) -> Uuid {
        let id = Uuid::now_v7();
        let (organization, event, anna) = (self.organization, self.event, self.anna);
        self.test
            .execute(&[&format!(
                "INSERT INTO action (id, organization_id, event_id, local_number, title, owner_user_id, due_date,
                                     status, version, created_at, updated_at)
                 VALUES ('{id}', '{organization}', '{event}',
                         (SELECT coalesce(max(local_number), 0) + 1 FROM action WHERE event_id = '{event}'),
                         'Festzelt aufstellen', '{anna}', '2026-09-14', '{status}', 1, now(), now())"
            )])
            .await;
        id
    }

    /// Adds a firm commitment of Anna with the version 1 and returns its ID.
    async fn commitment(&self) -> Uuid {
        let (id, person) = (Uuid::now_v7(), Uuid::now_v7());
        let (organization, event, anna) = (self.organization, self.event, self.anna);
        self.test
            .execute(&[
                &format!(
                    "INSERT INTO person (id, organization_id, local_number, name, version, created_at, updated_at)
                     VALUES ('{person}', '{organization}', 1, 'Erika Muster', 1, now(), now())"
                ),
                &format!(
                    "INSERT INTO commitment (id, organization_id, event_id, local_number, text, person_id,
                                             owner_user_id, due_date, status, version, created_at, updated_at)
                     VALUES ('{id}', '{organization}', '{event}', 1, 'Liefert die Bänke', '{person}', '{anna}',
                             '2026-09-17', 'firm', 1, now(), now())"
                ),
            ])
            .await;
        id
    }

    /// A digest to Anna with one item for the record, sent `age` ago with the outcome `status`, and its notice.
    /// Returns the reference of the item.
    async fn digest(&self, kind: &str, record: Uuid, status: &str, age: &str) -> DigestItemId {
        let (intent, item) = (Uuid::now_v7(), Uuid::now_v7());
        let (organization, anna) = (self.organization, self.anna);
        self.test
            .execute(&[
                &format!(
                    "INSERT INTO outbound_intent (id, organization_id, user_id, purpose, message_id, status,
                                                  created_at, finished_at)
                     VALUES ('{intent}', '{organization}', '{anna}', 'telegram-digest', '{intent}', '{status}',
                             now() - interval '{age}', now() - interval '{age}')"
                ),
                &format!(
                    "INSERT INTO digest_item (id, intent_id, organization_id, record_kind, record_id, record_version,
                                              reason, position)
                     VALUES ('{item}', '{intent}', '{organization}', '{kind}', '{record}', 1, 'overdue', 0)"
                ),
                &format!(
                    "INSERT INTO reminder_state (organization_id, record_kind, record_id, user_id, notified_version,
                                                 notified_at)
                     VALUES ('{organization}', '{kind}', '{record}', '{anna}', 1, now() - interval '{age}')
                     ON CONFLICT DO NOTHING"
                ),
            ])
            .await;
        DigestItemId::from_uuid(item)
    }

    /// The status and the version of a work record.
    async fn record(&self, table: &str, id: Uuid) -> (String, i64) {
        let status = self
            .test
            .scalar(&format!("SELECT status FROM {table} WHERE id = '{id}'"))
            .await;
        let version = self
            .test
            .scalar(&format!("SELECT version FROM {table} WHERE id = '{id}'"))
            .await;
        (status, version)
    }

    /// The response of Anna to the record and its version, or empty texts.
    async fn response(&self, id: Uuid) -> (String, String) {
        let anna = self.anna;
        let row: String = self
            .test
            .scalar(&format!(
                "SELECT coalesce(response, '') || ' ' || coalesce(response_version::text, '')
                 FROM reminder_state WHERE record_id = '{id}' AND user_id = '{anna}'"
            ))
            .await;
        let (response, version) = row.split_once(' ').unwrap();
        (response.to_owned(), version.to_owned())
    }
}

/// The answer to a press that changed nothing.
const OUTDATED: &str = "veraltet";

#[tokio::test]
async fn owner_marks_action_done() {
    let world = World::start().await;
    let done = world.action("open").await;
    let blocked = world.action("in-progress").await;
    let done_item = world.digest("action", done, "sent", "1 hour").await;
    let blocked_item = world.digest("action", blocked, "unknown", "6 days").await;

    let text = world
        .webhook
        .press(ANNA, private(ANNA), Answer::Done, done_item)
        .await;
    assert!(!text.contains(OUTDATED), "{text}");
    // Telegram sends a message older than 48 hours as an inaccessible message: only the chat, and the date 0.
    let inaccessible = json!({"message_id": 5, "date": 0, "chat": private(ANNA)});
    let text = world
        .webhook
        .press_on(ANNA, inaccessible, Answer::Blocked, blocked_item)
        .await;
    assert!(!text.contains(OUTDATED), "{text}");

    assert_eq!(world.record("action", done).await, ("done".into(), 2));
    assert_eq!(world.record("action", blocked).await, ("blocked".into(), 2));
    // The response is for the version that the answer made, so the rules do not remind Anna of her own change.
    assert_eq!(world.response(done).await, ("done".into(), "2".into()));
    assert_eq!(
        world.response(blocked).await,
        ("blocked".into(), "2".into())
    );
    let notified: i64 = world
        .test
        .scalar(&format!(
            "SELECT notified_version FROM reminder_state WHERE record_id = '{blocked}'"
        ))
        .await;
    assert_eq!(notified, 2);
    // The command ran with the capability of Anna: the audit names her.
    let audited: i64 = world
        .test
        .scalar(&format!(
            "SELECT count(*) FROM audit_event WHERE action = 'action.change' AND actor_id = '{}'",
            world.anna
        ))
        .await;
    assert_eq!(audited, 2);
    support::logs::assert_clean(&["4711", "Festzelt", "Anna Beispiel"]);
}

#[tokio::test]
async fn owner_marks_commitment_fulfilled() {
    let world = World::start().await;
    let commitment = world.commitment().await;
    let item = world
        .digest("commitment", commitment, "sent", "1 hour")
        .await;

    let text = world
        .webhook
        .press(ANNA, private(ANNA), Answer::Fulfilled, item)
        .await;

    assert!(!text.contains(OUTDATED), "{text}");
    assert_eq!(
        world.record("commitment", commitment).await,
        ("fulfilled".into(), 2)
    );
    assert_eq!(
        world.response(commitment).await,
        ("fulfilled".into(), "2".into())
    );
}

#[tokio::test]
async fn on_track_records_only() {
    let world = World::start().await;
    let action = world.action("open").await;
    let commitment = world.commitment().await;
    let action_item = world.digest("action", action, "sent", "1 hour").await;
    let commitment_item = world
        .digest("commitment", commitment, "sent", "1 hour")
        .await;

    for item in [action_item, commitment_item] {
        let text = world
            .webhook
            .press(ANNA, private(ANNA), Answer::OnTrack, item)
            .await;
        assert!(!text.contains(OUTDATED), "{text}");
    }

    assert_eq!(world.record("action", action).await, ("open".into(), 1));
    assert_eq!(
        world.record("commitment", commitment).await,
        ("firm".into(), 1)
    );
    assert_eq!(
        world.response(action).await,
        ("on-track".into(), "1".into())
    );
    assert_eq!(
        world.response(commitment).await,
        ("on-track".into(), "1".into())
    );
}

#[tokio::test]
async fn stale_or_foreign_press_changes_nothing() {
    let world = World::start().await;
    let action = world.action("open").await;
    let item = world.digest("action", action, "sent", "1 hour").await;
    let unchanged = || async {
        assert_eq!(world.record("action", action).await, ("open".into(), 1));
        assert_eq!(world.response(action).await, (String::new(), String::new()));
    };

    // Another linked member, in the own private chat and in the chat of Anna.
    for chat in [private(BERT), private(ANNA)] {
        let text = world.webhook.press(BERT, chat, Answer::Done, item).await;
        assert!(text.contains(OUTDATED), "{text}");
        unchanged().await;
    }
    // A Telegram account without a link.
    let text = world
        .webhook
        .press(4799, private(4799), Answer::Done, item)
        .await;
    assert!(text.contains(OUTDATED), "{text}");
    unchanged().await;
    // A forwarded message.
    let forwarded = json!({
        "message_id": 6, "date": 1, "chat": private(ANNA), "text": "Digest",
        "forward_origin": {"type": "hidden_user", "date": 1, "sender_user_name": "Testperson"}
    });
    let text = world
        .webhook
        .press_on(ANNA, forwarded, Answer::Done, item)
        .await;
    assert!(text.contains(OUTDATED), "{text}");
    unchanged().await;
    // A group chat: the gateway answers the query, but says nothing.
    let group = json!({"id": -100_4711, "type": "group", "title": "Testgruppe"});
    let text = world.webhook.press(ANNA, group, Answer::Done, item).await;
    assert!(!text.contains("Danke"), "{text}");
    unchanged().await;
    // An answer that the item does not have, and data that does not decode.
    let text = world
        .webhook
        .press(ANNA, private(ANNA), Answer::Fulfilled, item)
        .await;
    assert!(text.contains(OUTDATED), "{text}");
    let message = json!({"message_id": 5, "date": 1, "chat": private(ANNA), "text": "Digest"});
    let text = world
        .webhook
        .deliver_callback(ANNA, message, "done:kaputt")
        .await;
    assert!(text.contains(OUTDATED), "{text}");
    unchanged().await;
    // A digest after 8 days, and a digest that failed.
    for (status, age) in [("sent", "8 days"), ("failed", "1 hour")] {
        let old = world.digest("action", action, status, age).await;
        let text = world
            .webhook
            .press(ANNA, private(ANNA), Answer::Done, old)
            .await;
        assert!(text.contains(OUTDATED), "{status} {age}: {text}");
        unchanged().await;
    }
    // Anna is a viewer of the event now: she may not change the record any more.
    let (organization, event, anna) = (world.organization, world.event, world.anna);
    world
        .test
        .execute(&[&format!(
            "UPDATE event_membership SET event_role = 'event-viewer'
             WHERE organization_id = '{organization}' AND event_id = '{event}' AND user_id = '{anna}'"
        )])
        .await;
    for answer in [Answer::Done, Answer::OnTrack] {
        let text = world.webhook.press(ANNA, private(ANNA), answer, item).await;
        assert!(text.contains(OUTDATED), "{text}");
        unchanged().await;
    }
    world
        .test
        .execute(&[&format!(
            "UPDATE event_membership SET event_role = 'event-contributor'
             WHERE organization_id = '{organization}' AND event_id = '{event}' AND user_id = '{anna}'"
        )])
        .await;
    // A new version of the record.
    world
        .test
        .execute(&[&format!(
            "UPDATE action SET version = 2 WHERE id = '{action}'"
        )])
        .await;
    for answer in [Answer::Done, Answer::OnTrack] {
        let text = world.webhook.press(ANNA, private(ANNA), answer, item).await;
        assert!(text.contains(OUTDATED), "{text}");
        assert_eq!(world.record("action", action).await, ("open".into(), 2));
        assert_eq!(world.response(action).await, (String::new(), String::new()));
    }
    world
        .test
        .execute(&[&format!(
            "UPDATE action SET version = 1 WHERE id = '{action}'"
        )])
        .await;
    // A closed event takes no writes (ADR 0071).
    world
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'closed', closed_at = now() WHERE id = '{event}'"
        )])
        .await;
    for answer in [Answer::Done, Answer::OnTrack] {
        let text = world.webhook.press(ANNA, private(ANNA), answer, item).await;
        assert!(text.contains(OUTDATED), "{text}");
        unchanged().await;
    }
    world
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'active', closed_at = NULL WHERE id = '{event}'"
        )])
        .await;
    // A removed member.
    world
        .test
        .execute(&[
            &format!("DELETE FROM event_membership WHERE user_id = '{anna}'"),
            &format!("DELETE FROM organization_membership WHERE user_id = '{anna}'"),
        ])
        .await;
    for answer in [Answer::Done, Answer::OnTrack] {
        let text = world.webhook.press(ANNA, private(ANNA), answer, item).await;
        assert!(text.contains(OUTDATED), "{text}");
        unchanged().await;
    }
    support::logs::assert_clean(&["4711", "4712", "4799", "Festzelt", "Testgruppe"]);
}

#[tokio::test]
async fn invalid_transition_is_outdated() {
    let world = World::start().await;
    let action = world.action("blocked").await;
    let item = world.digest("action", action, "sent", "1 hour").await;

    let text = world
        .webhook
        .press(ANNA, private(ANNA), Answer::Blocked, item)
        .await;

    assert!(text.contains(OUTDATED), "{text}");
    assert_eq!(world.record("action", action).await, ("blocked".into(), 1));
    assert_eq!(world.response(action).await, (String::new(), String::new()));
}

#[tokio::test]
async fn only_the_private_chat_clears_the_blocked_mark() {
    let world = World::start().await;
    let anna = world.anna;
    world
        .test
        .execute(&[&format!(
            "UPDATE telegram_identity SET blocked_at = now() WHERE user_id = '{anna}'"
        )])
        .await;
    let action = world.action("open").await;
    let item = world.digest("action", action, "sent", "1 hour").await;
    let blocked = || async {
        world
            .test
            .scalar::<i64>("SELECT count(*) FROM telegram_identity WHERE blocked_at IS NOT NULL")
            .await
    };
    let group = json!({"id": -100_4711, "type": "group", "title": "Testgruppe"});
    let from = json!({"id": ANNA, "is_bot": false, "first_name": "Testperson"});

    // A message and a press in a group do not show that Anna allows the bot again.
    let message = json!({"update_id": 100, "message": {
        "message_id": 7, "date": 1, "text": "Hallo", "chat": group.clone(), "from": from.clone()
    }});
    assert_eq!(world.webhook.deliver(&message).await, StatusCode::OK);
    world
        .webhook
        .press(ANNA, group, Answer::OnTrack, item)
        .await;
    assert_eq!(blocked().await, 1, "a group update keeps the mark");

    // A press in the private chat clears it.
    world
        .webhook
        .press(ANNA, private(ANNA), Answer::OnTrack, item)
        .await;
    assert_eq!(blocked().await, 0, "Anna reached the bot again");

    // A message in the private chat clears it too.
    world
        .test
        .execute(&[&format!(
            "UPDATE telegram_identity SET blocked_at = now() WHERE user_id = '{anna}'"
        )])
        .await;
    let message = json!({"update_id": 101, "message": {
        "message_id": 8, "date": 1, "text": "Hallo", "chat": private(ANNA), "from": from
    }});
    assert_eq!(world.webhook.deliver(&message).await, StatusCode::OK);
    assert_eq!(blocked().await, 0, "Anna wrote to the bot again");
}
