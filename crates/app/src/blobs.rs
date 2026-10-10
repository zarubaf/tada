//! The port of the object storage (ADR 0009).
//!
//! PostgreSQL owns names, versions and permissions. The store only holds bytes under keys.

use std::fmt::{self, Debug, Display};
use std::io;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use futures::stream::BoxStream;
use jiff::Timestamp;
use tada_domain::ids::{OrganizationId, RedactionId};
use uuid::Uuid;

/// The prefix of the objects of the deletion journal (ADR 0072). The sweep of orphan objects keeps them.
pub const JOURNAL_PREFIX: &str = "redaction-journal/";

/// A stream of bytes, for an upload or a download. Nothing holds the whole file in memory (ADR 0043).
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, io::Error>> + Send>>;

/// The key of an object: the organization ID, then a generated ID. Never a file name (ADR 0009).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobKey(String);

impl BlobKey {
    /// A new key in the organization.
    pub fn new(organization_id: OrganizationId) -> Self {
        Self(format!("{organization_id}/{}", Uuid::now_v7()))
    }

    /// For store adapters only: restores the key of a stored document version.
    /// Other code gets a key from `new`.
    #[doc(hidden)]
    pub fn restore(key: String) -> Self {
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key of the journal object of one redaction (ADR 0072):
    /// `redaction-journal/{organization_id}/{redaction_id}.json`.
    pub fn journal(organization_id: OrganizationId, redaction_id: RedactionId) -> Self {
        Self(format!(
            "{JOURNAL_PREFIX}{organization_id}/{redaction_id}.json"
        ))
    }

    /// The organization and the redaction of a key in the form that `journal` generates,
    /// or `None` for any other name.
    pub fn parse_journal(name: &str) -> Option<(OrganizationId, RedactionId)> {
        let (organization, redaction) = name
            .strip_prefix(JOURNAL_PREFIX)?
            .strip_suffix(".json")?
            .split_once('/')?;
        let organization = canonical_uuid(organization)?;
        let redaction = canonical_uuid(redaction)?;
        Some((
            OrganizationId::from_uuid(organization),
            RedactionId::from_uuid(redaction),
        ))
    }

    /// The key of an object in the form that `new` generates, or `None` for any other name,
    /// for example the objects under the prefix of the deletion journal.
    pub fn parse(name: &str) -> Option<Self> {
        let (organization, object) = name.split_once('/')?;
        canonical_uuid(organization)?;
        canonical_uuid(object)?;
        Some(Self(name.to_owned()))
    }
}

/// The UUID of `part` if `part` is its lowercase hyphenated form, the only form that a key holds.
fn canonical_uuid(part: &str) -> Option<Uuid> {
    Uuid::parse_str(part)
        .ok()
        .filter(|id| id.hyphenated().to_string() == part)
}

impl Display for BlobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An object in the listing of the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub key: String,
    pub last_modified: Timestamp,
}

/// A multipart upload that neither completed nor aborted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadInfo {
    pub key: String,
    pub upload_id: String,
    pub initiated: Timestamp,
}

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    /// The upload has more bytes than its limit. The store keeps nothing of it.
    #[error("the upload is larger than {limit} bytes")]
    TooLarge { limit: u64 },
    /// The stream of the upload failed, for example because the client disconnected.
    #[error("the upload stream failed")]
    Upload(#[source] io::Error),
    #[error("the object storage failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
pub trait BlobStore: Debug + Send + Sync {
    /// Writes the stream to `key` and returns its size. Above `limit` bytes, it stops and keeps nothing.
    /// Infrastructure query (ADR 0039): the key names the object, and the key starts with the organization ID.
    async fn put(&self, key: &BlobKey, body: ByteStream, limit: u64) -> Result<u64, BlobError>;

    /// Reads the object, or returns `None` if it does not exist.
    /// Infrastructure query (ADR 0039): the key names the object, and the key starts with the organization ID.
    async fn get(&self, key: &BlobKey) -> Result<Option<ByteStream>, BlobError>;

    /// The size of the object, or `None` if it does not exist.
    /// Infrastructure query (ADR 0039): the key names the object, and the key starts with the organization ID.
    async fn head(&self, key: &BlobKey) -> Result<Option<u64>, BlobError>;

    /// Infrastructure query (ADR 0039): the key names the object, and the key starts with the organization ID.
    async fn delete(&self, key: &BlobKey) -> Result<(), BlobError>;

    /// Streams all objects whose key starts with `prefix`, page by page.
    /// Infrastructure query (ADR 0039): the sweep of orphan objects reads the whole bucket, and the store has no organization scope.
    fn list(&self, prefix: &str) -> BoxStream<'static, Result<ObjectInfo, BlobError>>;

    /// All multipart uploads under `prefix` that neither completed nor aborted.
    /// Infrastructure query (ADR 0039): the sweep of orphan objects reads the whole bucket, and the store has no organization scope.
    async fn list_incomplete_uploads(&self, prefix: &str) -> Result<Vec<UploadInfo>, BlobError>;

    /// Aborts the multipart upload and deletes its parts.
    /// Infrastructure query (ADR 0039): the key names the object, and the key starts with the organization ID.
    async fn abort_upload(&self, key: &str, upload_id: &str) -> Result<(), BlobError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_keys_that_new_generates() {
        let key = BlobKey::new(OrganizationId::from_uuid(Uuid::now_v7()));
        assert_eq!(BlobKey::parse(key.as_str()), Some(key));
        for name in [
            "redaction-journal/2030-05-18.json",
            "redaction-journal/a/b",
            "not-a-uuid/not-a-uuid",
            "",
            "0198f2b0-0000-7000-8000-000000000000",
            "0198f2b0-0000-7000-8000-000000000000/0198F2B0-0000-7000-8000-000000000001",
            "0198f2b0-0000-7000-8000-000000000000/0198f2b0-0000-7000-8000-000000000001/x",
        ] {
            assert_eq!(BlobKey::parse(name), None, "{name}");
        }
    }

    #[test]
    fn a_journal_key_names_its_organization_and_redaction() {
        let organization = OrganizationId::from_uuid(Uuid::now_v7());
        let redaction = RedactionId::from_uuid(Uuid::now_v7());
        let key = BlobKey::journal(organization, redaction);
        assert_eq!(
            key.as_str(),
            format!("redaction-journal/{organization}/{redaction}.json")
        );
        assert_eq!(
            BlobKey::parse_journal(key.as_str()),
            Some((organization, redaction))
        );
        // The sweep of orphan objects never takes a journal object for an upload.
        assert_eq!(BlobKey::parse(key.as_str()), None);
        for name in [
            "redaction-journal/2030-05-18.json",
            &format!("redaction-journal/{organization}/{redaction}"),
            &format!("redaction-journal/{organization}/{redaction}.json/x"),
            &format!("{organization}/{redaction}.json"),
            &format!(
                "redaction-journal/{organization}/{}.json",
                redaction.as_uuid().simple()
            ),
        ] {
            assert_eq!(BlobKey::parse_journal(name), None, "{name}");
        }
    }
}
