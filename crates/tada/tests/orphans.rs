//! The sweep of orphan objects in the object storage (ADR 0074).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::io;

use bytes::Bytes;
use jiff::{SignedDuration, Timestamp};
use support::files::pdf;
use support::organization::organization;
use tada_adapters::storage::testing::TestGarage;
use tada_app::blobs::{BlobKey, BlobStore, ByteStream};
use tada_app::orphans::{SWEEP_GRACE, SweepReport, sweep};
use tada_store_pg::testing::TestDatabase;

const JOURNAL: &str = "redaction-journal/2030-05-18.json";

fn body() -> ByteStream {
    Box::pin(futures::stream::iter([Ok::<_, io::Error>(
        Bytes::from_static(b"x"),
    )]))
}

async fn put(garage: &TestGarage, key: &BlobKey) {
    garage.storage.put(key, body(), u64::MAX).await.unwrap();
}

/// An organization with one uploaded document, and its object.
async fn uploaded(
    test: &TestDatabase,
    garage: &TestGarage,
) -> (tada_app::domain::ids::OrganizationId, String) {
    let (organization, _, client) = organization(test, garage, "testwil").await;
    let event = client.create_event("OPEN30", "Open Day Testwil").await;
    client
        .upload(
            &format!("/api/v1/events/{event}/documents"),
            "Programm.pdf",
            pdf("Programm Testwil eins"),
        )
        .await;
    let key = test
        .scalar::<String>("SELECT blob_key FROM document_version")
        .await;
    (organization, key)
}

#[tokio::test]
async fn sweep_keeps_referenced_young_and_journal() {
    let test = TestDatabase::start().await;
    let garage = TestGarage::start().await;
    let (organization, referenced) = uploaded(&test, &garage).await;
    let orphan = BlobKey::new(organization);
    put(&garage, &orphan).await;
    put(&garage, &BlobKey::restore(JOURNAL.to_owned())).await;
    let stale_upload = BlobKey::new(organization);
    garage.start_upload(stale_upload.as_str()).await;

    // Everything is younger than the grace: the sweep deletes and aborts nothing.
    let report = sweep(&garage.storage, &test.database, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(
        report,
        SweepReport {
            deleted: 0,
            aborted: 0,
            kept: 3
        }
    );
    assert_eq!(garage.keys().await.len(), 3);
    assert_eq!(garage.open_uploads().await, 1);

    // After the grace, the orphan goes, the referenced object and the journal stay,
    // and the incomplete upload is aborted.
    let later = Timestamp::now() + SWEEP_GRACE + SignedDuration::from_mins(1);
    let report = sweep(&garage.storage, &test.database, later).await.unwrap();
    assert_eq!(
        report,
        SweepReport {
            deleted: 1,
            aborted: 1,
            kept: 2
        }
    );
    let mut expected = vec![referenced, JOURNAL.to_owned()];
    expected.sort();
    assert_eq!(garage.keys().await, expected);
    assert_eq!(garage.open_uploads().await, 0);
}

/// A redacted version keeps its row but not its object. The column comes with the redaction migration.
#[tokio::test]
#[ignore = "needs document_version.redacted_at"]
async fn sweep_deletes_the_object_of_a_redacted_version() {
    let test = TestDatabase::start().await;
    let garage = TestGarage::start().await;
    let (_, referenced) = uploaded(&test, &garage).await;
    test.scalar::<i64>(
        "WITH u AS (UPDATE document_version SET redacted_at = now() RETURNING 1) SELECT count(*) FROM u",
    )
    .await;

    let later = Timestamp::now() + SWEEP_GRACE + SignedDuration::from_mins(1);
    let report = sweep(&garage.storage, &test.database, later).await.unwrap();
    assert_eq!(report.deleted, 1, "{referenced}");
    assert!(garage.keys().await.is_empty());
}
