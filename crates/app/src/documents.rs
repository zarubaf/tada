//! Documents and their versions (ADR 0009, ADR 0043, ADR 0051, ADR 0055).
//!
//! In Slice 1, a document belongs to one event, and access follows the event role (ADR 0052, ARCHITECTURE.md).
//! Its readable ID `DOC-<n>` is local to the organization (ADR 0038).
//! An upload becomes a document version and a source version of the kind `upload` (ADR 0050).
//!
//! The upload computes the SHA-256 hash of the file with `sha2`.
//! `sha2` is a pure crate without I/O, so it is a dependency of `app` and not of an adapter.

use std::borrow::Cow;
use std::fmt::{self, Debug};
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use futures::TryStreamExt;
use jiff::Timestamp;
use sha2::{Digest, Sha256};
use tada_domain::RecordVersion;
use tada_domain::ids::{
    DocumentId, DocumentVersionId, EventId, LocalIdKind, SourceVersionId, UserId,
};
use tada_domain::sources::SourceText;
use uuid::Uuid;

use crate::access::{self, AccessError, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::blobs::{BlobError, BlobKey, BlobStore, ByteStream};
use crate::caller::{Actor, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;
use crate::uploads::detect::{
    FileType, Rejected, SNIFF_BYTES, TextValidator, detect, sanitize_file_name,
};

/// A document with its newest version.
#[derive(Clone, PartialEq, Eq)]
pub struct DocumentView {
    pub id: DocumentId,
    pub event_id: EventId,
    /// The number of the readable ID `DOC-<n>`, unique in the organization.
    pub local_number: u64,
    /// The file name of the first upload, or the name that the draft proposal of a new document gave.
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
        LocalIdKind::Document.readable_id(self.local_number)
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

/// One immutable version of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionView {
    pub id: DocumentVersionId,
    pub document_id: DocumentId,
    /// 1 for the first version of the document, then 2, 3 and so on.
    pub number: u32,
    /// The SHA-256 hash of the content.
    pub sha256: [u8; 32],
    /// The member who added the version.
    pub uploaded_by: UserId,
    pub created_at: Timestamp,
    pub content: VersionContent,
}

impl VersionView {
    /// The file of an upload version.
    pub fn file(&self) -> Option<&UploadedFile> {
        match &self.content {
            VersionContent::Upload(file) => Some(file),
            VersionContent::Draft { .. } => None,
        }
    }
}

/// What a document version holds. Each kind has its own fields (ADR 0051).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionContent {
    Upload(UploadedFile),
    /// A draft in Markdown, added by the acceptance of a draft proposal.
    Draft {
        status: DraftStatus,
    },
}

/// The status of a draft version (ADR 0051).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    Draft,
    Review,
    Approved,
    Superseded,
    Archived,
}

impl DraftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Review => "review",
            Self::Approved => "approved",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Draft,
            Self::Review,
            Self::Approved,
            Self::Superseded,
            Self::Archived,
        ]
        .into_iter()
        .find(|status| status.as_str() == name)
    }
}

/// The file of an upload version.
#[derive(Clone, PartialEq, Eq)]
pub struct UploadedFile {
    /// The original file name, after `sanitize_file_name`.
    pub file_name: String,
    pub file_type: FileType,
    pub size_bytes: u64,
    /// The source version of the kind `upload` that holds the same file (ADR 0050).
    pub source_version_id: SourceVersionId,
}

/// The file name can contain personal data, so `Debug` leaves it out (ADR 0035).
impl Debug for UploadedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UploadedFile")
            .field("file_type", &self.file_type)
            .field("size_bytes", &self.size_bytes)
            .field("source_version_id", &self.source_version_id)
            .finish_non_exhaustive()
    }
}

/// A stored upload version with what a download needs.
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
    /// The normalized text of a text file, for the search and for passages (ADR 0050).
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

    /// The upload version `id` with its object. A draft has no file, so its ID gives `None`.
    async fn version(
        &self,
        scope: OrgScope,
        id: DocumentVersionId,
    ) -> Result<Option<StoredVersion>, StoreError>;
}

/// The largest text file whose text tada stores for the search and for passages (ADR 0050, ARCHITECTURE.md).
/// The cap bounds the memory of each upload. A larger text file is stored without its text.
/// Under the cap, the store still drops the searchable text if its search index exceeds the PostgreSQL limit.
const MAX_SOURCE_TEXT_BYTES: usize = 1024 * 1024;

/// What an upload learns from its stream while the object storage reads it (ADR 0043).
/// It never holds more than the first `MAX_SOURCE_TEXT_BYTES` of the file.
#[derive(Default)]
struct Inspection {
    /// The first bytes of the file.
    prefix: Vec<u8>,
    /// True if the file has more bytes than `prefix`.
    truncated: bool,
    hasher: Sha256,
    text: TextValidator,
}

impl Debug for Inspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inspection")
            .field("prefix_len", &self.prefix.len())
            .field("truncated", &self.truncated)
            .finish_non_exhaustive()
    }
}

impl Inspection {
    fn feed(&mut self, chunk: &[u8]) {
        self.hasher.update(chunk);
        self.text.feed(chunk);
        let room = MAX_SOURCE_TEXT_BYTES - self.prefix.len();
        self.prefix
            .extend_from_slice(&chunk[..room.min(chunk.len())]);
        self.truncated |= chunk.len() > room;
    }

    /// The type of the file, its hash, and its normalized text if it is a text file that is not too large.
    fn finish(self, file_name: &str) -> Result<(FileType, [u8; 32], Option<SourceText>), Rejected> {
        let extension = file_name.rsplit_once('.').map(|(_, extension)| extension);
        let head = &self.prefix[..self.prefix.len().min(SNIFF_BYTES)];
        let file_type = detect(head, extension, self.text.finish())?;
        let text = (file_type.is_text() && !self.truncated)
            .then(|| String::from_utf8(self.prefix).ok())
            .flatten()
            .map(|text| SourceText::normalize(&text));
        Ok((file_type, self.hasher.finalize().into(), text))
    }
}

/// The ports that the document commands and queries use.
#[derive(Debug, Clone, Copy)]
pub struct DocumentStores<'a> {
    pub identity: &'a dyn IdentityStore,
    pub documents: &'a dyn DocumentStore,
    pub blobs: &'a dyn BlobStore,
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    /// The event or the document is not in the caller's organization, or the caller has no event role in its event.
    #[error("the event or the document does not exist or the caller cannot see it")]
    NotFound,
    /// A viewer cannot upload (ADR 0052).
    #[error("the caller cannot upload here")]
    Forbidden,
    /// The file is larger than the upload limit (ADR 0043).
    #[error("the file is larger than the limit")]
    TooLarge,
    /// The content of the file does not match an allowed type (ADR 0043, ADR 0055).
    #[error("the file type is not allowed")]
    UnsupportedType,
    /// The files of the organization and the new file are larger than its storage quota (ADR 0043).
    #[error("the storage quota is exceeded")]
    QuotaExceeded,
    /// The stream of the upload failed, for example because the client disconnected.
    #[error("the upload stream failed")]
    Interrupted(#[source] io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl UploadError {
    /// All codes that the upload commands can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::MalformedRequest,
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::PayloadTooLarge,
        ProblemCode::UnsupportedMediaType,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

/// The entry of `validation-failed` for an upload over the storage quota.
static QUOTA_EXCEEDED: [FieldError; 1] = [FieldError {
    field: Cow::Borrowed("file"),
    code: "quota-exceeded",
}];

impl CommandError for UploadError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::TooLarge => ProblemCode::PayloadTooLarge,
            Self::UnsupportedType => ProblemCode::UnsupportedMediaType,
            Self::QuotaExceeded => ProblemCode::ValidationFailed,
            Self::Interrupted(_) => ProblemCode::MalformedRequest,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::QuotaExceeded => &QUOTA_EXCEEDED,
            _ => &[],
        }
    }
}

impl From<AccessError> for UploadError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

impl From<BlobError> for UploadError {
    fn from(error: BlobError) -> Self {
        match error {
            BlobError::TooLarge { .. } => Self::TooLarge,
            BlobError::Upload(error) => Self::Interrupted(error),
            BlobError::Storage(error) => Self::Store(StoreError::Unavailable(error)),
        }
    }
}

/// Uploads a file as the first version of a new document in the event `event_id`.
/// See `upload_version` for the steps. The name of the document is the file name.
pub async fn upload_document(
    caller: &MemberCaller,
    event_id: EventId,
    file_name: &str,
    body: ByteStream,
    limit: u64,
    stores: DocumentStores<'_>,
    clock: &dyn Clock,
) -> Result<DocumentView, UploadError> {
    check_may_upload(caller, event_id, stores.identity).await?;
    let target = UploadTarget::New {
        id: DocumentId::from_uuid(Uuid::now_v7()),
        event_id,
    };
    upload(caller, target, file_name, body, limit, stores, clock).await
}

/// Uploads a file as a new version of the document `document_id`. The older versions stay unchanged (ADR 0009).
///
/// 1. The caller needs the right to propose in the event of the document: a contributor or a manager (ADR 0052).
/// 2. The stream goes to a new key in the object storage. On the way, tada keeps the first bytes,
///    computes the SHA-256 hash and checks if the file is UTF-8 text (ADR 0043).
/// 3. The content gives the type (ADR 0055).
/// 4. The store checks the storage quota and publishes the version, its source version and an audit event in one transaction.
///
/// If a step fails, nothing is stored. A rejected upload also deletes its object.
/// After a store error, the outcome of the commit is unknown, so the object stays.
pub async fn upload_version(
    caller: &MemberCaller,
    document_id: DocumentId,
    file_name: &str,
    body: ByteStream,
    limit: u64,
    stores: DocumentStores<'_>,
    clock: &dyn Clock,
) -> Result<DocumentView, UploadError> {
    let document = stores
        .documents
        .get(caller.scope(), document_id)
        .await?
        .ok_or(UploadError::NotFound)?;
    check_may_upload(caller, document.event_id, stores.identity).await?;
    let target = UploadTarget::Existing(document_id);
    upload(caller, target, file_name, body, limit, stores, clock).await
}

async fn check_may_upload(
    caller: &MemberCaller,
    event_id: EventId,
    identity: &dyn IdentityStore,
) -> Result<(), UploadError> {
    if access::event_access(caller, event_id, identity)
        .await?
        .can_propose()
    {
        Ok(())
    } else {
        Err(UploadError::Forbidden)
    }
}

/// Streams the file to a new key, checks it and publishes it.
/// It deletes the object after a definite rejection, and keeps it after a store error of the publish.
async fn upload(
    caller: &MemberCaller,
    target: UploadTarget,
    file_name: &str,
    body: ByteStream,
    limit: u64,
    stores: DocumentStores<'_>,
    clock: &dyn Clock,
) -> Result<DocumentView, UploadError> {
    let file_name = sanitize_file_name(file_name);
    let blob_key = BlobKey::new(caller.scope().organization_id());
    let inspection = Arc::new(Mutex::new(Inspection::default()));
    let tap = Arc::clone(&inspection);
    let body: ByteStream = Box::pin(body.inspect_ok(move |chunk| {
        tap.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .feed(chunk);
    }));
    let size_bytes = match stores.blobs.put(&blob_key, body, limit).await {
        Ok(size_bytes) => size_bytes,
        Err(error) => {
            discard(stores.blobs, &blob_key).await;
            return Err(error.into());
        }
    };
    let inspection =
        std::mem::take(&mut *inspection.lock().unwrap_or_else(PoisonError::into_inner));
    let result = match inspection.finish(&file_name) {
        Ok((file_type, sha256, text)) => {
            let upload = NewUpload {
                target,
                version_id: DocumentVersionId::from_uuid(Uuid::now_v7()),
                source_version_id: SourceVersionId::from_uuid(Uuid::now_v7()),
                blob_key: blob_key.clone(),
                file_name,
                file_type,
                size_bytes,
                sha256,
                text,
                author: caller.actor(),
                uploaded_by: caller.user_id(),
                created_at: clock.now(),
            };
            publish(caller, &upload, stores.documents).await
        }
        Err(Rejected) => Err(UploadError::UnsupportedType),
    };
    match &result {
        // A store error leaves the outcome of the commit unknown: the version can exist, so its object stays (ADR 0009).
        // An object without a version is harmless; a version without its object loses evidence.
        Ok(_) | Err(UploadError::Store(_)) => {}
        // Each other error is a definite rejection, and no version refers to the object.
        Err(_) => discard(stores.blobs, &blob_key).await,
    }
    result
}

/// Deletes the object of a rejected upload.
/// A failed delete leaves an object without a version. No code reads it, and the original error matters more.
async fn discard(blobs: &dyn BlobStore, key: &BlobKey) {
    let _ = blobs.delete(key).await;
}

/// Publishes the checked upload as a new version with an audit event.
async fn publish(
    caller: &MemberCaller,
    upload: &NewUpload,
    documents: &dyn DocumentStore,
) -> Result<DocumentView, UploadError> {
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::DocumentVersionUpload,
        Some(upload.version_id.as_uuid()),
        Some(caller.scope()),
    );
    match documents.publish(caller.scope(), upload, &audit).await? {
        Published::Published(document) => Ok(*document),
        Published::QuotaExceeded => Err(UploadError::QuotaExceeded),
        Published::NotFound => Err(UploadError::NotFound),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadDocumentError {
    /// The event, the document or the version is not in the caller's organization, or the caller has no event role in its event.
    #[error("the record does not exist or the caller cannot see it")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ReadDocumentError {
    /// All codes that the document queries can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ReadDocumentError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            Self::NotFound => None,
        }
    }
}

impl From<AccessError> for ReadDocumentError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

/// The object of a published version is missing from the object storage.
#[derive(Debug, thiserror::Error)]
#[error("the object of a published document version is missing")]
struct MissingObject;

/// Fails with `NotFound` if the caller cannot read the event (ADR 0052).
async fn check_may_read(
    caller: &impl Principal,
    event_id: EventId,
    identity: &dyn IdentityStore,
) -> Result<(), ReadDocumentError> {
    if access::event_access(caller, event_id, identity)
        .await?
        .can_read()
    {
        Ok(())
    } else {
        Err(ReadDocumentError::NotFound)
    }
}

/// The documents of the event, the newest first.
/// With `name`, only the documents whose name contains it, without regard to case.
pub async fn list_documents(
    caller: &impl Principal,
    event_id: EventId,
    name: Option<&str>,
    after: Option<DocumentCursor>,
    limit: PageLimit,
    stores: DocumentStores<'_>,
) -> Result<Page<DocumentView, DocumentCursor>, ReadDocumentError> {
    check_may_read(caller, event_id, stores.identity).await?;
    let name = name.map(str::trim).filter(|name| !name.is_empty());
    // One more than the limit shows if a next page exists.
    let mut items = stores
        .documents
        .list(caller.scope(), event_id, name, after, limit.get() + 1)
        .await?;
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| DocumentCursor(last.local_number));
    Ok(Page { items, next })
}

/// The document `id` with its newest version.
pub async fn get_document(
    caller: &impl Principal,
    id: DocumentId,
    stores: DocumentStores<'_>,
) -> Result<DocumentView, ReadDocumentError> {
    let document = stores
        .documents
        .get(caller.scope(), id)
        .await?
        .ok_or(ReadDocumentError::NotFound)?;
    check_may_read(caller, document.event_id, stores.identity).await?;
    Ok(document)
}

/// The versions of the document `id`, in the order of their numbers.
pub async fn list_versions(
    caller: &impl Principal,
    id: DocumentId,
    stores: DocumentStores<'_>,
) -> Result<Vec<VersionView>, ReadDocumentError> {
    get_document(caller, id, stores).await?;
    Ok(stores.documents.versions(caller.scope(), id).await?)
}

/// The upload version `id` and the stream of its file. A draft version has no file: it is not found here.
pub async fn download(
    caller: &impl Principal,
    id: DocumentVersionId,
    stores: DocumentStores<'_>,
) -> Result<(VersionView, ByteStream), ReadDocumentError> {
    let stored = stores
        .documents
        .version(caller.scope(), id)
        .await?
        .ok_or(ReadDocumentError::NotFound)?;
    check_may_read(caller, stored.event_id, stores.identity).await?;
    let body = stores
        .blobs
        .get(&stored.blob_key)
        .await
        .map_err(|error| match error {
            BlobError::Storage(error) => StoreError::Unavailable(error),
            error => StoreError::Internal(Box::new(error)),
        })?
        .ok_or_else(|| StoreError::Internal(Box::new(MissingObject)))?;
    Ok((stored.version, body))
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
                sha256: [0; 32],
                uploaded_by: UserId::from_uuid(Uuid::from_u128(3)),
                created_at: Timestamp::UNIX_EPOCH,
                content: VersionContent::Upload(UploadedFile {
                    file_name: "Programm.pdf".to_owned(),
                    file_type: FileType::Pdf,
                    size_bytes: 10,
                    source_version_id: SourceVersionId::from_uuid(Uuid::from_u128(5)),
                }),
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
    fn each_draft_status_has_its_name() {
        for name in ["draft", "review", "approved", "superseded", "archived"] {
            assert_eq!(
                DraftStatus::parse(name).map(DraftStatus::as_str),
                Some(name)
            );
        }
        assert_eq!(DraftStatus::parse("upload"), None);
    }

    #[test]
    fn debug_shows_no_file_name() {
        let shown = format!("{:?}", document(1));
        assert!(!shown.contains("Programm"), "{shown}");
    }

    fn inspect(
        chunks: &[&[u8]],
        file_name: &str,
    ) -> Result<(FileType, [u8; 32], Option<SourceText>), Rejected> {
        let mut inspection = Inspection::default();
        for chunk in chunks {
            inspection.feed(chunk);
        }
        inspection.finish(file_name)
    }

    #[test]
    fn hashes_the_whole_stream_across_chunks() {
        let content = b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\n%%EOF\n";
        let (file_type, sha256, text) = inspect(
            &[&content[..5], &content[5..12], &content[12..]],
            "Programm.pdf",
        )
        .unwrap();
        assert_eq!(file_type, FileType::Pdf);
        assert_eq!(sha256, <[u8; 32]>::from(Sha256::digest(content)));
        assert_eq!(text, None, "a PDF has no text in Slice 1");
    }

    #[test]
    fn keeps_the_normalized_text_of_a_text_file() {
        let (file_type, _, text) =
            inspect(&[b"Flugshow\r\num 14 ", b"Uhr\r\n"], "Notizen.md").unwrap();
        assert_eq!(file_type, FileType::Markdown);
        assert_eq!(text.unwrap().as_str(), "Flugshow\num 14 Uhr\n");
    }

    #[test]
    fn keeps_no_text_of_a_text_file_over_the_text_limit() {
        let line = [b'a'; 1000];
        let mut inspection = Inspection::default();
        while inspection.prefix.len() < MAX_SOURCE_TEXT_BYTES {
            inspection.feed(&line);
        }
        assert_eq!(
            inspection.prefix.len(),
            MAX_SOURCE_TEXT_BYTES,
            "the prefix never grows over the limit"
        );
        let (file_type, _, text) = inspection.finish("Liste.csv").unwrap();
        assert_eq!(file_type, FileType::Csv);
        assert_eq!(text, None);
    }

    #[test]
    fn rejects_a_renamed_executable() {
        let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
        elf.resize(4096, 0);
        assert_eq!(inspect(&[&elf], "Programm.pdf"), Err(Rejected));
        assert_eq!(inspect(&[&elf], "Programm.txt"), Err(Rejected));
    }

    #[test]
    fn each_error_has_a_code_in_its_list() {
        let upload_errors = [
            UploadError::NotFound,
            UploadError::Forbidden,
            UploadError::TooLarge,
            UploadError::UnsupportedType,
            UploadError::QuotaExceeded,
            UploadError::Interrupted(io::Error::other("test")),
            UploadError::Store(StoreError::Unavailable("test".into())),
            UploadError::Store(StoreError::Internal("test".into())),
        ];
        for error in upload_errors {
            assert!(UploadError::CODES.contains(&error.code()), "{error:?}");
        }
        for error in [
            ReadDocumentError::NotFound,
            ReadDocumentError::Store(StoreError::Unavailable("test".into())),
            ReadDocumentError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(
                ReadDocumentError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
    }

    #[test]
    fn quota_exceeded_names_its_entry_code() {
        let error = UploadError::QuotaExceeded;
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
        let codes: Vec<_> = error
            .field_errors()
            .iter()
            .map(|entry| entry.code)
            .collect();
        assert_eq!(codes, ["quota-exceeded"]);
    }

    #[test]
    fn a_failed_object_storage_is_unavailable() {
        let error = UploadError::from(BlobError::Storage("test".into()));
        assert_eq!(error.code(), ProblemCode::Unavailable);
        assert!(error.store_error().is_some(), "the API logs it");
        assert_eq!(
            UploadError::from(BlobError::TooLarge { limit: 1 }).code(),
            ProblemCode::PayloadTooLarge
        );
    }
}
