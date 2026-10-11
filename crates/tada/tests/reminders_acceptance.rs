//! The acceptance of the reminders (roadmap slice 2c, ADR 0076), end to end and in order: the schedule starts the
//! daily check, the check plans a digest, the send obeys the quiet hours and the cooldown across restarts, a
//! changed deadline and a completed action cancel obsolete reminders, the owner answers through Telegram, and the
//! digest goes out without AI. The data is invented, and the app clock of each worker is fixed by the test.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use tada_adapters::fake_model::FakeModel;
use tada_app::caller::OrganizationRole;
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::ids::{DigestItemId, EventId, OrganizationId, UserId};
use tada_app::domain::reminders::{Answer, CallbackData};
use tada_app::jobs::{Handlers, Ran, SCHEDULED_KINDS, ScheduleRunner};
use tada_app::messenger::{Button, SendOutcome};
use tada_app::model::ModelError;
use tada_app::pm::send::DigestAi;
use tada_app::pm::{PM_CHECK_JOB, SEND_DIGEST_JOB, ensure_default_check_schedules};
use tada_store_pg::Database;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

use support::messenger::{FakeMessenger, Sent, digest_ai, digest_worker, run_all, run_one};
use support::telegram::{Webhook, private};

/// The invented Telegram accounts: Anna owns the actions, Bert is another linked member.
const ANNA: i64 = 4711;
const BERT: i64 = 4712;

/// A clock that stands at `time`.
fn at(time: &str) -> Arc<dyn Clock> {
    Arc::new(support::TestClock::new(time.parse().unwrap()))
}

/// One organization with the default AI-PM policy and one event, in which Anna and Bert contribute.
struct World {
    test: TestDatabase,
    organization: OrganizationId,
    event: EventId,
    anna: UserId,
}

impl World {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let event = test.create_event(organization, "FEST").await;
        let anna = Self::linked(&test, organization, event, "Anna Beispiel", ANNA).await;
        Self::linked(&test, organization, event, "Bert Muster", BERT).await;
        Self {
            test,
            organization,
            event,
            anna,
        }
    }

    /// Adds a member who opted in, in the time zone Europe/Zurich, with a confirmed Telegram link.
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
                "UPDATE app_user SET reminders_opt_in_at = now(), time_zone = 'Europe/Zurich' WHERE id = '{user}'"
            ),
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
    async fn action(&self, title: &str, due: &str, status: &str) -> Uuid {
        let id = Uuid::now_v7();
        let (organization, event, anna) = (self.organization, self.event, self.anna);
        self.test
            .execute(&[&format!(
                "INSERT INTO action (id, organization_id, event_id, local_number, title, owner_user_id, due_date,
                                     status, version, created_at, updated_at)
                 VALUES ('{id}', '{organization}', '{event}',
                         (SELECT coalesce(max(local_number), 0) + 1 FROM action WHERE event_id = '{event}'),
                         '{title}', '{anna}', '{due}', '{status}', 1, now(), now())"
            )])
            .await;
        id
    }

    /// A new worker process: its own connection pool and new handler instances, so nothing is kept in memory.
    fn worker(
        &self,
        messenger: &Arc<FakeMessenger>,
        ai: Option<DigestAi>,
        clock: Arc<dyn Clock>,
    ) -> (Database, Handlers) {
        let database = self.test.connect();
        let handlers = digest_worker(&database, messenger.clone(), ai, clock);
        (database, handlers)
    }

    /// The scheduler step of a worker after `missed` of downtime: the daily check was due `missed` ago. The
    /// schedule reads the database clock, so the test moves the row and not the clock. Returns the number of
    /// started checks.
    async fn scheduler_step(&self, missed: &str) -> u32 {
        self.test
            .execute(&[&format!(
                "UPDATE schedule SET next_run_at = now() - interval '{missed}' WHERE kind = '{PM_CHECK_JOB}'"
            )])
            .await;
        self.test
            .connect()
            .start_due(SCHEDULED_KINDS)
            .await
            .unwrap()
            .started
    }

    /// The digests of Anna, oldest first: ID and status.
    async fn digests(&self) -> Vec<(Uuid, String)> {
        let rows: String = self
            .test
            .scalar(&format!(
                "SELECT coalesce(string_agg(id::text || ' ' || status, ',' ORDER BY created_at), '')
                 FROM outbound_intent WHERE purpose = 'telegram-digest' AND user_id = '{}'",
                self.anna
            ))
            .await;
        rows.split(',')
            .filter(|row| !row.is_empty())
            .map(|row| {
                let (id, status) = row.split_once(' ').unwrap();
                (id.parse().unwrap(), status.to_owned())
            })
            .collect()
    }

    /// The records of the items of a digest, in the order of their IDs.
    async fn records(&self, intent: Uuid) -> Vec<Uuid> {
        let ids: String = self
            .test
            .scalar(&format!(
                "SELECT coalesce(string_agg(record_id::text, ',' ORDER BY record_id), '') FROM digest_item
                 WHERE intent_id = '{intent}'"
            ))
            .await;
        ids.split(',')
            .filter(|id| !id.is_empty())
            .map(|id| id.parse().unwrap())
            .collect()
    }

    /// The run time of each send job that waits, in UTC.
    async fn waiting_sends(&self) -> Vec<String> {
        let rows: String = self
            .test
            .scalar(&format!(
                "SELECT coalesce(string_agg(to_char(run_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI'), ','), '')
                 FROM job WHERE kind = '{SEND_DIGEST_JOB}' AND failed_at IS NULL"
            ))
            .await;
        rows.split(',')
            .filter(|row| !row.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// The status and the version of an action.
    async fn action_state(&self, id: Uuid) -> (String, i64) {
        let status = self
            .test
            .scalar(&format!("SELECT status FROM action WHERE id = '{id}'"))
            .await;
        let version = self
            .test
            .scalar(&format!("SELECT version FROM action WHERE id = '{id}'"))
            .await;
        (status, version)
    }

    /// The reference of the item of `record` in the digest `intent`.
    async fn item(&self, intent: Uuid, record: Uuid) -> DigestItemId {
        let id: Uuid = self
            .test
            .scalar(&format!(
                "SELECT id FROM digest_item WHERE intent_id = '{intent}' AND record_id = '{record}'"
            ))
            .await;
        DigestItemId::from_uuid(id)
    }
}

/// The answers that the buttons of a message carry.
fn buttons(sent: &Sent) -> Vec<CallbackData> {
    sent.rows
        .iter()
        .flatten()
        .filter_map(|button| match button {
            Button::Callback { data, .. } => Some(CallbackData::decode(data.as_str()).unwrap()),
            Button::Url { .. } => None,
        })
        .collect()
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

fn completed(ran: &[Ran]) -> bool {
    ran.iter()
        .all(|ran| matches!(ran, Ran::Completed(_) | Ran::CompletedWithWarning(..)))
}

/// The answer of the gateway to a press that changed nothing.
const OUTDATED: &str = "veraltet";

#[tokio::test]
async fn reminders_from_plan_to_answer() {
    // 1. An organization with the default AI-PM policy; Anna opted in, has a confirmed Telegram link and lives
    // in Europe/Zurich. On Tuesday 15 September 2026, one action is due tomorrow and one is blocked.
    let world = World::start().await;
    assert_eq!(
        ensure_default_check_schedules(&world.test.database)
            .await
            .unwrap(),
        1,
        "the default policy has its daily check"
    );
    let flyer = world.action("Flyer drucken", "2026-09-16", "open").await;
    let tent = world
        .action("Festzelt aufstellen", "2026-09-17", "blocked")
        .await;
    let messenger = FakeMessenger::new(SendOutcome::Sent);

    // 2. The schedule starts the daily check, and the check plans one digest with both items.
    // 3. Quiet hours: the check runs at 21:00 in Zurich, and the send waits for 08:00.
    assert_eq!(world.scheduler_step("1 minute").await, 1);
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-15T19:00:00Z"));
    assert!(matches!(
        run_one(&database, &handlers).await,
        Ran::Completed(_)
    ));
    let digests = world.digests().await;
    assert_eq!(digests.len(), 1, "{digests:?}");
    let (first, ref status) = digests[0];
    assert_eq!(status, "pending");
    assert_eq!(world.records(first).await, sorted(vec![flyer, tent]));
    assert_eq!(
        world.waiting_sends().await,
        vec!["2026-09-16T06:00"],
        "the send waits for 08:00 in Zurich"
    );
    assert!(messenger.sent().is_empty());

    // 4. Quiet hours survive a restart: a new worker that comes back at 07:59 and finds the send due sends
    // nothing, and the send waits for 08:00 again.
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-16T05:59:00Z"));
    assert!(matches!(
        run_one(&database, &handlers).await,
        Ran::Completed(_)
    ));
    assert!(messenger.sent().is_empty(), "a send before 08:00");
    assert_eq!(world.waiting_sends().await, vec!["2026-09-16T06:00"]);

    // 5. A changed deadline cancels the obsolete reminder: before 08:00, the due date of the flyer moves out of
    // the look-ahead. At 08:00 a new worker sends the tent only.
    world
        .test
        .execute(&[&format!(
            "UPDATE action SET due_date = '2026-11-30', version = 2 WHERE id = '{flyer}'"
        )])
        .await;
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-16T06:00:00Z"));
    assert!(completed(&run_all(&database, &handlers).await));
    let sent = messenger.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, ANNA);
    assert!(
        sent[0].text.contains("Festzelt aufstellen"),
        "{}",
        sent[0].text
    );
    assert!(!sent[0].text.contains("Flyer drucken"), "{}", sent[0].text);
    assert_eq!(world.digests().await[0].1, "sent");
    assert_eq!(world.records(first).await, vec![tent]);

    // 6. The cooldown survives a restart: a new action comes up, and the next check of a new worker on the
    // same day plans no second digest.
    let benches = world.action("Bänke bestellen", "2026-09-17", "open").await;
    assert_eq!(world.scheduler_step("1 minute").await, 1);
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-16T06:30:00Z"));
    assert!(completed(&run_all(&database, &handlers).await));
    assert_eq!(
        world.digests().await.len(),
        1,
        "a second digest on the same day"
    );
    assert_eq!(messenger.sent().len(), 1);

    // 7. No flood after downtime (PRODUCT.md): the worker is down for three days and comes back on Monday at
    // 07:00 in Zurich. The scheduler starts one check, and the check plans one digest: the follow-up of the
    // tent and the benches. The quiet hours hold it until 08:00.
    assert_eq!(
        world.scheduler_step("3 days").await,
        1,
        "one check, not three"
    );
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-21T05:00:00Z"));
    assert!(matches!(
        run_one(&database, &handlers).await,
        Ran::Completed(_)
    ));
    let digests = world.digests().await;
    assert_eq!(digests.len(), 2, "{digests:?}");
    let (monday, ref status) = digests[1];
    assert_eq!(status, "pending");
    assert_eq!(world.records(monday).await, sorted(vec![tent, benches]));
    assert_eq!(world.waiting_sends().await, vec!["2026-09-21T06:00"]);

    // 8. A completed action cancels the obsolete reminder: before 08:00, Anna presses "Erledigt" on the tent in
    // the digest of Wednesday, in her private chat. The action is done, and at 08:00 the digest has the benches
    // only.
    let done = world.item(first, tent).await;
    assert!(
        buttons(&messenger.sent()[0])
            .iter()
            .any(|data| data.answer == Answer::Done && data.item == done),
        "the digest of Wednesday has the button"
    );
    let webhook = Webhook::start(&world.test, at("2026-09-21T05:10:00Z")).await;
    let text = webhook.press(ANNA, private(ANNA), Answer::Done, done).await;
    assert!(!text.contains(OUTDATED), "{text}");
    assert_eq!(world.action_state(tent).await, ("done".into(), 2));
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-21T06:00:00Z"));
    assert!(completed(&run_all(&database, &handlers).await));
    let sent = messenger.sent();
    assert_eq!(sent.len(), 2, "one digest after the downtime");
    assert!(sent[1].text.contains("Bänke bestellen"), "{}", sent[1].text);
    assert!(!sent[1].text.contains("Festzelt"), "{}", sent[1].text);
    assert_eq!(world.digests().await[1].1, "sent");
    assert_eq!(world.records(monday).await, vec![benches]);

    // 9. Only the owner acts: Bert presses the same button, and the "Erledigt" of the benches, which is still
    // open. Nothing changes.
    let benches_done = world.item(monday, benches).await;
    for item in [done, benches_done] {
        let text = webhook.press(BERT, private(BERT), Answer::Done, item).await;
        assert!(text.contains(OUTDATED), "{text}");
    }
    assert_eq!(world.action_state(tent).await, ("done".into(), 2));
    assert_eq!(world.action_state(benches).await, ("open".into(), 1));

    // 10. The next check reminds Anna of nothing for the done tent.
    assert_eq!(world.scheduler_step("1 minute").await, 1);
    let (database, handlers) = world.worker(&messenger, None, at("2026-09-22T06:30:00Z"));
    assert!(completed(&run_all(&database, &handlers).await));
    assert_eq!(world.digests().await.len(), 2, "a digest for a done action");
    assert_eq!(messenger.sent().len(), 2);

    // 11. AI is optional: the organization turns AI on, and the model fails. On Wednesday the follow-up of the
    // benches goes out with the template.
    let organization = world.organization;
    world
        .test
        .execute(&[
            &format!(
                "INSERT INTO pm_policy (organization_id, local_time, time_zone, weekdays, quiet_start, quiet_end,
                                        cooldown_hours, follow_up_working_days, look_ahead_working_days, paused,
                                        ai_enabled)
                 VALUES ('{organization}', '08:30', 'Europe/Zurich', 31, '20:00', '08:00', 20, 2, 2, false, true)"
            ),
            "INSERT INTO ai_price VALUES ('claude-haiku-5-5', '2026-01-01', 1, 5)",
        ])
        .await;
    let model = Arc::new(FakeModel::new([Err(ModelError::Temporary)]));
    assert_eq!(world.scheduler_step("1 minute").await, 1);
    let (database, handlers) = world.worker(
        &messenger,
        Some(digest_ai(&world.test.database, model.clone())),
        at("2026-09-23T06:30:00Z"),
    );
    assert!(completed(&run_all(&database, &handlers).await));
    assert_eq!(model.requests().len(), 1, "the AI was asked");
    let sent = messenger.sent();
    assert_eq!(sent.len(), 3);
    assert!(sent[2].text.contains("Bänke bestellen"), "{}", sent[2].text);
    assert!(!sent[2].text.contains("Festzelt"), "{}", sent[2].text);
    let digests = world.digests().await;
    assert_eq!(digests.len(), 3, "{digests:?}");
    assert_eq!(digests[2].1, "sent");
    assert_eq!(world.records(digests[2].0).await, vec![benches]);
}
