//! The port of the object storage (ADR 0009).
//!
//! PostgreSQL owns names, versions and permissions. The store only holds bytes under keys.

use std::fmt::{self, Debug, Display};
use std::io;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use tada_domain::ids::OrganizationId;
use uuid::Uuid;

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
}

impl Display for BlobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
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
}
