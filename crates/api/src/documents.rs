//! Documents and their versions: upload, list, read and download (ADR 0009, ADR 0043, ADR 0055),
//! and the rendering, approval and comparison of draft versions (ADR 0051).
//!
//! An upload is a raw request body with the media type `application/octet-stream`, not a multipart form.
//! The handler then streams the body to the object storage without a parser in between, and the
//! `X-File-Name` header carries the file name.

use std::collections::BTreeMap;

use axum::body::Body;
use axum::extract::{FromRequest, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::TryStreamExt;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::blobs::ByteStream;
use tada_app::documents::{
    self as app, ApproveError, DocumentCursor, DocumentReads, DocumentStores, DocumentView,
    DraftRendering as AppDraftRendering, DraftStatus, LineKind, ReadDocumentError, Resolution,
    UploadError, VersionContent, VersionDiff as AppVersionDiff, VersionView,
};
use tada_app::domain::ids::{DocumentId, DocumentVersionId, EventId};
use tada_app::drafts::{CitedFact as AppCitedFact, LintKind, LintWarning as AppLintWarning};
use tada_app::problem::ProblemCode;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};
use crate::values::{FactState, Passage, Value, state_parts};

/// The header with the file name of an upload, percent-encoded as UTF-8.
const FILE_NAME_HEADER: HeaderName = HeaderName::from_static("x-file-name");

/// The media type of an upload body.
const UPLOAD_MEDIA_TYPE: &str = "application/octet-stream";

/// A download never runs as a page of tada (ADR 0009): no script, no plugin, no same origin.
/// Only a page of tada can frame it, so the text preview works and another site cannot frame it.
const DOWNLOAD_CSP: HeaderValue =
    HeaderValue::from_static("default-src 'none'; frame-ancestors 'self'; sandbox");

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_documents, upload_document))
        .routes(routes!(get_document))
        .routes(routes!(list_document_versions, upload_document_version))
        .routes(routes!(download_document_version))
        .routes(routes!(render_document_version))
        .routes(routes!(approve_document_version))
        .routes(routes!(diff_document_versions))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "upload_document",
            codes(&[AUTHENTICATED, PATH, UPLOAD_BODY, UploadError::CODES]),
        ),
        (
            "upload_document_version",
            codes(&[AUTHENTICATED, PATH, UPLOAD_BODY, UploadError::CODES]),
        ),
        (
            "list_documents",
            codes(&[AUTHENTICATED, PATH, QUERY, ReadDocumentError::CODES]),
        ),
        (
            "get_document",
            codes(&[AUTHENTICATED, PATH, ReadDocumentError::CODES]),
        ),
        (
            "list_document_versions",
            codes(&[AUTHENTICATED, PATH, ReadDocumentError::CODES]),
        ),
        (
            "download_document_version",
            codes(&[AUTHENTICATED, PATH, QUERY, ReadDocumentError::CODES]),
        ),
        (
            "render_document_version",
            codes(&[AUTHENTICATED, PATH, ReadDocumentError::CODES]),
        ),
        (
            "approve_document_version",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, ApproveError::CODES]),
        ),
        (
            "diff_document_versions",
            codes(&[AUTHENTICATED, PATH, QUERY, ReadDocumentError::CODES]),
        ),
    ]
}

/// The codes of the `Upload` extractor.
const UPLOAD_BODY: &[ProblemCode] = &[
    ProblemCode::MalformedRequest,
    ProblemCode::UnsupportedMediaType,
    ProblemCode::PayloadTooLarge,
];

/// A document with its newest version.
#[derive(Debug, Serialize, ToSchema)]
pub struct Document {
    pub id: Uuid,
    pub event_id: Uuid,
    /// The readable ID, unique in the organization, for example `DOC-001`.
    pub readable_id: String,
    /// The file name of the first upload.
    pub name: String,
    /// The user ID of the member who created the document.
    pub owner: Uuid,
    pub created_at: Timestamp,
    /// The record version. It increases with each new document version.
    pub version: i64,
    pub newest_version: DocumentVersion,
    /// True if the newest version cites an older version of a fact: the client shows „Fakten geändert“ (ADR 0051).
    /// Only `get_document` gives it; it is absent in a list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facts_changed: Option<bool>,
}

impl From<DocumentView> for Document {
    fn from(document: DocumentView) -> Self {
        Self {
            id: document.id.as_uuid(),
            event_id: document.event_id.as_uuid(),
            readable_id: document.readable_id(),
            name: document.name,
            owner: document.owner.as_uuid(),
            created_at: document.created_at,
            version: document.version.get(),
            newest_version: document.newest_version.into(),
            facts_changed: None,
        }
    }
}

/// The kind of a document version (ADR 0051).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentVersionKind {
    /// A file that a member uploaded. It has the file fields.
    Upload,
    /// A Markdown draft from an accepted proposal. It has a `status` and no file.
    Draft,
}

/// The status of a draft version (ADR 0051).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DraftVersionStatus {
    Draft,
    Review,
    Approved,
    Superseded,
    Archived,
}

impl From<DraftStatus> for DraftVersionStatus {
    fn from(status: DraftStatus) -> Self {
        match status {
            DraftStatus::Draft => Self::Draft,
            DraftStatus::Review => Self::Review,
            DraftStatus::Approved => Self::Approved,
            DraftStatus::Superseded => Self::Superseded,
            DraftStatus::Archived => Self::Archived,
        }
    }
}

/// One immutable version of a document.
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    /// 1 for the first version of the document, then 2, 3 and so on.
    pub number: u32,
    pub kind: DocumentVersionKind,
    /// The status of a draft. It is absent for an upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DraftVersionStatus>,
    /// The file name of the upload, without control characters and path separators. It is absent for a draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// The media type that tada detected from the content (ADR 0055). It is absent for a draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// It is absent for a draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// The SHA-256 hash of the content, as lowercase hexadecimal digits.
    pub sha256: String,
    /// The user ID of the member who added the version.
    pub uploaded_by: Uuid,
    /// The source version that holds the same file (ADR 0050). It is absent for a draft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version_id: Option<Uuid>,
    pub created_at: Timestamp,
}

impl From<VersionView> for DocumentVersion {
    fn from(version: VersionView) -> Self {
        let (kind, status, file) = match &version.content {
            VersionContent::Upload(file) => (DocumentVersionKind::Upload, None, Some(file)),
            VersionContent::Draft { status } => {
                (DocumentVersionKind::Draft, Some((*status).into()), None)
            }
        };
        Self {
            id: version.id.as_uuid(),
            document_id: version.document_id.as_uuid(),
            number: version.number,
            kind,
            status,
            file_name: file.map(|file| file.file_name.clone()),
            media_type: file.map(|file| file.file_type.media_type().to_owned()),
            size_bytes: file.map(|file| file.size_bytes),
            sha256: version
                .sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            uploaded_by: version.uploaded_by.as_uuid(),
            source_version_id: file.map(|file| file.source_version_id.as_uuid()),
            created_at: version.created_at,
        }
    }
}

/// The body of an upload: the file name and the stream of the file.
struct Upload {
    file_name: String,
    body: ByteStream,
}

impl FromRequest<ApiState> for Upload {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &ApiState) -> Result<Self, ApiError> {
        let headers = request.headers();
        if !is_octet_stream(headers) {
            return Err(ApiError::new(ProblemCode::UnsupportedMediaType)
                .with_detail("The body must have the media type application/octet-stream."));
        }
        let file_name = headers
            .get(FILE_NAME_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(percent_decode)
            .ok_or_else(|| {
                ApiError::new(ProblemCode::MalformedRequest).with_detail(
                    "The header X-File-Name must hold the percent-encoded UTF-8 file name.",
                )
            })?;
        // A declared size over the limit fails before the server reads the body.
        // The command also counts the bytes, because the declared size can be wrong or absent.
        let declared = headers
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        if declared.is_some_and(|size| size > state.upload_max_bytes.get()) {
            return Err(ApiError::new(ProblemCode::PayloadTooLarge));
        }
        let body = request
            .into_body()
            .into_data_stream()
            .map_err(std::io::Error::other);
        Ok(Self {
            file_name,
            body: Box::pin(body),
        })
    }
}

fn is_octet_stream(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case(UPLOAD_MEDIA_TYPE))
}

/// Decodes `%XX` sequences into UTF-8. Returns `None` for a broken sequence or bytes that are not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let digits = bytes.get(index + 1..index + 3)?;
            let high = char::from(digits[0]).to_digit(16)?;
            let low = char::from(digits[1]).to_digit(16)?;
            decoded.push(u8::try_from(high * 16 + low).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// Uploads a file as the first version of a new document in the event.
///
/// The body is the raw file with the media type `application/octet-stream`, not a multipart form,
/// so that the server streams it to the object storage. The header `X-File-Name` holds the file
/// name, percent-encoded as UTF-8. The server detects the type from the content (ADR 0055).
/// The body can be as large as `TADA_UPLOAD_MAX_BYTES`. The caller needs the event role
/// contributor or manager, or the organization role owner or admin (ADR 0052).
#[utoipa::path(
    post,
    path = "/events/{event_id}/documents",
    operation_id = "upload_document",
    tag = "documents",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("X-File-Name" = String, Header, description = "The file name, percent-encoded as UTF-8."),
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "The raw file. Not a multipart form."),
    responses(
        (status = CREATED, description = "The new document.", body = Document),
        (status = "default", description = "A problem (ADR 0037). An upload over the storage quota is `validation-failed` with the entry `quota-exceeded` on `/file`.", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn upload_document(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    upload: Upload,
) -> Result<(StatusCode, axum::Json<Document>), ApiError> {
    let document = app::upload_document(
        &caller,
        EventId::from_uuid(event_id),
        &upload.file_name,
        upload.body,
        state.upload_max_bytes.get(),
        state.document_stores(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(document.into())))
}

/// Uploads a file as a new version of the document. The older versions stay unchanged.
///
/// The body has the same form as for `upload_document`.
#[utoipa::path(
    post,
    path = "/documents/{document_id}/versions",
    operation_id = "upload_document_version",
    tag = "documents",
    params(
        ("document_id" = Uuid, Path, description = "The ID of the document."),
        ("X-File-Name" = String, Header, description = "The file name, percent-encoded as UTF-8."),
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "The raw file. Not a multipart form."),
    responses(
        (status = CREATED, description = "The document with the new version as its newest version.", body = Document),
        (status = "default", description = "A problem (ADR 0037). An upload over the storage quota is `validation-failed` with the entry `quota-exceeded` on `/file`.", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn upload_document_version(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(document_id): Path<Uuid>,
    upload: Upload,
) -> Result<(StatusCode, axum::Json<Document>), ApiError> {
    let document = app::upload_version(
        &caller,
        DocumentId::from_uuid(document_id),
        &upload.file_name,
        upload.body,
        state.upload_max_bytes.get(),
        state.document_stores(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(document.into())))
}

/// The parameters of `ListDocuments`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListDocumentsQuery {
    /// Only the documents whose name contains this text, without regard to case.
    pub q: Option<String>,
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
}

/// One page of documents.
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentPage {
    pub items: Vec<Document>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Lists the documents of the event, the newest first.
#[utoipa::path(
    get,
    path = "/events/{event_id}/documents",
    operation_id = "list_documents",
    tag = "documents",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ListDocumentsQuery,
    ),
    responses(
        (status = OK, description = "One page of documents.", body = DocumentPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_documents(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListDocumentsQuery>,
) -> Result<axum::Json<DocumentPage>, ApiError> {
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_documents(
        &caller,
        EventId::from_uuid(event_id),
        query.q.as_deref(),
        after,
        limit,
        state.document_reads(),
    )
    .await?;
    Ok(axum::Json(DocumentPage {
        items: page.items.into_iter().map(Document::from).collect(),
        next_cursor: page.next.map(encode_cursor),
    }))
}

/// Reads one document with its newest version.
#[utoipa::path(
    get,
    path = "/documents/{document_id}",
    operation_id = "get_document",
    tag = "documents",
    params(("document_id" = Uuid, Path, description = "The ID of the document.")),
    responses(
        (status = OK, description = "The document.", body = Document),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_document(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(document_id): Path<Uuid>,
) -> Result<axum::Json<Document>, ApiError> {
    let id = DocumentId::from_uuid(document_id);
    let document = app::get_document(&caller, id, state.document_reads()).await?;
    let facts_changed = app::facts_changed(&caller, id, state.document_reads()).await?;
    Ok(axum::Json(Document {
        facts_changed: Some(facts_changed),
        ..document.into()
    }))
}

/// All versions of a document.
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentVersionList {
    /// The versions in the order of their numbers.
    pub items: Vec<DocumentVersion>,
}

/// Lists the versions of the document, in the order of their numbers.
#[utoipa::path(
    get,
    path = "/documents/{document_id}/versions",
    operation_id = "list_document_versions",
    tag = "documents",
    params(("document_id" = Uuid, Path, description = "The ID of the document.")),
    responses(
        (status = OK, description = "The versions of the document.", body = DocumentVersionList),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_document_versions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(document_id): Path<Uuid>,
) -> Result<axum::Json<DocumentVersionList>, ApiError> {
    let versions = app::list_versions(
        &caller,
        DocumentId::from_uuid(document_id),
        state.document_reads(),
    )
    .await?;
    Ok(axum::Json(DocumentVersionList {
        items: versions.into_iter().map(DocumentVersion::from).collect(),
    }))
}

/// How the browser handles a download.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    /// Save the file.
    #[default]
    Attachment,
    /// Show the file in the browser. Only PDF and plain text; each other type is still an attachment.
    Inline,
}

/// The parameters of `DownloadDocumentVersion`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DownloadQuery {
    /// The default is `attachment`.
    #[param(inline)]
    pub disposition: Option<Disposition>,
}

/// Downloads the file of a document version.
///
/// The response has the media type that tada detected, `X-Content-Type-Options: nosniff`, a
/// `Content-Security-Policy` that allows nothing, and the file name in `Content-Disposition`
/// (RFC 6266). `disposition=inline` applies only to PDF and plain text (ADR 0009, ADR 0043).
#[utoipa::path(
    get,
    path = "/document-versions/{version_id}/content",
    operation_id = "download_document_version",
    tag = "documents",
    params(
        ("version_id" = Uuid, Path, description = "The ID of the document version."),
        DownloadQuery,
    ),
    responses(
        (status = OK, description = "The file. Its media type is the `media_type` of the version.", content_type = "application/octet-stream", body = Vec<u8>),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn download_document_version(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(version_id): Path<Uuid>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, ApiError> {
    let (version, body) = app::download(
        &caller,
        DocumentVersionId::from_uuid(version_id),
        state.document_stores(),
    )
    .await?;
    // `download` answers `not-found` for a draft, so the version has a file.
    let file = version
        .file()
        .ok_or_else(|| ApiError::new(ProblemCode::NotFound))?;
    let inline = query.disposition.unwrap_or_default() == Disposition::Inline
        && file.file_type.inline_preview();
    let disposition = content_disposition(
        if inline { "inline" } else { "attachment" },
        &file.file_name,
    )?;
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static(file.file_type.media_type()),
        ),
        (header::CONTENT_LENGTH, HeaderValue::from(file.size_bytes)),
        (header::CONTENT_DISPOSITION, disposition),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (header::CONTENT_SECURITY_POLICY, DOWNLOAD_CSP),
        // Blocks no-cors loads of the file by another site (for example `<img>` or `<script>`).
        // It does not stop a frame: `frame-ancestors` in the CSP does.
        (
            HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-origin"),
        ),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        ),
    ];
    Ok((headers, Body::from_stream(body)).into_response())
}

/// A draft as the reader sees it (ADR 0051, ADR 0058). The client renders the Markdown and resolves each `tada:`
/// link from `links` only: a link without an entry shows „entfernt“. The server never formats a value.
#[derive(Debug, Serialize, ToSchema)]
pub struct DraftRendering {
    /// The Markdown with LF line ends. Raw HTML in it is text.
    pub markdown: String,
    /// The warnings of the draft lint for the review. They do not block.
    pub lint_warnings: Vec<LintWarning>,
    /// The target of each `tada:` link, by the exact text of its destination.
    pub links: BTreeMap<String, LinkTarget>,
}

impl From<AppDraftRendering> for DraftRendering {
    fn from(draft: AppDraftRendering) -> Self {
        Self {
            markdown: draft.markdown.as_str().to_owned(),
            lint_warnings: draft
                .lint_warnings
                .into_iter()
                .map(LintWarning::from)
                .collect(),
            links: draft
                .links
                .into_iter()
                .map(|(link, target)| (link, target.into()))
                .collect(),
        }
    }
}

/// A warning of the draft lint on one line, from 1.
#[derive(Debug, Serialize, ToSchema)]
pub struct LintWarning {
    pub line: u32,
    pub kind: LintWarningKind,
}

/// What the lint found outside a `tada:` link. The list of kinds is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LintWarningKind {
    Number,
    Date,
    Money,
    /// Raw HTML, which the client shows as text.
    RawHtml,
}

impl From<AppLintWarning> for LintWarning {
    fn from(warning: AppLintWarning) -> Self {
        Self {
            line: warning.line,
            kind: match warning.kind {
                LintKind::Number => LintWarningKind::Number,
                LintKind::Date => LintWarningKind::Date,
                LintKind::Money => LintWarningKind::Money,
                LintKind::RawHtml => LintWarningKind::RawHtml,
            },
        }
    }
}

/// The target of a `tada:` link for the reader. `kind` names it. The list of kinds is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LinkTarget {
    /// The exact fact version that the draft cites. The client marks an assumption with „Annahme“
    /// and shows an unknown as „unbekannt“.
    Fact {
        fact_id: Uuid,
        version: i64,
        state: FactState,
        /// The value. It is absent if the state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// It is absent if the state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        approximate: Option<bool>,
    },
    /// The cited passage of a source version.
    Source {
        source_version_id: Uuid,
        passage: Passage,
    },
    /// The reader cannot see the target. The client shows „entfernt“.
    Hidden,
}

impl From<Resolution> for LinkTarget {
    fn from(resolution: Resolution) -> Self {
        match resolution {
            Resolution::Fact(fact) => {
                let (state, value, approximate) = state_parts(&fact.state);
                Self::Fact {
                    fact_id: fact.fact_id.as_uuid(),
                    version: fact.number.get(),
                    state,
                    value,
                    approximate,
                }
            }
            Resolution::Source(evidence) => Self::Source {
                source_version_id: evidence.source_version_id.as_uuid(),
                passage: (&evidence.passage).into(),
            },
            Resolution::Hidden => Self::Hidden,
        }
    }
}

/// A draft version as the reader sees it.
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentVersionRendering {
    pub version: DocumentVersion,
    pub draft: DraftRendering,
}

/// Reads a draft version with what the client needs to render it: the Markdown, the lint warnings and the target
/// of each `tada:` link for the caller. An upload version is not found here.
#[utoipa::path(
    get,
    path = "/document-versions/{version_id}/rendering",
    operation_id = "render_document_version",
    tag = "documents",
    params(("version_id" = Uuid, Path, description = "The ID of the draft version.")),
    responses(
        (status = OK, description = "The draft version for the caller.", body = DocumentVersionRendering),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn render_document_version(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(version_id): Path<Uuid>,
) -> Result<axum::Json<DocumentVersionRendering>, ApiError> {
    let rendering = app::render_context(
        &caller,
        DocumentVersionId::from_uuid(version_id),
        state.document_reads(),
    )
    .await?;
    Ok(axum::Json(DocumentVersionRendering {
        version: rendering.version.into(),
        draft: rendering.draft.into(),
    }))
}

/// The input of `ApproveDocumentVersion`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ApproveDocumentVersionRequest {
    /// The record version of the document that the caller read.
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Approves a draft version. Only an event manager approves (ADR 0052).
///
/// The approved version never changes. The version that was approved before becomes `superseded`.
/// A version that is approved, superseded or archived, or older than the approved version, gives `invalid-transition`.
#[utoipa::path(
    post,
    path = "/document-versions/{version_id}/approve",
    operation_id = "approve_document_version",
    tag = "documents",
    params(("version_id" = Uuid, Path, description = "The ID of the draft version.")),
    request_body = ApproveDocumentVersionRequest,
    responses(
        (status = OK, description = "The approved version.", body = DocumentVersion),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn approve_document_version(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(version_id): Path<Uuid>,
    Json(request): Json<ApproveDocumentVersionRequest>,
) -> Result<axum::Json<DocumentVersion>, ApiError> {
    let version = app::approve_version(
        &caller,
        DocumentVersionId::from_uuid(version_id),
        record_version(request.expected_version)?,
        state.document_reads(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(version.into()))
}

/// The parameters of `DiffDocumentVersions`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DiffQuery {
    /// The ID of the older draft version.
    pub from: Uuid,
    /// The ID of the newer draft version.
    pub to: Uuid,
}

/// The difference between two draft versions of a document (ADR 0051).
#[derive(Debug, Serialize, ToSchema)]
pub struct VersionDiff {
    /// Each line of the two versions, in the order of the newer version, with the removed lines at their places.
    pub lines: Vec<LineChange>,
    pub facts: FactDiff,
}

/// One line of the text difference.
#[derive(Serialize, ToSchema)]
pub struct LineChange {
    pub kind: LineChangeKind,
    /// The line number in the older version. It is absent for an added line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_line: Option<u32>,
    /// The line number in the newer version. It is absent for a removed line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_line: Option<u32>,
    /// The text of the line without its line end.
    pub text: String,
}

/// The text is document content, so `Debug` leaves it out (ADR 0035).
impl std::fmt::Debug for LineChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineChange")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LineChangeKind {
    Unchanged,
    Removed,
    Added,
}

/// The facts that the two manifests cite. If a version cites more than one version of a fact, the newest counts.
#[derive(Debug, Serialize, ToSchema)]
pub struct FactDiff {
    /// The facts that both versions cite, each in another version.
    pub changed: Vec<FactChange>,
    /// The facts that only the newer version cites.
    pub added: Vec<CitedFact>,
    /// The facts that only the older version cites.
    pub removed: Vec<CitedFact>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FactChange {
    pub fact_id: Uuid,
    /// The fact version that the older draft version cites.
    pub from: i64,
    /// The fact version that the newer draft version cites.
    pub to: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CitedFact {
    pub fact_id: Uuid,
    pub version: i64,
}

impl From<AppVersionDiff> for VersionDiff {
    fn from(diff: AppVersionDiff) -> Self {
        let cited = |fact: AppCitedFact| CitedFact {
            fact_id: fact.fact_id.as_uuid(),
            version: fact.version.get(),
        };
        Self {
            lines: diff
                .lines
                .into_iter()
                .map(|line| LineChange {
                    kind: match line.kind {
                        LineKind::Unchanged => LineChangeKind::Unchanged,
                        LineKind::Removed => LineChangeKind::Removed,
                        LineKind::Added => LineChangeKind::Added,
                    },
                    old_line: line.old_line,
                    new_line: line.new_line,
                    text: line.text,
                })
                .collect(),
            facts: FactDiff {
                changed: diff
                    .facts
                    .changed
                    .into_iter()
                    .map(|change| FactChange {
                        fact_id: change.fact_id.as_uuid(),
                        from: change.from.get(),
                        to: change.to.get(),
                    })
                    .collect(),
                added: diff.facts.added.into_iter().map(cited).collect(),
                removed: diff.facts.removed.into_iter().map(cited).collect(),
            },
        }
    }
}

/// Compares two draft versions of the document: the lines, and the facts of their manifests.
/// A version that is not a draft version of this document is not found.
#[utoipa::path(
    get,
    path = "/documents/{document_id}/diff",
    operation_id = "diff_document_versions",
    tag = "documents",
    params(
        ("document_id" = Uuid, Path, description = "The ID of the document."),
        DiffQuery,
    ),
    responses(
        (status = OK, description = "The difference from `from` to `to`.", body = VersionDiff),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn diff_document_versions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(document_id): Path<Uuid>,
    Query(query): Query<DiffQuery>,
) -> Result<axum::Json<VersionDiff>, ApiError> {
    let diff = app::diff_versions(
        &caller,
        DocumentId::from_uuid(document_id),
        DocumentVersionId::from_uuid(query.from),
        DocumentVersionId::from_uuid(query.to),
        state.document_reads(),
    )
    .await?;
    Ok(axum::Json(diff.into()))
}

/// `Content-Disposition` with the file name (RFC 6266).
///
/// `filename` holds an ASCII fallback for old clients: each character other than letters, digits,
/// space, `.`, `-`, `_`, `(` and `)` becomes `_`, so no `"`, `\` or `;` can end the value.
/// `filename*` holds the exact name, percent-encoded as UTF-8 (RFC 8187).
fn content_disposition(kind: &'static str, file_name: &str) -> Result<HeaderValue, ApiError> {
    let fallback: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::with_capacity(file_name.len());
    for byte in file_name.bytes() {
        // The `attr-char` of RFC 8187 stays; each other byte becomes `%XX`.
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    HeaderValue::from_str(&format!(
        "{kind}; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .map_err(|_| ApiError::new(ProblemCode::Internal))
}

/// The cursor is opaque for clients (ADR 0044): the number of the readable ID, in Base64.
fn encode_cursor(cursor: DocumentCursor) -> String {
    cursor::encode(cursor.0.to_string())
}

fn decode_cursor(text: &str) -> Result<DocumentCursor, ApiError> {
    let text = cursor::decode_text(text)?;
    Ok(DocumentCursor(text.parse().map_err(|_| cursor::invalid())?))
}

impl ApiState {
    fn document_stores(&self) -> DocumentStores<'_> {
        DocumentStores {
            identity: self.identity.as_ref(),
            documents: self.documents.as_ref(),
            blobs: self.blobs.as_ref(),
        }
    }

    fn document_reads(&self) -> DocumentReads<'_> {
        DocumentReads {
            identity: self.identity.as_ref(),
            documents: self.documents.as_ref(),
            facts: self.facts.as_ref(),
            sources: self.sources.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disposition(kind: &'static str, name: &str) -> String {
        content_disposition(kind, name)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_plain_name_is_the_same_in_both_parameters() {
        assert_eq!(
            disposition("attachment", "Programm.pdf"),
            "attachment; filename=\"Programm.pdf\"; filename*=UTF-8''Programm.pdf"
        );
    }

    #[test]
    fn quotes_and_semicolons_cannot_end_the_value() {
        let value = disposition("attachment", "a\"; filename=x.html; b.pdf");
        assert_eq!(
            value,
            "attachment; filename=\"a__ filename_x.html_ b.pdf\"; \
             filename*=UTF-8''a%22%3B%20filename%3Dx.html%3B%20b.pdf"
        );
    }

    #[test]
    fn encodes_a_name_that_is_not_ascii_as_utf_8() {
        assert_eq!(
            disposition("inline", "Übersicht 2030.txt"),
            "inline; filename=\"_bersicht 2030.txt\"; filename*=UTF-8''%C3%9Cbersicht%202030.txt"
        );
    }

    #[test]
    fn decodes_a_percent_encoded_file_name() {
        assert_eq!(
            percent_decode("%C3%9Cbersicht%202030.pdf").as_deref(),
            Some("Übersicht 2030.pdf")
        );
        assert_eq!(percent_decode("Plan.pdf").as_deref(), Some("Plan.pdf"));
    }

    #[test]
    fn rejects_a_broken_percent_sequence() {
        for broken in ["%", "%4", "%zz", "%+1", "%C3"] {
            assert_eq!(percent_decode(broken), None, "{broken}");
        }
    }

    #[test]
    fn a_cursor_survives_its_round_trip() {
        let cursor = decode_cursor(&encode_cursor(DocumentCursor(42))).unwrap();
        assert_eq!(cursor, DocumentCursor(42));
        assert!(decode_cursor("not a cursor").is_err());
    }

    #[test]
    fn a_draft_version_has_a_status_and_no_file_fields() {
        use tada_app::domain::ids::{DocumentVersionId, UserId};
        let version = VersionView {
            id: DocumentVersionId::from_uuid(Uuid::from_u128(1)),
            document_id: DocumentId::from_uuid(Uuid::from_u128(2)),
            number: 2,
            sha256: [0; 32],
            uploaded_by: UserId::from_uuid(Uuid::from_u128(3)),
            created_at: Timestamp::UNIX_EPOCH,
            content: VersionContent::Draft {
                status: DraftStatus::Review,
            },
        };
        let json = serde_json::to_value(DocumentVersion::from(version)).unwrap();
        assert_eq!(json["kind"], "draft");
        assert_eq!(json["status"], "review");
        for absent in ["file_name", "media_type", "size_bytes", "source_version_id"] {
            assert!(json.get(absent).is_none(), "{absent}");
        }
    }
}
