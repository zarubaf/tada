//! Legal redaction with the object storage and the worker (ADR 0072): the delete of the object after the commit,
//! the deletion journal, and its replay after a restore of the database.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::TryStreamExt;
use serde_json::{Value, json};
use support::files::pdf;
use support::organization::{Client, organization};
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_adapters::storage::testing::TestGarage;
use tada_app::blobs::{BlobKey, BlobStore, ByteStream};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, RedactionId, UserId};
use tada_app::domain::redaction::{RedactionTarget, RedactionTargetKind};
use tada_app::jobs::{Ran, run_next};
use tada_app::redaction::{RedactionError, ReplayReport, redact, redact_for_retention, replay};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// An organization with an event, a member text with its changeset, and an uploaded document.
struct World {
    test: TestDatabase,
    garage: TestGarage,
    organization: OrganizationId,
    owner: MemberCaller,
    client: Client,
    event: Uuid,
    /// The source item of the member text.
    text: Uuid,
    /// The upload version and its object.
    upload: Uuid,
    object: String,
}

impl World {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, user, client) = organization(&test, &garage, "testwil").await;
        let event = client.create_event("OPEN30", "Open Day Testwil").await;
        let operation = client.date_operation(&event).await;
        client.propose(&event, operation, "im Mai 2030").await;
        client
            .upload(
                &format!("/api/v1/events/{event}/documents"),
                "Programm.pdf",
                pdf("Programm Testwil eins"),
            )
            .await;
        let text = test
            .scalar::<Uuid>("SELECT id FROM source_item WHERE kind = 'member-text'")
            .await;
        let upload = test.scalar::<Uuid>("SELECT id FROM document_version").await;
        let object = test
            .scalar::<String>("SELECT blob_key FROM document_version")
            .await;
        Self {
            event: event.parse().unwrap(),
            owner: MemberCaller::new(user, organization, OrganizationRole::Owner),
            test,
            garage,
            organization,
            client,
            text,
            upload,
            object,
        }
    }

    fn targets(&self) -> [RedactionTarget; 2] {
        [
            RedactionTarget {
                kind: RedactionTargetKind::SourceItem,
                id: self.text,
            },
            RedactionTarget {
                kind: RedactionTargetKind::DocumentVersion,
                id: self.upload,
            },
        ]
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

    async fn journal(&self, redaction: RedactionId) -> Value {
        let key = BlobKey::journal(self.organization, redaction);
        let stream = self.garage.storage.get(&key).await.unwrap().unwrap();
        let chunks: Vec<Bytes> = stream.try_collect().await.unwrap();
        serde_json::from_slice(&chunks.concat()).unwrap()
    }
}

fn body(content: &[u8]) -> ByteStream {
    let bytes = Bytes::copy_from_slice(content);
    Box::pin(futures::stream::iter([Ok::<_, io::Error>(bytes)]))
}

#[tokio::test]
async fn member_and_admin_and_ai_cannot_redact() {
    let world = World::start().await;
    // An AI client cannot call the command at all: it takes a member (see the doctest of `redact`).
    for role in [OrganizationRole::Member, OrganizationRole::Admin] {
        let caller = MemberCaller::new(UserId::from_uuid(Uuid::now_v7()), world.organization, role);
        let result = redact(
            &caller,
            &world.targets(),
            &world.test.database,
            &world.garage.storage,
        )
        .await;
        assert!(
            matches!(result, Err(RedactionError::Forbidden)),
            "{role:?}: {result:?}"
        );
    }
    // The owner of another organization does not find the targets.
    let other = world.test.create_organization("musterhausen").await;
    let stranger = MemberCaller::new(
        UserId::from_uuid(Uuid::now_v7()),
        other,
        OrganizationRole::Owner,
    );
    let result = redact(
        &stranger,
        &world.targets(),
        &world.test.database,
        &world.garage.storage,
    )
    .await;
    assert!(
        matches!(result, Err(RedactionError::NotFound)),
        "{result:?}"
    );
    assert_eq!(world.count("SELECT count(*) FROM redaction").await, 0);
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE redacted_at IS NOT NULL")
            .await,
        0
    );
    assert!(world.garage.keys().await.contains(&world.object));
}

#[tokio::test]
async fn redaction_deletes_blob_after_commit() {
    let world = World::start().await;
    let redacted = redact(
        &world.owner,
        &world.targets()[1..],
        &world.test.database,
        &world.garage.storage,
    )
    .await
    .unwrap();
    assert_eq!(redacted.applied.len(), 1);
    assert_eq!(redacted.undeleted_objects, 0);
    assert!(!world.garage.keys().await.contains(&world.object));
    // The row stays with its key, so the download answers that the content is redacted.
    let document = world
        .test
        .scalar::<Uuid>("SELECT document_id FROM document_version")
        .await;
    let view = world
        .client
        .get(&format!("/api/v1/documents/{document}"))
        .await;
    assert_eq!(view["newest_version"]["redacted"], json!(true), "{view}");
}

#[tokio::test]
async fn journal_object_written_by_worker() {
    let world = World::start().await;
    let redacted = redact(
        &world.owner,
        &world.targets(),
        &world.test.database,
        &world.garage.storage,
    )
    .await
    .unwrap();
    assert_eq!(redacted.applied.len(), 2);
    world.run_jobs().await;

    let mut journal: Vec<String> = world
        .garage
        .keys()
        .await
        .into_iter()
        .filter(|key| key.starts_with("redaction-journal/"))
        .collect();
    journal.sort();
    let mut expected: Vec<String> = redacted
        .applied
        .iter()
        .map(|id| format!("redaction-journal/{}/{id}.json", world.organization))
        .collect();
    expected.sort();
    assert_eq!(journal, expected);

    let source = redacted.applied[0];
    let entry = world.journal(source).await;
    let redacted_at = world
        .test
        .scalar::<String>(&format!(
            "SELECT to_json(redacted_at) #>> '{{}}' FROM redaction WHERE id = '{source}'"
        ))
        .await;
    assert_eq!(
        entry,
        json!({
            "id": source.to_string(),
            "organization_id": world.organization.to_string(),
            "target_kind": "source-item",
            "target_id": world.text.to_string(),
            "reason": "request",
            "redacted_at": entry["redacted_at"],
        })
    );
    // The same instant as the row; the journal writes it in UTC.
    let journal_time: jiff::Timestamp = entry["redacted_at"].as_str().unwrap().parse().unwrap();
    let row_time: jiff::Timestamp = redacted_at.parse().unwrap();
    assert_eq!(journal_time, row_time);
}

#[tokio::test]
async fn replay_after_restore_redacts_again() {
    let world = World::start().await;
    let test = &world.test;
    // The backup: the rows before the redaction.
    test.execute(&[
        "CREATE TABLE backup_source_item AS SELECT * FROM source_item",
        "CREATE TABLE backup_source_version AS SELECT * FROM source_version",
        "CREATE TABLE backup_proposal_evidence AS SELECT * FROM proposal_evidence",
        "CREATE TABLE backup_document_version AS SELECT * FROM document_version",
    ])
    .await;
    let redacted = redact(
        &world.owner,
        &world.targets(),
        &world.test.database,
        &world.garage.storage,
    )
    .await
    .unwrap();
    world.run_jobs().await;

    // The restore of the backup: the content comes back and the redaction rows go. The test switches the
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
        "UPDATE document_version s
         SET markdown = b.markdown, file_name = b.file_name, sha256 = b.sha256, redacted_at = b.redacted_at
         FROM backup_document_version b WHERE b.id = s.id",
        "DELETE FROM redaction",
    ])
    .await;
    // The restore of the object storage keeps the journal and brings the object back.
    world
        .garage
        .storage
        .put(&BlobKey::restore(world.object.clone()), body(b"x"), 10)
        .await
        .unwrap();
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE redacted_at IS NOT NULL")
            .await,
        0,
        "the restore brought the content back"
    );

    let report = replay(&world.test.database, &world.garage.storage)
        .await
        .unwrap();
    assert_eq!(
        report,
        ReplayReport {
            read: 2,
            applied: 2,
            already: 0,
            missing: 0,
            undeleted_objects: 0,
        }
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE redacted_at IS NULL")
            .await,
        0,
        "the text and the upload are redacted again"
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM proposal_evidence WHERE quote IS NOT NULL")
            .await,
        0
    );
    assert_eq!(
        world
            .count(&format!(
                "SELECT count(*) FROM document_version WHERE id = '{}' AND file_name IS NULL AND redacted_at IS NOT NULL",
                world.upload
            ))
            .await,
        1
    );
    assert!(!world.garage.keys().await.contains(&world.object));
    // The rows have the IDs, reasons and times of the journal, and no member.
    for id in &redacted.applied {
        assert_eq!(
            world
                .count(&format!(
                    "SELECT count(*) FROM redaction WHERE id = '{id}' AND reason = 'request' AND actor_id IS NULL"
                ))
                .await,
            1
        );
    }
    assert_eq!(
        world
            .count("SELECT count(*) FROM audit_event WHERE action LIKE '%.redact' AND actor_kind = 'service'")
            .await,
        2
    );
    // No new journal job: the journal holds the redactions already.
    assert_eq!(
        world
            .count("SELECT count(*) FROM job WHERE kind = 'write-redaction-journal'")
            .await,
        0
    );

    // A second replay finds nothing to do.
    let again = replay(&world.test.database, &world.garage.storage)
        .await
        .unwrap();
    assert_eq!(
        again,
        ReplayReport {
            read: 2,
            already: 2,
            ..ReplayReport::default()
        }
    );
}

#[tokio::test]
async fn replay_closes_an_event_that_the_restore_holds_open() {
    let world = World::start().await;
    let test = &world.test;
    let target = RedactionTarget {
        kind: RedactionTargetKind::Event,
        id: world.event,
    };
    // Only a closed event can be redacted; an open one is an invalid transition, not an internal error.
    let open = redact_for_retention(
        world.owner.scope(),
        target,
        &world.test.database,
        &world.garage.storage,
    )
    .await;
    assert!(
        matches!(open, Err(RedactionError::InvalidTransition)),
        "{open:?}"
    );

    test.execute(&[
        "CREATE TABLE backup_source_version AS SELECT * FROM source_version",
        "CREATE TABLE backup_document_version AS SELECT * FROM document_version",
        "UPDATE event SET status = 'closed', closed_at = now() - interval '400 days'",
    ])
    .await;
    let redacted = redact_for_retention(
        world.owner.scope(),
        target,
        &world.test.database,
        &world.garage.storage,
    )
    .await
    .unwrap();
    assert_eq!(redacted.applied.len(), 1);
    world.run_jobs().await;
    let journal = world.journal(redacted.applied[0]).await;

    // The backup is older than the close: the event is open, and its texts and object are back.
    test.execute(&[
        "SET LOCAL session_replication_role = replica",
        "UPDATE event SET status = 'active', closed_at = NULL, redacted_at = NULL",
        "UPDATE source_version s
         SET text = b.text, url = b.url, sha256 = b.sha256, author_actor = b.author_actor,
             redacted_at = b.redacted_at
         FROM backup_source_version b WHERE b.id = s.id",
        "UPDATE document_version s
         SET markdown = b.markdown, file_name = b.file_name, sha256 = b.sha256, redacted_at = b.redacted_at
         FROM backup_document_version b WHERE b.id = s.id",
        "DELETE FROM redaction",
    ])
    .await;
    world
        .garage
        .storage
        .put(&BlobKey::restore(world.object.clone()), body(b"x"), 10)
        .await
        .unwrap();

    let report = replay(&world.test.database, &world.garage.storage)
        .await
        .unwrap();
    assert_eq!((report.read, report.applied), (1, 1), "{report:?}");
    // The replay closed the event at the time of the redaction, then redacted it.
    let closed_at = world
        .test
        .scalar::<String>(
            "SELECT to_json(closed_at) #>> '{}' FROM event
             WHERE status = 'closed' AND redacted_at IS NOT NULL",
        )
        .await;
    let closed_at: jiff::Timestamp = closed_at.parse().unwrap();
    let redacted_at: jiff::Timestamp = journal["redacted_at"].as_str().unwrap().parse().unwrap();
    assert_eq!(closed_at, redacted_at);
    assert_eq!(
        world
            .count(
                "SELECT count(*) FROM audit_event WHERE action = 'event.close' AND actor_kind = 'service'
                   AND channel = 'cli'"
            )
            .await,
        1
    );
    assert_eq!(
        world
            .count("SELECT count(*) FROM source_version WHERE redacted_at IS NULL")
            .await,
        0
    );
    assert!(!world.garage.keys().await.contains(&world.object));
}

/// A proposal tombstone keeps the kind of its operation only (`{"kind": ...}`). The API reads its changeset
/// and the list of changesets without an error, and shows the proposal as redacted and rejected.
#[tokio::test]
async fn a_redacted_proposal_reads_through_the_api() {
    let world = World::start().await;
    let proposal = world.test.scalar::<Uuid>("SELECT id FROM proposal").await;
    let changeset = world
        .test
        .scalar::<Uuid>("SELECT changeset_id FROM proposal")
        .await;
    redact(
        &world.owner,
        &[RedactionTarget {
            kind: RedactionTargetKind::Proposal,
            id: proposal,
        }],
        &world.test.database,
        &world.garage.storage,
    )
    .await
    .unwrap();
    assert_eq!(
        world
            .test
            .scalar::<Value>("SELECT operation FROM proposal")
            .await,
        json!({"kind": "set_fact"})
    );

    let view = world
        .client
        .get(&format!("/api/v1/changesets/{changeset}"))
        .await;
    assert_eq!(view["proposals"], json!([]), "{view}");
    assert_eq!(
        view["redacted_proposals"][0]["id"],
        json!(proposal.to_string())
    );
    assert_eq!(
        view["redacted_proposals"][0]["operation_kind"],
        json!("set_fact")
    );
    assert_eq!(view["redacted_proposals"][0]["redacted"], json!(true));
    assert_eq!(view["redacted_proposals"][0]["status"], json!("rejected"));
    // The inbox leaves the emptied proposal out.
    let event = world.event;
    let inbox = world
        .client
        .get(&format!("/api/v1/events/{event}/changesets?status=open"))
        .await;
    assert_eq!(inbox["items"], json!([]), "{inbox}");
    let inbox = world.client.get("/api/v1/changesets?status=open").await;
    assert_eq!(inbox["items"], json!([]), "{inbox}");
}
