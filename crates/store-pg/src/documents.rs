//! The `DocumentStore` adapter (ADR 0009, ADR 0051): documents, their versions and the storage quota.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use sqlx::{Connection, PgConnection};
use tada_app::audit::AuditEvent;
use tada_app::blobs::BlobKey;
use tada_app::caller::OrgScope;
use tada_app::documents::{
    Approval, Approved, DocumentCursor, DocumentStore, DocumentView, DraftStatus, NewUpload,
    Published, StoredDraft, StoredVersion, UploadTarget, UploadedFile, VersionContent, VersionView,
};
use tada_app::domain::RecordVersion;
use tada_app::domain::documents::DraftMarkdown;
use tada_app::domain::ids::{
    DocumentId, DocumentVersionId, EventId, FactId, LocalIdKind, SourceVersionId, UserId,
};
use tada_app::domain::sources::{Evidence, Passage, SourceText};
use tada_app::drafts::{CitedFact, ProvenanceManifest};
use tada_app::store::StoreError;
use tada_app::uploads::detect::FileType;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::{actor, audit};

/// The kind of a source item, a source version and a document version of an uploaded file.
const UPLOAD: &str = "upload";

/// The kind of a document version of a draft (ADR 0051).
pub(crate) const DRAFT: &str = "draft";

/// The kind of the counter of the organization-local document numbers (ADR 0038).
pub(crate) const DOCUMENT_COUNTER: &str = LocalIdKind::Document.prefix();

/// A document version row, with the event of its document.
struct VersionRow {
    id: Uuid,
    document_id: Uuid,
    event_id: Uuid,
    number: i32,
    kind: String,
    blob_key: Option<String>,
    media_type: Option<String>,
    size_bytes: Option<i64>,
    sha256: Vec<u8>,
    file_name: Option<String>,
    uploaded_by: Uuid,
    source_version_id: Option<Uuid>,
    status: Option<String>,
    created_at: jiff_sqlx::Timestamp,
}

impl VersionRow {
    /// The content of an upload row: its file.
    fn uploaded_file(&self) -> Result<UploadedFile, InvalidRow> {
        Ok(UploadedFile {
            file_name: self
                .file_name
                .clone()
                .ok_or(InvalidRow("document_version.file_name"))?,
            file_type: self
                .media_type
                .as_deref()
                .and_then(FileType::of_media_type)
                .ok_or(InvalidRow("document_version.media_type"))?,
            size_bytes: self
                .size_bytes
                .and_then(|size| u64::try_from(size).ok())
                .ok_or(InvalidRow("document_version.size_bytes"))?,
            source_version_id: self
                .source_version_id
                .map(SourceVersionId::from_uuid)
                .ok_or(InvalidRow("document_version.source_version_id"))?,
        })
    }

    fn content(&self) -> Result<VersionContent, InvalidRow> {
        match self.kind.as_str() {
            UPLOAD => Ok(VersionContent::Upload(self.uploaded_file()?)),
            DRAFT => Ok(VersionContent::Draft {
                status: self
                    .status
                    .as_deref()
                    .and_then(DraftStatus::parse)
                    .ok_or(InvalidRow("document_version.status"))?,
            }),
            _ => Err(InvalidRow("document_version.kind")),
        }
    }

    fn view(&self) -> Result<VersionView, InvalidRow> {
        Ok(VersionView {
            id: DocumentVersionId::from_uuid(self.id),
            document_id: DocumentId::from_uuid(self.document_id),
            number: u32::try_from(self.number)
                .map_err(|_| InvalidRow("document_version.number"))?,
            sha256: self
                .sha256
                .clone()
                .try_into()
                .map_err(|_| InvalidRow("document_version.sha256"))?,
            uploaded_by: UserId::from_uuid(self.uploaded_by),
            created_at: self.created_at.to_jiff(),
            content: self.content()?,
        })
    }

    /// The stored upload with its object, or `None` for a draft, which has no file.
    fn stored_upload(self) -> Result<Option<StoredVersion>, InvalidRow> {
        let version = self.view()?;
        if version.file().is_none() {
            return Ok(None);
        }
        Ok(Some(StoredVersion {
            event_id: EventId::from_uuid(self.event_id),
            version,
            blob_key: BlobKey::restore(
                self.blob_key
                    .ok_or(InvalidRow("document_version.blob_key"))?,
            ),
        }))
    }
}

/// A document row with the row of its newest version.
struct DocumentRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    name: String,
    owner_user_id: Uuid,
    created_at: jiff_sqlx::Timestamp,
    version: i64,
    version_id: Uuid,
    number: i32,
    kind: String,
    blob_key: Option<String>,
    media_type: Option<String>,
    size_bytes: Option<i64>,
    sha256: Vec<u8>,
    file_name: Option<String>,
    uploaded_by: Uuid,
    source_version_id: Option<Uuid>,
    status: Option<String>,
    version_created_at: jiff_sqlx::Timestamp,
}

impl TryFrom<DocumentRow> for DocumentView {
    type Error = InvalidRow;

    fn try_from(row: DocumentRow) -> Result<Self, InvalidRow> {
        let newest = VersionRow {
            id: row.version_id,
            document_id: row.id,
            event_id: row.event_id,
            number: row.number,
            kind: row.kind,
            blob_key: row.blob_key,
            media_type: row.media_type,
            size_bytes: row.size_bytes,
            sha256: row.sha256,
            file_name: row.file_name,
            uploaded_by: row.uploaded_by,
            source_version_id: row.source_version_id,
            status: row.status,
            created_at: row.version_created_at,
        }
        .view()?;
        Ok(DocumentView {
            id: DocumentId::from_uuid(row.id),
            event_id: EventId::from_uuid(row.event_id),
            local_number: u64::try_from(row.local_number)
                .map_err(|_| InvalidRow("document.local_number"))?,
            name: row.name,
            owner: UserId::from_uuid(row.owner_user_id),
            created_at: row.created_at.to_jiff(),
            version: RecordVersion::new(row.version).ok_or(InvalidRow("document.version"))?,
            newest_version: newest,
        })
    }
}

/// The filters of `documents`. Each filter that is `None` matches all documents of the organization.
#[derive(Default)]
struct DocumentFilter<'a> {
    id: Option<DocumentId>,
    event: Option<EventId>,
    name: Option<&'a str>,
    after: Option<DocumentCursor>,
    limit: Option<u32>,
}

/// The documents of the organization with their newest versions, the newest document first.
async fn documents(
    conn: &mut PgConnection,
    scope: OrgScope,
    filter: DocumentFilter<'_>,
) -> Result<Vec<DocumentView>, StoreError> {
    let after = filter
        .after
        .map(|cursor| i64::try_from(cursor.0).unwrap_or(i64::MAX));
    let rows = sqlx::query_as!(
        DocumentRow,
        r#"SELECT d.id, d.event_id, d.local_number, d.name, d.owner_user_id,
                  d.created_at AS "created_at: jiff_sqlx::Timestamp", d.version,
                  v.id AS "version_id!", v.number AS "number!", v.kind AS "kind!", v.blob_key,
                  v.media_type, v.size_bytes, v.sha256 AS "sha256!", v.file_name,
                  v.uploaded_by AS "uploaded_by!", v.source_version_id, v.status,
                  v.created_at AS "version_created_at!: jiff_sqlx::Timestamp"
           FROM document d
           JOIN LATERAL (
               SELECT * FROM document_version
               WHERE organization_id = d.organization_id AND document_id = d.id
               ORDER BY number DESC
               LIMIT 1
           ) v ON true
           WHERE d.organization_id = $1
             AND ($2::uuid IS NULL OR d.id = $2)
             AND ($3::uuid IS NULL OR d.event_id = $3)
             AND ($4::text IS NULL OR position(lower($4) IN lower(d.name)) > 0)
             AND ($5::bigint IS NULL OR d.local_number < $5)
           ORDER BY d.local_number DESC
           LIMIT $6"#,
        scope.organization_id().as_uuid(),
        filter.id.map(DocumentId::as_uuid),
        filter.event.map(EventId::as_uuid),
        filter.name,
        after,
        filter.limit.map(i64::from),
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    rows.into_iter()
        .map(|row| Ok(DocumentView::try_from(row)?))
        .collect()
}

/// The version rows of the organization, in the order of their documents and numbers.
async fn versions(
    conn: &mut PgConnection,
    scope: OrgScope,
    document: Option<DocumentId>,
    id: Option<DocumentVersionId>,
) -> Result<Vec<VersionRow>, StoreError> {
    sqlx::query_as!(
        VersionRow,
        r#"SELECT v.id, v.document_id, d.event_id, v.number, v.kind, v.blob_key, v.media_type,
                  v.size_bytes, v.sha256, v.file_name, v.uploaded_by, v.source_version_id, v.status,
                  v.created_at AS "created_at: jiff_sqlx::Timestamp"
           FROM document_version v
           JOIN document d ON d.organization_id = v.organization_id AND d.id = v.document_id
           WHERE v.organization_id = $1
             AND ($2::uuid IS NULL OR v.document_id = $2)
             AND ($3::uuid IS NULL OR v.id = $3)
           ORDER BY v.document_id, v.number"#,
        scope.organization_id().as_uuid(),
        document.map(DocumentId::as_uuid),
        id.map(DocumentVersionId::as_uuid),
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)
}

/// True if the files of the organization and `size` more bytes fit into its storage quota.
/// It locks the organization row until the end of the transaction, so concurrent uploads check one after the other.
///
/// It is the first lock of the transaction, as the lock order of the crate documentation requires.
async fn fits_quota(
    conn: &mut PgConnection,
    scope: OrgScope,
    size: u64,
) -> Result<bool, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let quota = sqlx::query_scalar!(
        "SELECT storage_quota_bytes FROM organization WHERE id = $1 FOR NO KEY UPDATE",
        organization,
    )
    .fetch_one(&mut *conn)
    .await?;
    let used = sqlx::query_scalar!(
        r#"SELECT coalesce(sum(size_bytes), 0)::bigint AS "used!"
           FROM document_version WHERE organization_id = $1"#,
        organization,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(i64::try_from(size).is_ok_and(|size| used.saturating_add(size) <= quota))
}

/// The document and the number of the new version, or `None` if an existing document is not in the organization.
/// A new document gets the next number of the organization from its counter row (ADR 0038).
/// An existing document gets its next record version; the update locks its row, so its version numbers stay unique.
async fn version_target(
    conn: &mut PgConnection,
    scope: OrgScope,
    upload: &NewUpload,
) -> Result<Option<(DocumentId, EventId, i32)>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    match upload.target {
        UploadTarget::New { id, event_id } => {
            let local_number = sqlx::query_scalar!(
                r#"INSERT INTO local_id_counter (organization_id, scope_id, kind, next)
                   VALUES ($1, $1, $2, 2)
                   ON CONFLICT (organization_id, scope_id, kind)
                   DO UPDATE SET next = local_id_counter.next + 1
                   RETURNING next - 1 AS "local_number!""#,
                organization,
                DOCUMENT_COUNTER,
            )
            .fetch_one(&mut *conn)
            .await?;
            sqlx::query!(
                "INSERT INTO document
                     (id, organization_id, event_id, local_number, name, owner_user_id, created_at, version)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
                id.as_uuid(),
                organization,
                event_id.as_uuid(),
                local_number,
                upload.file_name,
                upload.uploaded_by.as_uuid(),
                upload.created_at.to_sqlx() as _,
            )
            .execute(&mut *conn)
            .await?;
            Ok(Some((id, event_id, 1)))
        }
        UploadTarget::Existing(id) => {
            let Some(event) = sqlx::query_scalar!(
                "UPDATE document SET version = version + 1
                 WHERE organization_id = $1 AND id = $2
                 RETURNING event_id",
                organization,
                id.as_uuid(),
            )
            .fetch_optional(&mut *conn)
            .await?
            else {
                return Ok(None);
            };
            let number = sqlx::query_scalar!(
                r#"SELECT coalesce(max(number), 0) + 1 AS "number!"
                   FROM document_version WHERE organization_id = $1 AND document_id = $2"#,
                organization,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            Ok(Some((id, EventId::from_uuid(event), number)))
        }
    }
}

/// The source item of the uploads of the document: the item of its first upload, or a new item.
/// Each upload of a document is a version of the same source item (ADR 0050).
async fn source_item(
    conn: &mut PgConnection,
    scope: OrgScope,
    document: DocumentId,
    event: EventId,
    now: Timestamp,
) -> Result<Uuid, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let existing = sqlx::query_scalar!(
        "SELECT s.source_item_id
         FROM document_version v
         JOIN source_version s ON s.organization_id = v.organization_id AND s.id = v.source_version_id
         WHERE v.organization_id = $1 AND v.document_id = $2
         ORDER BY v.number
         LIMIT 1",
        organization,
        document.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(item) = existing {
        return Ok(item);
    }
    let item = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO source_item (id, organization_id, event_id, kind, created_at)
         VALUES ($1, $2, $3, $4, $5)",
        item,
        organization,
        event.as_uuid(),
        UPLOAD,
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(item)
}

/// True if the error is SQLSTATE 54000 `program_limit_exceeded`: here, the search index of a text is too long.
fn is_text_too_long(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "54000")
}

/// Writes the source version of `upload` with the text `text`.
async fn insert_source_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    upload: &NewUpload,
    item: Uuid,
    text: Option<&SourceText>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO source_version
             (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        upload.source_version_id.as_uuid(),
        scope.organization_id().as_uuid(),
        item,
        UPLOAD,
        upload.author.channel().as_str(),
        actor::to_json(&upload.author),
        text.map(SourceText::as_str),
        &upload.sha256[..],
        upload.created_at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Writes the source version and the document version of `upload`.
async fn insert_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    upload: &NewUpload,
    (document, event, number): (DocumentId, EventId, i32),
) -> Result<(), sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let item = source_item(conn, scope, document, event, upload.created_at).await?;
    // The search index of a text can exceed the PostgreSQL limit of 1 MB for a `tsvector` (SQLSTATE 54000),
    // for example in a CSV file of many unique numbers. Then the version keeps its file, without searchable text.
    let mut savepoint = conn.begin().await?;
    match insert_source_version(&mut savepoint, scope, upload, item, upload.text.as_ref()).await {
        Ok(()) => savepoint.commit().await?,
        Err(error) if is_text_too_long(&error) => {
            savepoint.rollback().await?;
            insert_source_version(conn, scope, upload, item, None).await?;
        }
        Err(error) => return Err(error),
    }
    let size =
        i64::try_from(upload.size_bytes).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query!(
        "INSERT INTO document_version
             (id, organization_id, document_id, number, kind, blob_key, media_type, size_bytes,
              sha256, file_name, uploaded_by, source_version_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        upload.version_id.as_uuid(),
        organization,
        document.as_uuid(),
        number,
        UPLOAD,
        upload.blob_key.as_str(),
        upload.file_type.media_type(),
        size,
        &upload.sha256[..],
        upload.file_name,
        upload.uploaded_by.as_uuid(),
        upload.source_version_id.as_uuid(),
        upload.created_at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[async_trait]
impl DocumentStore for Database {
    async fn publish(
        &self,
        scope: OrgScope,
        upload: &NewUpload,
        audit: &AuditEvent,
    ) -> Result<Published, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        if !fits_quota(&mut tx, scope, upload.size_bytes)
            .await
            .map_err(store_error)?
        {
            return Ok(Published::QuotaExceeded);
        }
        let Some(target) = version_target(&mut tx, scope, upload)
            .await
            .map_err(store_error)?
        else {
            return Ok(Published::NotFound);
        };
        insert_version(&mut tx, scope, upload, target)
            .await
            .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let filter = DocumentFilter {
            id: Some(target.0),
            ..DocumentFilter::default()
        };
        let document = documents(&mut tx, scope, filter)
            .await?
            .pop()
            .ok_or(InvalidRow("document"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Published::Published(Box::new(document)))
    }

    async fn list(
        &self,
        scope: OrgScope,
        event: EventId,
        name: Option<&str>,
        after: Option<DocumentCursor>,
        limit: u32,
    ) -> Result<Vec<DocumentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let filter = DocumentFilter {
            event: Some(event),
            name,
            after,
            limit: Some(limit),
            ..DocumentFilter::default()
        };
        documents(&mut conn, scope, filter).await
    }

    async fn get(
        &self,
        scope: OrgScope,
        id: DocumentId,
    ) -> Result<Option<DocumentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let filter = DocumentFilter {
            id: Some(id),
            ..DocumentFilter::default()
        };
        Ok(documents(&mut conn, scope, filter).await?.pop())
    }

    async fn versions(
        &self,
        scope: OrgScope,
        document: DocumentId,
    ) -> Result<Vec<VersionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        versions(&mut conn, scope, Some(document), None)
            .await?
            .iter()
            .map(|row| Ok(row.view()?))
            .collect()
    }

    async fn version(
        &self,
        scope: OrgScope,
        id: DocumentVersionId,
    ) -> Result<Option<StoredVersion>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        match versions(&mut conn, scope, None, Some(id)).await?.pop() {
            Some(row) => Ok(row.stored_upload()?),
            None => Ok(None),
        }
    }

    async fn draft(
        &self,
        scope: OrgScope,
        id: DocumentVersionId,
    ) -> Result<Option<StoredDraft>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let Some(row) = versions(&mut conn, scope, None, Some(id)).await?.pop() else {
            return Ok(None);
        };
        if row.kind != DRAFT {
            return Ok(None);
        }
        let organization = scope.organization_id().as_uuid();
        let markdown = sqlx::query_scalar!(
            r#"SELECT markdown AS "markdown!" FROM document_version
               WHERE organization_id = $1 AND id = $2 AND markdown IS NOT NULL"#,
            organization,
            id.as_uuid(),
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(store_error)?;
        let facts = sqlx::query!(
            "SELECT fact_id, fact_version_number FROM document_manifest_fact
             WHERE organization_id = $1 AND document_version_id = $2
             ORDER BY fact_id, fact_version_number",
            organization,
            id.as_uuid(),
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(store_error)?;
        // The offsets count characters of the normalized text, and `substr` counts characters too.
        // A draft cites only source versions with text (ADR 0051), so a cited version without text is a broken
        // row: the read fails instead of a manifest that differs from the stored one.
        let sources = sqlx::query!(
            r#"SELECT m.source_version_id, m.start_offset, m.end_offset,
                      substr(s.text, m.start_offset + 1, m.end_offset - m.start_offset) AS quote
               FROM document_manifest_source m
               JOIN source_version s ON s.organization_id = m.organization_id AND s.id = m.source_version_id
               WHERE m.organization_id = $1 AND m.document_version_id = $2
               ORDER BY m.source_version_id, m.start_offset, m.end_offset"#,
            organization,
            id.as_uuid(),
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(store_error)?;
        let offset = |value: i32| {
            u32::try_from(value).map_err(|_| InvalidRow("document_manifest_source.start_offset"))
        };
        let manifest = ProvenanceManifest {
            facts: facts
                .into_iter()
                .map(|row| {
                    Ok(CitedFact {
                        fact_id: FactId::from_uuid(row.fact_id),
                        version: RecordVersion::new(row.fact_version_number)
                            .ok_or(InvalidRow("document_manifest_fact.fact_version_number"))?,
                    })
                })
                .collect::<Result<_, InvalidRow>>()?,
            sources: sources
                .into_iter()
                .map(|row| {
                    Ok(Evidence {
                        source_version_id: SourceVersionId::from_uuid(row.source_version_id),
                        passage: Passage {
                            start: offset(row.start_offset)?,
                            end: offset(row.end_offset)?,
                            quote: row.quote.ok_or(InvalidRow("source_version.text"))?,
                            page: None,
                        },
                    })
                })
                .collect::<Result<_, InvalidRow>>()?,
        };
        let version = row.view()?;
        let VersionContent::Draft { status } = version.content else {
            return Err(InvalidRow("document_version.kind").into());
        };
        Ok(Some(StoredDraft {
            event_id: EventId::from_uuid(row.event_id),
            markdown: DraftMarkdown::parse(&markdown)
                .map_err(|_| InvalidRow("document_version.markdown"))?,
            status,
            version,
            manifest,
        }))
    }

    async fn approve(
        &self,
        scope: OrgScope,
        approval: &Approval,
        audit: &AuditEvent,
    ) -> Result<Approved, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The lock of the document row (kind 6 of the lock order) orders the approval after a new version
        // of the document and before the next one, because the apply of a draft updates this row too.
        // It orders two approvals of the document too. The lock is a statement of its own: in READ COMMITTED,
        // only a statement after the wait sees what a concurrent approval committed, because an approval
        // does not update the document row.
        let Some(document) = sqlx::query!(
            "SELECT d.id, d.version FROM document d
             WHERE d.organization_id = $1
               AND d.id = (SELECT document_id FROM document_version WHERE organization_id = $1 AND id = $2)
             FOR UPDATE",
            organization,
            approval.version_id.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?
        else {
            return Ok(Approved::NotFound);
        };
        let target = sqlx::query!(
            r#"SELECT v.kind, v.status,
                      EXISTS (SELECT 1 FROM document_version n
                              WHERE n.organization_id = v.organization_id AND n.document_id = v.document_id
                                AND n.number > v.number AND n.status = 'approved') AS "newer_approved!"
               FROM document_version v
               WHERE v.organization_id = $1 AND v.id = $2"#,
            organization,
            approval.version_id.as_uuid(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        if target.kind != DRAFT {
            return Ok(Approved::NotFound);
        }
        if document.version != approval.expected_version.get() {
            return Ok(Approved::VersionConflict);
        }
        let open = target
            .status
            .as_deref()
            .and_then(DraftStatus::parse)
            .is_some_and(|status| matches!(status, DraftStatus::Draft | DraftStatus::Review));
        if !open || target.newer_approved {
            return Ok(Approved::InvalidTransition);
        }
        sqlx::query!(
            "UPDATE document_version SET status = 'superseded'
             WHERE organization_id = $1 AND document_id = $2 AND status = 'approved'",
            organization,
            document.id,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        sqlx::query!(
            "UPDATE document_version SET status = 'approved', approved_by = $3, approved_at = $4
             WHERE organization_id = $1 AND id = $2",
            organization,
            approval.version_id.as_uuid(),
            approval.approved_by.as_uuid(),
            approval.approved_at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let version = versions(&mut tx, scope, None, Some(approval.version_id))
            .await?
            .pop()
            .ok_or(InvalidRow("document_version"))?
            .view()?;
        tx.commit().await.map_err(store_error)?;
        Ok(Approved::Approved(Box::new(version)))
    }

    async fn facts_changed(
        &self,
        scope: OrgScope,
        document: DocumentId,
    ) -> Result<bool, StoreError> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM document_manifest_fact m
                   JOIN fact f ON f.organization_id = m.organization_id AND f.id = m.fact_id
                   WHERE m.organization_id = $1
                     AND m.document_version_id = (
                         SELECT id FROM document_version
                         WHERE organization_id = $1 AND document_id = $2
                         ORDER BY number DESC
                         LIMIT 1)
                     AND f.version > m.fact_version_number
               ) AS "changed!""#,
            scope.organization_id().as_uuid(),
            document.as_uuid(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use tada_app::audit::AuditAction;
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::identity::{DisplayName, Email};
    use tada_app::domain::ids::OrganizationId;
    use tada_app::domain::sources::SourceText;

    use super::*;
    use crate::testing::{TestDatabase, sqlstate};

    /// An organization with one event and one member.
    struct Fixture {
        organization: OrganizationId,
        event: EventId,
        caller: MemberCaller,
    }

    impl Fixture {
        fn scope(&self) -> OrgScope {
            self.caller.scope()
        }
    }

    async fn fixture(test: &TestDatabase, slug: &str) -> Fixture {
        let organization = test.create_organization(slug).await;
        let event = test.create_event(organization, "OPEN30").await;
        let anna = test
            .create_user(
                &DisplayName::parse("Anna Muster").unwrap(),
                &Email::parse(&format!("anna@{slug}.example.org")).unwrap(),
            )
            .await;
        test.add_membership(organization, anna, OrganizationRole::Member)
            .await;
        Fixture {
            organization,
            event,
            caller: MemberCaller::new(anna, organization, OrganizationRole::Member),
        }
    }

    fn upload(f: &Fixture, target: UploadTarget, name: &str, content: &[u8]) -> NewUpload {
        NewUpload {
            target,
            version_id: DocumentVersionId::from_uuid(Uuid::now_v7()),
            source_version_id: SourceVersionId::from_uuid(Uuid::now_v7()),
            blob_key: BlobKey::new(f.organization),
            file_name: name.to_owned(),
            file_type: FileType::Text,
            size_bytes: content.len() as u64,
            sha256: Sha256::digest(content).into(),
            text: Some(SourceText::normalize(&String::from_utf8_lossy(content))),
            author: f.caller.actor(),
            uploaded_by: f.caller.user_id(),
            created_at: "2030-05-18T08:00:00.123456Z".parse().unwrap(),
        }
    }

    fn new_document(f: &Fixture) -> UploadTarget {
        UploadTarget::New {
            id: DocumentId::from_uuid(Uuid::now_v7()),
            event_id: f.event,
        }
    }

    fn audit_of(f: &Fixture, upload: &NewUpload) -> AuditEvent {
        AuditEvent::new(
            f.caller.actor(),
            AuditAction::DocumentVersionUpload,
            Some(upload.version_id.as_uuid()),
            Some(f.scope()),
        )
    }

    async fn publish(test: &TestDatabase, f: &Fixture, upload: &NewUpload) -> Published {
        test.database
            .publish(f.scope(), upload, &audit_of(f, upload))
            .await
            .unwrap()
    }

    async fn published(test: &TestDatabase, f: &Fixture, upload: &NewUpload) -> DocumentView {
        match publish(test, f, upload).await {
            Published::Published(document) => *document,
            other => panic!("not published: {other:?}"),
        }
    }

    async fn count(test: &TestDatabase, table: &str) -> i64 {
        test.scalar(&format!("SELECT count(*) FROM {table}")).await
    }

    #[tokio::test]
    async fn publishes_a_new_document_with_its_first_version_and_source_version() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let upload = upload(&f, new_document(&f), "Programm.txt", b"Flugshow um 14 Uhr");
        let document = published(&test, &f, &upload).await;

        assert_eq!(document.readable_id(), "DOC-001");
        assert_eq!(document.event_id, f.event);
        assert_eq!(document.name, "Programm.txt");
        assert_eq!(document.owner, f.caller.user_id());
        assert_eq!(document.version, RecordVersion::FIRST);
        let version = &document.newest_version;
        assert_eq!(version.id, upload.version_id);
        assert_eq!(version.number, 1);
        assert_eq!(version.sha256, upload.sha256);
        let file = version.file().unwrap();
        assert_eq!(file.file_name, "Programm.txt");
        assert_eq!(file.file_type, FileType::Text);
        assert_eq!(file.size_bytes, 18);
        assert_eq!(file.source_version_id, upload.source_version_id);
        assert_eq!(
            test.database.get(f.scope(), document.id).await.unwrap(),
            Some(document.clone())
        );

        let (kind, text, sha256): (String, Option<String>, Vec<u8>) =
            sqlx::query_as("SELECT kind, text, sha256 FROM source_version WHERE id = $1")
                .bind(upload.source_version_id.as_uuid())
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(
            (kind.as_str(), text.as_deref()),
            ("upload", Some("Flugshow um 14 Uhr"))
        );
        assert_eq!(sha256, upload.sha256);
        let audit: (String, String, Uuid) = sqlx::query_as(
            "SELECT action, record_kind, record_id FROM audit_event WHERE organization_id = $1",
        )
        .bind(f.organization.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(
            audit,
            (
                "document_version.upload".to_owned(),
                "document_version".to_owned(),
                upload.version_id.as_uuid()
            )
        );

        let second = published(
            &test,
            &f,
            &self::upload(&f, new_document(&f), "Plan.txt", b"Plan"),
        )
        .await;
        assert_eq!(second.readable_id(), "DOC-002");
    }

    #[tokio::test]
    async fn a_new_version_keeps_the_first_version_unchanged() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let first = upload(&f, new_document(&f), "Programm.txt", b"Version eins");
        let document = published(&test, &f, &first).await;
        let before = test
            .database
            .versions(f.scope(), document.id)
            .await
            .unwrap();

        let second = upload(
            &f,
            UploadTarget::Existing(document.id),
            "Programm-neu.txt",
            b"Version zwei",
        );
        let changed = published(&test, &f, &second).await;

        assert_eq!(changed.version, RecordVersion::new(2).unwrap());
        assert_eq!(
            changed.name, "Programm.txt",
            "the name stays the name of the first upload"
        );
        assert_eq!(changed.newest_version.number, 2);
        assert_eq!(
            changed.newest_version.file().unwrap().file_name,
            "Programm-neu.txt"
        );
        let after = test
            .database
            .versions(f.scope(), document.id)
            .await
            .unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[0], before[0]);
        assert_eq!(after[0].sha256, first.sha256);
        assert_eq!(after[1].sha256, second.sha256);
        // Both uploads are versions of one source item (ADR 0050).
        assert_eq!(count(&test, "source_item").await, 1);
        assert_eq!(count(&test, "source_version").await, 2);
    }

    #[tokio::test]
    async fn rejects_an_upload_over_the_storage_quota_and_stores_nothing() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let quota: i64 = test
            .scalar("SELECT storage_quota_bytes FROM organization")
            .await;
        assert_eq!(quota, 5 * 1024 * 1024 * 1024, "the default quota is 5 GiB");
        sqlx::query("UPDATE organization SET storage_quota_bytes = 20 WHERE id = $1")
            .bind(f.organization.as_uuid())
            .execute(&test.database.pool)
            .await
            .unwrap();
        published(
            &test,
            &f,
            &upload(&f, new_document(&f), "a.txt", &[b'a'; 15]),
        )
        .await;

        let over = upload(&f, new_document(&f), "b.txt", &[b'b'; 6]);
        assert_eq!(publish(&test, &f, &over).await, Published::QuotaExceeded);
        for table in [
            "document",
            "document_version",
            "source_version",
            "audit_event",
        ] {
            assert_eq!(count(&test, table).await, 1, "{table}");
        }
        // The quota includes the files of all events, and an upload up to the limit fits.
        published(
            &test,
            &f,
            &upload(&f, new_document(&f), "c.txt", &[b'c'; 5]),
        )
        .await;
    }

    #[tokio::test]
    async fn another_organization_reads_nothing_and_cannot_add_a_version() {
        let test = TestDatabase::start().await;
        let a = fixture(&test, "testwil").await;
        let b = fixture(&test, "musterhausen").await;
        let first = upload(&a, new_document(&a), "Programm.txt", b"Testwil");
        let document = published(&test, &a, &first).await;

        assert_eq!(
            test.database.get(b.scope(), document.id).await.unwrap(),
            None
        );
        assert!(
            test.database
                .versions(b.scope(), document.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            test.database
                .version(b.scope(), first.version_id)
                .await
                .unwrap(),
            None
        );
        assert!(
            test.database
                .list(b.scope(), a.event, None, None, 10)
                .await
                .unwrap()
                .is_empty()
        );
        let foreign = upload(&b, UploadTarget::Existing(document.id), "x.txt", b"x");
        assert_eq!(publish(&test, &b, &foreign).await, Published::NotFound);
        assert_eq!(count(&test, "document_version").await, 1);

        // Each organization counts its own document numbers.
        let own = published(
            &test,
            &b,
            &upload(&b, new_document(&b), "Plan.txt", b"Plan"),
        )
        .await;
        assert_eq!(own.readable_id(), "DOC-001");
    }

    #[tokio::test]
    async fn lists_the_documents_of_an_event_the_newest_first_with_a_name_filter() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let other_event = test.create_event(f.organization, "FLY31").await;
        for name in ["Programm.txt", "Lageplan.txt", "programm-alt.txt"] {
            published(&test, &f, &upload(&f, new_document(&f), name, b"x")).await;
        }
        let elsewhere = UploadTarget::New {
            id: DocumentId::from_uuid(Uuid::now_v7()),
            event_id: other_event,
        };
        published(&test, &f, &upload(&f, elsewhere, "Programm.txt", b"x")).await;

        let names = |documents: Vec<DocumentView>| -> Vec<String> {
            documents
                .into_iter()
                .map(|document| document.name)
                .collect()
        };
        let all = test
            .database
            .list(f.scope(), f.event, None, None, 10)
            .await
            .unwrap();
        assert_eq!(
            names(all.clone()),
            ["programm-alt.txt", "Lageplan.txt", "Programm.txt"]
        );
        let page = test
            .database
            .list(
                f.scope(),
                f.event,
                None,
                Some(DocumentCursor(all[0].local_number)),
                1,
            )
            .await
            .unwrap();
        assert_eq!(names(page), ["Lageplan.txt"]);
        let found = test
            .database
            .list(f.scope(), f.event, Some("PROGRAMM"), None, 10)
            .await
            .unwrap();
        assert_eq!(names(found), ["programm-alt.txt", "Programm.txt"]);
        // `%` and `_` are plain characters in the filter.
        let none = test
            .database
            .list(f.scope(), f.event, Some("%"), None, 10)
            .await
            .unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn the_content_of_a_version_never_changes() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let first = upload(&f, new_document(&f), "Programm.txt", b"Version eins");
        published(&test, &f, &first).await;
        for change in [
            "UPDATE document_version SET blob_key = 'other' WHERE id = $1",
            "UPDATE document_version SET sha256 = decode(repeat('00', 32), 'hex') WHERE id = $1",
            "UPDATE document_version SET file_name = 'x.txt' WHERE id = $1",
            "UPDATE document_version SET size_bytes = 1 WHERE id = $1",
        ] {
            let error = sqlx::query(change)
                .bind(first.version_id.as_uuid())
                .execute(&test.database.pool)
                .await
                .unwrap_err();
            assert_eq!(sqlstate(&error), "23001", "{change}");
        }
        let stored = test
            .database
            .version(f.scope(), first.version_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.blob_key, first.blob_key);
        assert_eq!(stored.version.sha256, first.sha256);
    }

    #[tokio::test]
    async fn documents_and_versions_are_never_deleted() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let first = upload(&f, new_document(&f), "Programm.txt", b"Version eins");
        published(&test, &f, &first).await;
        for statement in [
            "DELETE FROM document_version",
            "TRUNCATE document_version CASCADE",
            "DELETE FROM document",
            "TRUNCATE document CASCADE",
        ] {
            let error = sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&test.database.pool)
                .await
                .unwrap_err();
            assert_eq!(sqlstate(&error), "23001", "{statement}");
        }
        assert_eq!(count(&test, "document").await, 1);
        assert_eq!(count(&test, "document_version").await, 1);
    }

    #[tokio::test]
    async fn keeps_a_text_without_search_index_if_the_index_is_too_long() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        // Unique numbers give a search index that is larger than the text and larger than the PostgreSQL limit of 1 MB.
        let mut text = String::new();
        let mut number = 1_000_000_u32;
        while text.len() < 700 * 1024 {
            text.push_str(&format!("{number};"));
            number += 1;
        }
        let upload = upload(&f, new_document(&f), "Inventar.csv", text.as_bytes());
        let document = published(&test, &f, &upload).await;

        assert_eq!(document.newest_version.sha256, upload.sha256);
        let without_text: bool = test
            .scalar(&format!(
                "SELECT text IS NULL FROM source_version WHERE id = '{}'",
                upload.source_version_id
            ))
            .await;
        assert!(without_text);
        assert_eq!(count(&test, "document_version").await, 1);
    }

    #[tokio::test]
    async fn a_version_cannot_name_an_object_of_another_organization() {
        let test = TestDatabase::start().await;
        let a = fixture(&test, "testwil").await;
        let b = fixture(&test, "musterhausen").await;
        let mut foreign = upload(&a, new_document(&a), "Programm.txt", b"x");
        foreign.blob_key = BlobKey::new(b.organization);
        let error = test
            .database
            .publish(a.scope(), &foreign, &audit_of(&a, &foreign))
            .await
            .unwrap_err();
        let StoreError::Internal(source) = &error else {
            panic!("not internal: {error:?}");
        };
        let constraint = source
            .downcast_ref::<sqlx::Error>()
            .and_then(|error| error.as_database_error())
            .and_then(|error| error.constraint());
        assert_eq!(
            constraint,
            Some("document_version_blob_key_in_organization")
        );
        assert_eq!(count(&test, "document_version").await, 0);
    }

    #[tokio::test]
    async fn only_a_draft_has_a_status_and_markdown_and_it_has_no_file() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let first = upload(&f, new_document(&f), "Programm.txt", b"Version eins");
        let document = published(&test, &f, &first).await;
        // A second version of the kind `kind`, with or without a file, a status and Markdown.
        let insert = |kind: &'static str,
                      file: bool,
                      status: Option<&'static str>,
                      markdown: Option<&'static str>| {
            sqlx::query(
                "INSERT INTO document_version
                     (id, organization_id, document_id, number, kind, sha256, uploaded_by, status, created_at,
                      markdown, blob_key, media_type, size_bytes, file_name, source_version_id)
                 VALUES ($1, $2, $3, 2, $4, decode(repeat('00', 32), 'hex'), $5, $6, now(), $7,
                         CASE WHEN $10 THEN $8 END, CASE WHEN $10 THEN 'text/plain; charset=utf-8' END,
                         CASE WHEN $10 THEN 1 END, CASE WHEN $10 THEN 'x.txt' END, CASE WHEN $10 THEN $9::uuid END)",
            )
            .bind(Uuid::now_v7())
            .bind(f.organization.as_uuid())
            .bind(document.id.as_uuid())
            .bind(kind)
            .bind(f.caller.user_id().as_uuid())
            .bind(status)
            .bind(markdown)
            .bind(format!("{}/{}", f.organization, Uuid::now_v7()))
            .bind(first.source_version_id.as_uuid())
            .bind(file)
            .execute(&test.database.pool)
        };
        for (kind, file, status, markdown) in [
            ("draft", false, None, Some("Text")),
            ("draft", false, Some("draft"), None),
            ("draft", true, Some("draft"), Some("Text")),
            ("upload", true, Some("draft"), None),
            ("upload", true, None, Some("Text")),
        ] {
            let error = insert(kind, file, status, markdown).await.unwrap_err();
            assert_eq!(
                sqlstate(&error),
                "23514",
                "{kind} {file} {status:?} {markdown:?}"
            );
        }
        insert("draft", false, Some("draft"), Some("Text"))
            .await
            .unwrap();
        // The status of a draft can change: it is not content.
        let changed = sqlx::query(
            "UPDATE document_version SET status = 'approved', approved_by = uploaded_by, approved_at = now()
             WHERE document_id = $1 AND number = 2",
        )
        .bind(document.id.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(changed.rows_affected(), 1);
    }

    /// The approval record of a version never changes, and the status moves only forward (ADR 0051):
    /// draft to review, draft or review to approved, approved to superseded, and each status but archived to
    /// archived.
    #[tokio::test]
    async fn the_approval_of_a_version_never_changes_and_its_status_moves_only_forward() {
        let test = TestDatabase::start().await;
        let f = fixture(&test, "testwil").await;
        let first = upload(&f, new_document(&f), "Programm.txt", b"Version eins");
        let document = published(&test, &f, &first).await;
        // A new draft version with the number `number`.
        let insert = |number: i32| {
            let id = Uuid::now_v7();
            let query = sqlx::query(
                "INSERT INTO document_version
                     (id, organization_id, document_id, number, kind, sha256, uploaded_by, status, created_at, markdown)
                 VALUES ($1, $2, $3, $4, 'draft', decode(repeat('00', 32), 'hex'), $5, 'draft', now(), 'Text')",
            )
            .bind(id)
            .bind(f.organization.as_uuid())
            .bind(document.id.as_uuid())
            .bind(number)
            .bind(f.caller.user_id().as_uuid())
            .execute(&test.database.pool);
            async move {
                query.await.unwrap();
                id
            }
        };
        let update = |id: Uuid, change: &str| {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE document_version SET {change} WHERE id = $1"
            )))
            .bind(id)
            .execute(&test.database.pool)
        };
        let refuses = |id: Uuid, state: &'static str, changes: Vec<&'static str>| async move {
            for change in changes {
                let error = update(id, change).await.unwrap_err();
                assert_eq!(sqlstate(&error), "23001", "{state}: {change}");
            }
        };
        let approve = "status = 'approved', approved_by = uploaded_by, approved_at = now()";
        let set_approval = "approved_by = uploaded_by, approved_at = now()";

        let draft = insert(2).await;
        refuses(
            draft,
            "draft",
            vec!["status = 'superseded'", "status = 'approved'", set_approval],
        )
        .await;
        update(draft, "status = 'review'").await.unwrap();
        refuses(
            draft,
            "review",
            vec!["status = 'draft'", "status = 'superseded'", set_approval],
        )
        .await;
        update(draft, approve).await.unwrap();
        refuses(
            draft,
            "approved",
            vec![
                "approved_at = approved_at + interval '1 second'",
                "approved_by = NULL, approved_at = NULL",
                "status = 'draft'",
                "status = 'review'",
                "status = 'archived', approved_at = approved_at + interval '1 second'",
                approve,
            ],
        )
        .await;
        update(draft, "status = 'superseded'").await.unwrap();
        refuses(
            draft,
            "superseded",
            vec![
                "status = 'approved'",
                "status = 'draft'",
                "status = 'review'",
                "approved_at = now() + interval '1 day'",
            ],
        )
        .await;
        update(draft, "status = 'archived'").await.unwrap();
        refuses(
            draft,
            "archived",
            vec![
                "status = 'draft'",
                "status = 'review'",
                "status = 'approved'",
                "status = 'superseded'",
                "approved_at = now() + interval '1 day'",
            ],
        )
        .await;

        // A draft and a version in review can be archived without an approval.
        let other = insert(3).await;
        update(other, "status = 'archived'").await.unwrap();
        let review = insert(4).await;
        update(review, "status = 'review'").await.unwrap();
        update(review, "status = 'archived'").await.unwrap();
    }
}
