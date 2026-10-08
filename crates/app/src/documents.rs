//! Documents and their versions (ADR 0009, ADR 0043, ADR 0051, ADR 0055).
//!
//! A document belongs to one event, and access follows the event role (OP9).
//! Its readable ID `DOC-<n>` is local to the organization (ADR 0038).
//! An upload becomes a document version and a source version of the kind `upload` (ADR 0050).

use std::fmt::{self, Debug};

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::{DocumentId, DocumentVersionId, EventId, SourceVersionId, UserId};
use tada_domain::sources::SourceText;

use crate::audit::AuditEvent;
use crate::blobs::BlobKey;
use crate::caller::{Actor, OrgScope};
use crate::store::StoreError;
use crate::uploads::detect::FileType;

/// The prefix of the readable ID of a document (ADR 0038).
const DOCUMENT_PREFIX: &str = "DOC";

/// A document with its newest version.
#[derive(Clone, PartialEq, Eq)]
pub struct DocumentView {
    pub id: DocumentId,
    pub event_id: EventId,
    /// The number of the readable ID `DOC-<n>`, unique in the organization.
    pub local_number: u64,
    /// The file name of the first upload.
    pub name: String,
    /// The member who created the document.
    pub owner: UserId,
    pub created_at: Timestamp,
    /// The record version. It increases with each new document version.
    pub version: RecordVersion,
    pub newest_version: VersionView,
}

impl DocumentView {
    /// The readable ID, for example `DOC-001` (ADR 0038).
    pub fn readable_id(&self) -> String {
        format!("{DOCUMENT_PREFIX}-{:03}", self.local_number)
    }
}

/// The name can contain personal data, so `Debug` leaves it out (ADR 0035).
impl Debug for DocumentView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentView")
            .field("id", &self.id)
            .field("event_id", &self.event_id)
            .field("local_number", &self.local_number)
            .field("owner", &self.owner)
            .field("created_at", &self.created_at)
            .field("version", &self.version)
            .field("newest_version", &self.newest_version)
            .finish_non_exhaustive()
    }
}

/// One immutable version of a document: an uploaded file.
#[derive(Clone, PartialEq, Eq)]
pub struct VersionView {
    pub id: DocumentVersionId,
    pub document_id: DocumentId,
    /// 1 for the first version of the document, then 2, 3 and so on.
    pub number: u32,
    /// The original file name, after `sanitize_file_name`.
    pub file_name: String,
    pub file_type: FileType,
    pub size_bytes: u64,
    /// The SHA-256 hash of the file.
    pub sha256: [u8; 32],
    pub uploaded_by: UserId,
    /// The source version of the kind `upload` that holds the same file (ADR 0050).
    pub source_version_id: SourceVersionId,
    pub created_at: Timestamp,
}

/// The file name can contain personal data, so `Debug` leaves it out (ADR 0035).
impl Debug for VersionView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersionView")
            .field("id", &self.id)
            .field("document_id", &self.document_id)
            .field("number", &self.number)
            .field("file_type", &self.file_type)
            .field("size_bytes", &self.size_bytes)
            .field("uploaded_by", &self.uploaded_by)
            .field("source_version_id", &self.source_version_id)
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// A stored version with what a download needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVersion {
    pub event_id: EventId,
    pub version: VersionView,
    pub blob_key: BlobKey,
}

/// The position after the last document of a page (ADR 0044).
/// The documents are in the order of their readable IDs, the newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentCursor(pub u64);

/// The document that an upload adds a version to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadTarget {
    /// A new document in the event. Its name is the file name.
    New {
        id: DocumentId,
        event_id: EventId,
    },
    Existing(DocumentId),
}

/// A checked upload in the object storage, ready to publish.
#[derive(Clone, PartialEq, Eq)]
pub struct NewUpload {
    pub target: UploadTarget,
    pub version_id: DocumentVersionId,
    pub source_version_id: SourceVersionId,
    pub blob_key: BlobKey,
    pub file_name: String,
    pub file_type: FileType,
    pub size_bytes: u64,
    pub sha256: [u8; 32],
    /// The normalized text of a text file, for the search and for passages (OP10).
    pub text: Option<SourceText>,
    /// The author of the source version.
    pub author: Actor,
    pub uploaded_by: UserId,
    pub created_at: Timestamp,
}

/// The file name and the text can contain personal data, so `Debug` leaves them out (ADR 0035).
impl Debug for NewUpload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewUpload")
            .field("target", &self.target)
            .field("version_id", &self.version_id)
            .field("source_version_id", &self.source_version_id)
            .field("blob_key", &self.blob_key)
            .field("file_type", &self.file_type)
            .field("size_bytes", &self.size_bytes)
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// The result of `DocumentStore::publish`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    /// The document with the new version as its newest version.
    Published(Box<DocumentView>),
    /// The versions of the organization and the new file together are larger than the storage quota.
    /// The store changed nothing.
    QuotaExceeded,
    /// The existing document of the upload is not in the organization. The store changed nothing.
    NotFound,
}

/// The repository port for documents and their versions. Each method stays inside `scope`.
#[async_trait]
pub trait DocumentStore: Debug + Send + Sync {
    /// Publishes an upload in one transaction, or changes nothing.
    ///
    /// It checks the storage quota of the organization, with a lock, so that concurrent uploads cannot pass it together.
    /// A new document gets the next number `DOC-<n>` of the organization.
    /// It stores the document version, a source version of the kind `upload` and `audit`.
    async fn publish(
        &self,
        scope: OrgScope,
        upload: &NewUpload,
        audit: &AuditEvent,
    ) -> Result<Published, StoreError>;

    /// The documents of the event, the newest first, after the cursor `after`.
    /// With `name`, only the documents whose name contains it, without regard to case.
    async fn list(
        &self,
        scope: OrgScope,
        event: EventId,
        name: Option<&str>,
        after: Option<DocumentCursor>,
        limit: u32,
    ) -> Result<Vec<DocumentView>, StoreError>;

    async fn get(
        &self,
        scope: OrgScope,
        id: DocumentId,
    ) -> Result<Option<DocumentView>, StoreError>;

    /// The versions of the document in the order of their numbers. Empty if the document is not in the organization.
    async fn versions(
        &self,
        scope: OrgScope,
        document: DocumentId,
    ) -> Result<Vec<VersionView>, StoreError>;

    async fn version(
        &self,
        scope: OrgScope,
        id: DocumentVersionId,
    ) -> Result<Option<StoredVersion>, StoreError>;
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn document(local_number: u64) -> DocumentView {
        let id = DocumentId::from_uuid(Uuid::from_u128(1));
        DocumentView {
            id,
            event_id: EventId::from_uuid(Uuid::from_u128(2)),
            local_number,
            name: "Programm.pdf".to_owned(),
            owner: UserId::from_uuid(Uuid::from_u128(3)),
            created_at: Timestamp::UNIX_EPOCH,
            version: RecordVersion::FIRST,
            newest_version: VersionView {
                id: DocumentVersionId::from_uuid(Uuid::from_u128(4)),
                document_id: id,
                number: 1,
                file_name: "Programm.pdf".to_owned(),
                file_type: FileType::Pdf,
                size_bytes: 10,
                sha256: [0; 32],
                uploaded_by: UserId::from_uuid(Uuid::from_u128(3)),
                source_version_id: SourceVersionId::from_uuid(Uuid::from_u128(5)),
                created_at: Timestamp::UNIX_EPOCH,
            },
        }
    }

    #[test]
    fn a_readable_id_has_at_least_three_digits() {
        assert_eq!(document(1).readable_id(), "DOC-001");
        assert_eq!(document(42).readable_id(), "DOC-042");
        assert_eq!(document(1234).readable_id(), "DOC-1234");
    }

    #[test]
    fn debug_shows_no_file_name() {
        let shown = format!("{:?}", document(1));
        assert!(!shown.contains("Programm"), "{shown}");
    }
}
