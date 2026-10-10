//! The sweep of orphan objects in the object storage (ADR 0074).
//!
//! An upload writes the object first and the document version second (ADR 0009),
//! so a failure between the two steps leaves an object that no version refers to.
//! The sweep deletes such objects and aborts old incomplete multipart uploads.

use std::collections::HashSet;
use std::fmt::Debug;

use async_trait::async_trait;
use futures::TryStreamExt;
use jiff::{SignedDuration, Timestamp};

use crate::blobs::{BlobError, BlobKey, BlobStore};
use crate::store::StoreError;

/// An object or upload younger than this stays. The grace covers an upload whose version row
/// is not committed yet when the listing starts.
pub const SWEEP_GRACE: SignedDuration = SignedDuration::from_hours(24);

/// The number of keys that one store query checks.
const BATCH: usize = 200;

/// What the database knows about the objects.
#[async_trait]
pub trait BlobReferences: Debug + Send + Sync {
    /// The keys among `keys` that a document version with content refers to.
    /// Infrastructure query (ADR 0039): the sweep checks the keys of all organizations, and each key starts with its organization ID.
    async fn live_blob_keys(&self, keys: &[String]) -> Result<HashSet<String>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SweepError {
    #[error(transparent)]
    Blobs(#[from] BlobError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// What one sweep did. `kept` counts the listed objects that stay.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    pub deleted: usize,
    pub aborted: usize,
    pub kept: usize,
}

/// Deletes each object with a key `{organization_id}/{uuid}` that is older than the grace and
/// that no document version with content refers to. Aborts each incomplete multipart upload
/// that began before the grace. Other objects, for example the deletion journal, stay.
pub async fn sweep(
    blobs: &dyn BlobStore,
    references: &dyn BlobReferences,
    now: Timestamp,
) -> Result<SweepReport, SweepError> {
    let cutoff = now.saturating_sub(SWEEP_GRACE).unwrap_or(Timestamp::MIN);
    let mut report = SweepReport::default();
    let mut candidates: Vec<BlobKey> = Vec::new();
    let mut objects = blobs.list("");
    while let Some(object) = objects.try_next().await? {
        match BlobKey::parse(&object.key) {
            Some(key) if object.last_modified < cutoff => candidates.push(key),
            _ => report.kept += 1,
        }
        if candidates.len() >= BATCH {
            delete_orphans(blobs, references, &mut candidates, &mut report).await?;
        }
    }
    delete_orphans(blobs, references, &mut candidates, &mut report).await?;

    for upload in blobs.list_incomplete_uploads("").await? {
        if upload.initiated < cutoff {
            blobs.abort_upload(&upload.key, &upload.upload_id).await?;
            report.aborted += 1;
        }
    }
    Ok(report)
}

/// Deletes the candidates that no version refers to, and empties the list.
async fn delete_orphans(
    blobs: &dyn BlobStore,
    references: &dyn BlobReferences,
    candidates: &mut Vec<BlobKey>,
    report: &mut SweepReport,
) -> Result<(), SweepError> {
    if candidates.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = candidates
        .iter()
        .map(|key| key.as_str().to_owned())
        .collect();
    let live = references.live_blob_keys(&names).await?;
    for key in candidates.drain(..) {
        if live.contains(key.as_str()) {
            report.kept += 1;
        } else {
            blobs.delete(&key).await?;
            report.deleted += 1;
        }
    }
    Ok(())
}
