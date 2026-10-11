//! The person features of ADR 0045 end to end: export, redaction, replay, retention, the orphan sweep and the
//! housekeeping. Each test proves one acceptance point of the data-protection gate through the public
//! surfaces: the HTTP API, the worker functions and the replay function of the command line.
//! The focused tests (`person_data.rs`, `redaction.rs`, `retention.rs`, `orphans.rs`, `events.rs` and the
//! housekeeping test of the store) hold the detail.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use bytes::Bytes;
use futures::TryStreamExt;
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};
use support::SESSION_COOKIE;
use support::files::pdf;
use support::organization::{Client, organization};
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_adapters::storage::testing::TestGarage;
use tada_app::blobs::{BlobKey, BlobStore, ByteStream};
use tada_app::caller::{JobRunner, ServiceCaller};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::housekeeping::{HousekeepingStore, LONG_KEEP, SHORT_KEEP};
use tada_app::jobs::{Ran, run_next};
use tada_app::orphans::{SWEEP_GRACE, SweepReport, sweep};
use tada_app::redaction::{ReplayReport, replay};
use tada_app::retention::{RetentionReport, run};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// An organization with an owner and one event.
struct World {
    test: TestDatabase,
    garage: TestGarage,
    organization: OrganizationId,
    owner_id: UserId,
    owner: Client,
    event: String,
}

impl World {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, owner_id, owner) = organization(&test, &garage, "testwil").await;
        let event = owner.create_event("OPEN30", "Open Day Testwil").await;
        Self {
            test,
            garage,
            organization,
            owner_id,
            owner,
            event,
        }
    }

    async fn person(&self, body: Value) -> String {
        let person = self.owner.post("/api/v1/persons", &body).await;
        person["id"].as_str().unwrap().to_owned()
    }

    /// A changeset with the source text `text` and a date proposal whose evidence is `quote`.
    async fn changeset(&self, text: &str, quote: &str) -> Value {
        let start = text[..text.find(quote).unwrap()].chars().count();
        let operation = self.owner.date_operation(&self.event).await;
        let body = json!({
            "source_text": text,
            "proposals": [{
                "id": Uuid::now_v7(),
                "operation": operation,
                "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
                "reason": "The member wrote it.",
            }],
        });
        self.owner
            .post(&format!("/api/v1/events/{}/changesets", self.event), &body)
            .await
    }

    async fn source_item(&self, changeset: &Value) -> Uuid {
        let version = changeset["source_version_id"].as_str().unwrap();
        self.test
            .scalar::<Uuid>(&format!(
                "SELECT source_item_id FROM source_version WHERE id = '{version}'"
            ))
            .await
    }

    async fn count(&self, sql: &str) -> i64 {
        self.test.scalar::<i64>(sql).await
    }

    /// Runs the worker until no job is left.
    async fn run_jobs(&self) {
        let handlers = tada::worker::handlers(
            &self.test.database,
            Arc::new(MemoryMailer::new()),
            Arc::new(FluentMailTexts::new().unwrap()),
            Arc::new(SystemClock),
            support::public_url(),
            Arc::new(self.garage.storage.clone()),
            support::messenger::digest_ports(support::messenger::FakeMessenger::new(
                tada_app::messenger::SendOutcome::Sent,
            )),
        );
        loop {
            let ran = run_next(
                &self.test.database,
                &handlers,
                Uuid::now_v7(),
                Duration::from_secs(60),
            )
            .await
            .unwrap();
            match ran {
                Ran::Idle => break,
                Ran::Completed(_) => {}
                other => panic!("a job did not complete: {other:?}"),
            }
        }
    }

    async fn redact_items(&self, items: &[Uuid]) -> Value {
        let targets: Vec<Value> = items
            .iter()
            .map(|id| json!({"kind": "source-item", "id": id}))
            .collect();
        self.owner
            .post(
                "/api/v1/redactions",
                &json!({"targets": targets, "reason": "request"}),
            )
            .await
    }

    /// The text of each journal object.
    async fn journal(&self) -> Vec<String> {
        let mut texts = Vec::new();
        for key in self.garage.keys().await {
            if key.starts_with("redaction-journal/") {
                let stream = self
                    .garage
                    .storage
                    .get(&BlobKey::restore(key))
                    .await
                    .unwrap()
                    .unwrap();
                let chunks: Vec<Bytes> = stream.try_collect().await.unwrap();
                texts.push(String::from_utf8(chunks.concat()).unwrap());
            }
        }
        texts
    }
}

fn body() -> ByteStream {
    Box::pin(futures::stream::iter([Ok::<_, io::Error>(
        Bytes::from_static(b"x"),
    )]))
}

/// Acceptance point 1: the export holds every mention that the substring rule finds, and the account data.
#[tokio::test]
async fn export_holds_every_mention_and_the_account_data() {
    let world = World::start().await;
    let email: String = world
        .test
        .scalar(&format!(
            "SELECT email FROM email_identity WHERE user_id = '{}'",
            world.owner_id
        ))
        .await;
    let person = world
        .person(
            json!({"name": "Beat Muster", "email": "beat.muster@example.org",
                       "user_id": world.owner_id.as_uuid()}),
        )
        .await;
    world
        .changeset("Kontakt: BEAT MUSTER, im Mai 2030.", "im Mai 2030")
        .await;
    world
        .owner
        .post(
            &format!("/api/v1/events/{}/actions", world.event),
            &json!({"title": "Mail an Beat.Muster@Example.org", "owner_user_id": world.owner_id.as_uuid()}),
        )
        .await;
    world
        .owner
        .post(
            &format!("/api/v1/events/{}/actions", world.event),
            &json!({"title": "Zelt aufstellen", "owner_user_id": world.owner_id.as_uuid()}),
        )
        .await;

    let request = support::request(Method::GET, &format!("/api/v1/persons/{person}/export"))
        .header(
            header::COOKIE,
            format!("{SESSION_COOKIE}={}", world.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(world.owner.router.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let export: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(export["format_version"], 1);
    let mut kinds: Vec<&str> = export["mentions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|mention| mention["kind"].as_str().unwrap())
        .collect();
    kinds.sort_unstable();
    assert_eq!(kinds, ["action", "source-item"], "{export}");
    assert_eq!(export["account"]["email"], email.as_str());
    assert_eq!(export["account"]["organization_role"], "owner");
}

/// Acceptance point 2: a redaction removes the content from the source, its quotes, the proposals and the records.
/// Links and manifests still resolve, and the audit log and the journal hold IDs only.
#[tokio::test]
async fn redaction_removes_content_and_keeps_links_and_logs_free_of_it() {
    let world = World::start().await;
    let text = "Beat Muster kommt im Mai 2030 und ruft Zelt an.";
    let date = world.owner.date_operation(&world.event).await;
    let action = Uuid::now_v7();
    let operation = json!({
        "kind": "create-action", "id": action, "event_id": world.event,
        "title": "Zelt Beat Muster", "owner": world.owner_id.as_uuid(),
    });
    let proposal = |operation: Value, quote: &str| {
        let start = text.find(quote).unwrap();
        json!({
            "id": Uuid::now_v7(),
            "operation": operation,
            "evidence": [{"start": start, "end": start + quote.len(), "quote": quote}],
            "reason": "Beat Muster schrieb es.",
        })
    };
    let changeset = world
        .owner
        .post(
            &format!("/api/v1/events/{}/changesets", world.event),
            &json!({"source_text": text,
                    "proposals": [proposal(date, "im Mai 2030"), proposal(operation, "ruft Zelt an")]}),
        )
        .await;
    world.owner.apply(&changeset).await;
    assert_eq!(
        world
            .count("SELECT count(*) FROM record_evidence WHERE quote IS NOT NULL")
            .await,
        1,
        "the action cites the source"
    );
    let item = world.source_item(&changeset).await;
    world
        .owner
        .post(
            "/api/v1/dispute-notes",
            &json!({"target_kind": "source-item", "target_id": item, "text": "Beat Muster bestreitet es."}),
        )
        .await;
    // The owner redacts the source item and the two proposals that came from it.
    let mut targets = vec![json!({"kind": "source-item", "id": item})];
    for id in changeset["proposal_ids"].as_array().unwrap() {
        targets.push(json!({"kind": "proposal", "id": id}));
    }
    world
        .owner
        .post(
            "/api/v1/redactions",
            &json!({"targets": targets, "reason": "request"}),
        )
        .await;
    world.run_jobs().await;

    // The text, the hash, the quotes, the proposals, the records and the dispute note are gone.
    assert_eq!(
        world
            .count(
                "SELECT count(*) FROM source_version
                 WHERE text IS NULL AND sha256 IS NULL AND redacted_at IS NOT NULL"
            )
            .await,
        1
    );
    for sql in [
        "SELECT count(*) FROM proposal_evidence WHERE quote IS NOT NULL",
        "SELECT count(*) FROM record_evidence WHERE quote IS NOT NULL",
        "SELECT count(*) FROM proposal WHERE reason IS NOT NULL",
        "SELECT count(*) FROM proposal WHERE operation::text LIKE '%Muster%'",
        "SELECT count(*) FROM dispute_note WHERE text IS NOT NULL",
    ] {
        assert_eq!(world.count(sql).await, 0, "{sql}");
    }
    assert_eq!(
        world
            .count("SELECT count(*) FROM proposal WHERE redacted_at IS NOT NULL")
            .await,
        2
    );
    // Links and manifests still resolve, and they show no content.
    let view = world
        .owner
        .get(&format!(
            "/api/v1/changesets/{}",
            changeset["id"].as_str().unwrap()
        ))
        .await
        .to_string();
    assert!(view.contains(changeset["source_version_id"].as_str().unwrap()));
    assert!(view.contains("\"redacted\":true"), "{view}");
    assert!(
        !view.contains("Muster") && !view.contains("Mai 2030"),
        "{view}"
    );
    let records = world
        .owner
        .get(&format!("/api/v1/events/{}/actions", world.event))
        .await
        .to_string();
    assert!(records.contains(&action.to_string()), "{records}");
    assert!(!records.contains("ruft Zelt an"), "{records}");

    // The audit log and the journal name the item by ID and hold no content.
    assert_eq!(
        world
            .count(&format!(
                "SELECT count(*) FROM audit_event
                 WHERE action = 'source_item.redact' AND record_id = '{item}' AND detail ->> 'reason' = 'request'"
            ))
            .await,
        1
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM audit_event WHERE detail::text ILIKE '%Muster%' OR detail::text ILIKE '%Mai 2030%'")
            .await,
        0
    );
    let journal = world.journal().await;
    assert_eq!(journal.len(), 3);
    let entries: Vec<Value> = journal
        .iter()
        .map(|text| serde_json::from_str(text).unwrap())
        .collect();
    assert!(entries.iter().any(
        |entry| entry["target_kind"] == "source-item" && entry["target_id"] == item.to_string()
    ));
    assert!(
        journal
            .iter()
            .all(|text| !text.contains("Muster") && !text.contains("Mai 2030"))
    );
}

/// Acceptance point 3: after a restore that brings the content back, the replay redacts it again.
#[tokio::test]
async fn replay_after_a_restore_redacts_again() {
    let world = World::start().await;
    let changeset = world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;
    let item = world.source_item(&changeset).await;
    let test = &world.test;
    test.execute(&[
        "CREATE TABLE backup_source_item AS SELECT * FROM source_item",
        "CREATE TABLE backup_source_version AS SELECT * FROM source_version",
        "CREATE TABLE backup_proposal_evidence AS SELECT * FROM proposal_evidence",
    ])
    .await;
    world.redact_items(&[item]).await;
    world.run_jobs().await;

    // The restore: the rows come back without the redaction, the journal object stays. The test switches the
    // triggers off for this; no code of tada can do it.
    test.execute(&[
        "SET LOCAL session_replication_role = replica",
        "UPDATE source_item s SET redacted_at = b.redacted_at FROM backup_source_item b WHERE b.id = s.id",
        "UPDATE source_version s
         SET text = b.text, url = b.url, sha256 = b.sha256, author_actor = b.author_actor,
             redacted_at = b.redacted_at
         FROM backup_source_version b WHERE b.id = s.id",
        "UPDATE proposal_evidence s SET quote = b.quote, redacted_at = b.redacted_at
         FROM backup_proposal_evidence b WHERE b.id = s.id",
        "DELETE FROM redaction",
    ])
    .await;
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE text IS NOT NULL")
            .await,
        1,
        "the restore brought the content back"
    );

    let report = replay(&world.test.database, &world.garage.storage)
        .await
        .unwrap();
    assert_eq!(
        report,
        ReplayReport {
            read: 1,
            applied: 1,
            ..ReplayReport::default()
        }
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE text IS NOT NULL")
            .await,
        0
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM proposal_evidence WHERE quote IS NOT NULL")
            .await,
        0
    );
}

/// Acceptance point 4: the job redacts a closed event after its period, and a closed event refuses writes.
#[tokio::test]
async fn retention_redacts_a_closed_event_which_refuses_writes() {
    let world = World::start().await;
    world
        .changeset("Beat Muster kommt im Mai 2030.", "im Mai 2030")
        .await;

    // Retention is off until the owner sets a period.
    let read = world.owner.get("/api/v1/organization/retention").await;
    assert_eq!(read["closed_event_months"], Value::Null);
    world
        .owner
        .post(
            "/api/v1/organization/retention/set",
            &json!({"closed_event_months": 6, "source_months": null, "expected_version": read["version"]}),
        )
        .await;
    let db = &world.test.database;
    let report = run(db, db, &world.garage.storage, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(report, RetentionReport::default(), "an active event stays");

    world
        .owner
        .post(
            &format!("/api/v1/events/{}/close", world.event),
            &json!({"expected_version": 1}),
        )
        .await;
    // A young closed event stays; the same event 7 months later is due.
    let report = run(db, db, &world.garage.storage, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(report, RetentionReport::default());
    world
        .test
        .execute(&[&format!(
            "UPDATE event SET closed_at = now() - interval '7 months' WHERE id = '{}'",
            world.event
        )])
        .await;
    let report = run(db, db, &world.garage.storage, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(
        report,
        RetentionReport {
            events: 1,
            ..RetentionReport::default()
        }
    );
    assert_eq!(
        world
            .count(
                "SELECT count(*) FROM source_version WHERE text IS NULL AND redacted_at IS NOT NULL"
            )
            .await,
        1
    );

    // The event refuses writes.
    let request = support::request(
        Method::POST,
        &format!("/api/v1/events/{}/actions", world.event),
    )
    .header(header::CONTENT_TYPE, "application/json");
    let action = json!({"title": "Zelt aufstellen", "owner_user_id": world.owner_id.as_uuid()});
    let (status, problem) = world
        .owner
        .call(request, Body::from(action.to_string()))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "event-closed");
}

/// Acceptance point 5: the sweep deletes an orphan object and aborts an old incomplete upload, and it keeps a
/// referenced object.
#[tokio::test]
async fn sweep_removes_orphans_and_keeps_referenced_objects() {
    let world = World::start().await;
    world
        .owner
        .upload(
            &format!("/api/v1/events/{}/documents", world.event),
            "Programm.pdf",
            pdf("Programm Testwil eins"),
        )
        .await;
    let referenced = world
        .test
        .scalar::<String>("SELECT blob_key FROM document_version")
        .await;
    let orphan = BlobKey::new(world.organization);
    world
        .garage
        .storage
        .put(&orphan, body(), u64::MAX)
        .await
        .unwrap();
    world
        .garage
        .start_upload(BlobKey::new(world.organization).as_str())
        .await;

    let later = Timestamp::now() + SWEEP_GRACE + SignedDuration::from_mins(1);
    let report = sweep(&world.garage.storage, &world.test.database, later)
        .await
        .unwrap();
    assert_eq!(
        report,
        SweepReport {
            deleted: 1,
            aborted: 1,
            kept: 1
        }
    );
    assert_eq!(world.garage.keys().await, vec![referenced]);
    assert_eq!(world.garage.open_uploads().await, 0);
}

/// Acceptance point 6: the housekeeping deletes rows after their periods and keeps younger rows.
#[tokio::test]
async fn housekeeping_deletes_after_the_period_and_keeps_younger_rows() {
    let world = World::start().await;
    let now: Timestamp = "2031-06-30T12:00:00Z".parse().unwrap();
    let second = SignedDuration::from_secs(1);
    let before = |period: SignedDuration, offset: SignedDuration| now - period + offset;
    for (n, expires_at) in [before(SHORT_KEEP, -second), before(SHORT_KEEP, second)]
        .into_iter()
        .enumerate()
    {
        world
            .test
            .execute(&[&format!(
                "INSERT INTO magic_link (token_hash, user_id, expires_at, created_at)
                 VALUES (decode(repeat('{n}{n}', 32), 'hex'), '{user}', '{expires_at}', '{expires_at}')",
                user = world.owner_id
            )])
            .await;
    }
    let job = |n: i64, failed_at: Timestamp| {
        format!(
            "INSERT INTO job (id, kind, version, payload, run_at, failed_at, created_at)
             VALUES ('{}', 'write-redaction-journal', 1, '{{}}', '{failed_at}', '{failed_at}', '{failed_at}')",
            Uuid::from_u128(n as u128)
        )
    };
    world
        .test
        .execute(&[
            &job(1, before(LONG_KEEP, -second)),
            &job(2, before(LONG_KEEP, second)),
        ])
        .await;

    let caller = ServiceCaller::<JobRunner>::new();
    let report = world.test.database.run(&caller, now).await.unwrap();
    assert_eq!(report.magic_links, 1);
    assert_eq!(report.failed_jobs, 1);
    assert_eq!(world.count("SELECT count(*) FROM magic_link").await, 1);
    assert_eq!(world.count("SELECT count(*) FROM job").await, 1);
}
