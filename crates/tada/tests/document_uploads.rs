//! The document commands with PostgreSQL and Garage (ADR 0009, ADR 0043): upload, versions and download.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::io;

use bytes::Bytes;
use futures::TryStreamExt;
use sha2::{Digest, Sha256};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_app::blobs::ByteStream;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::documents::{
    DocumentStores, DocumentView, ReadDocumentError, UploadError, download, get_document,
    list_documents, list_versions, upload_document, upload_version,
};
use tada_app::domain::identity::EventRole;
use tada_app::domain::ids::{EventId, OrganizationId};
use tada_app::event_members::add_event_member;
use tada_app::paging::PageLimit;
use tada_app::problem::CommandError;
use tada_store_pg::testing::TestDatabase;

/// The upload limit of the tests.
const LIMIT: u64 = 1024 * 1024;

/// PostgreSQL and Garage with one organization, one event and its owner.
struct Fixture {
    test: TestDatabase,
    garage: TestGarage,
    organization: OrganizationId,
    event: EventId,
    owner: MemberCaller,
}

impl Fixture {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, owner) = member(&test, "testwil", OrganizationRole::Owner).await;
        let event = test.create_event(organization, "OPEN30").await;
        Self {
            test,
            garage,
            organization,
            event,
            owner,
        }
    }

    fn stores(&self) -> DocumentStores<'_> {
        DocumentStores {
            identity: &self.test.database,
            documents: &self.test.database,
            blobs: &self.garage.storage,
        }
    }

    /// A new member of the organization with the event role `role` in the event.
    async fn with_role(&self, role: EventRole) -> MemberCaller {
        let (_, caller) = member(&self.test, "testwil", OrganizationRole::Member).await;
        add_event_member(
            &self.owner,
            self.event,
            caller.user_id(),
            role,
            &self.test.database,
            &self.test.database,
            &SystemClock,
        )
        .await
        .unwrap();
        caller
    }

    async fn upload(
        &self,
        caller: &MemberCaller,
        name: &str,
        content: &[u8],
    ) -> Result<DocumentView, UploadError> {
        upload_document(
            caller,
            self.event,
            name,
            body(content),
            LIMIT,
            self.stores(),
            &SystemClock,
        )
        .await
    }

    async fn read(&self, caller: &MemberCaller, version: &DocumentView) -> Vec<u8> {
        let (_, stream) = download(caller, version.newest_version.id, self.stores())
            .await
            .unwrap();
        let chunks: Vec<Bytes> = stream.try_collect().await.unwrap();
        chunks.concat()
    }
}

async fn member(
    test: &TestDatabase,
    slug: &str,
    role: OrganizationRole,
) -> (OrganizationId, MemberCaller) {
    let (organization, user, _) = test.member(slug, role).await;
    (organization, MemberCaller::new(user, organization, role))
}

/// A stream of the content in chunks of 1000 bytes, as an HTTP body arrives.
fn body(content: &[u8]) -> ByteStream {
    let chunks: Vec<Result<Bytes, io::Error>> = content
        .chunks(1000)
        .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
        .collect();
    Box::pin(futures::stream::iter(chunks))
}

/// A small PDF document with one empty page, made at run time.
fn pdf(title: &str) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Title ({title}) >>"),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// The start of an ELF executable: a program that tada must never accept.
fn executable() -> Vec<u8> {
    let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    elf.resize(4096, 0);
    elf
}

fn sha256(content: &[u8]) -> [u8; 32] {
    Sha256::digest(content).into()
}

#[tokio::test]
async fn uploads_a_pdf_as_the_first_version_of_doc_001() {
    let f = Fixture::start().await;
    let content = pdf("Programm Open Day");
    let document = f.upload(&f.owner, "Programm.pdf", &content).await.unwrap();

    assert_eq!(document.readable_id(), "DOC-001");
    let version = &document.newest_version;
    assert_eq!(version.number, 1);
    assert_eq!(version.file_name, "Programm.pdf");
    assert_eq!(version.file_type.media_type(), "application/pdf");
    assert_eq!(version.size_bytes, content.len() as u64);
    assert_eq!(version.sha256, sha256(&content));
    assert_eq!(f.read(&f.owner, &document).await, content);
    assert_eq!(f.garage.keys().await.len(), 1);
    // The object key starts with the organization and holds no file name (ADR 0009).
    let key = &f.garage.keys().await[0];
    assert!(key.starts_with(&format!("{}/", f.organization)), "{key}");
    assert!(!key.contains("Programm"), "{key}");
    // The upload is a source version with the hash of the file, and a PDF has no text yet (OP10).
    let source: String = f
        .test
        .scalar(&format!(
            "SELECT kind || ' ' || (text IS NULL) FROM source_version WHERE id = '{}'",
            version.source_version_id
        ))
        .await;
    assert_eq!(source, "upload true");
}

#[tokio::test]
async fn a_second_version_keeps_the_first_version_readable_and_unchanged() {
    let f = Fixture::start().await;
    let first = pdf("Programm Version 1");
    let document = f.upload(&f.owner, "Programm.pdf", &first).await.unwrap();
    let second = pdf("Programm Version 2");
    let changed = upload_version(
        &f.owner,
        document.id,
        "Programm.pdf",
        body(&second),
        LIMIT,
        f.stores(),
        &SystemClock,
    )
    .await
    .unwrap();

    assert_eq!(changed.newest_version.number, 2);
    let versions = list_versions(&f.owner, document.id, f.stores())
        .await
        .unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0], document.newest_version);
    assert_eq!(f.read(&f.owner, &document).await, first);
    assert_eq!(
        sha256(&f.read(&f.owner, &document).await),
        versions[0].sha256
    );
    assert_eq!(f.read(&f.owner, &changed).await, second);
    assert_eq!(f.garage.keys().await.len(), 2);
}

#[tokio::test]
async fn rejects_a_renamed_executable_and_keeps_no_object() {
    let f = Fixture::start().await;
    let error = f
        .upload(&f.owner, "Programm.pdf", &executable())
        .await
        .unwrap_err();
    assert!(matches!(error, UploadError::UnsupportedType), "{error:?}");
    assert_eq!(error.code().as_str(), "unsupported-media-type");
    assert!(f.garage.keys().await.is_empty());
    let documents: i64 = f.test.scalar("SELECT count(*) FROM document").await;
    assert_eq!(documents, 0);
}

#[tokio::test]
async fn rejects_a_stream_over_the_limit_and_keeps_no_object() {
    let f = Fixture::start().await;
    let mut content = pdf("Gross");
    content.resize(LIMIT as usize + 1, b' ');
    let error = f.upload(&f.owner, "Gross.pdf", &content).await.unwrap_err();
    assert!(matches!(error, UploadError::TooLarge), "{error:?}");
    assert_eq!(error.code().as_str(), "payload-too-large");
    assert!(f.garage.keys().await.is_empty());
    assert_eq!(f.garage.open_uploads().await, 0);
}

#[tokio::test]
async fn rejects_an_upload_over_the_quota_with_quota_exceeded() {
    let f = Fixture::start().await;
    let content = pdf("Programm");
    let quota = content.len() as i64 + 10;
    let _: i64 = f
        .test
        .scalar(&format!(
            "WITH changed AS (UPDATE organization SET storage_quota_bytes = {quota} RETURNING 1)
             SELECT count(*) FROM changed"
        ))
        .await;
    let document = f.upload(&f.owner, "Programm.pdf", &content).await.unwrap();

    let error = f.upload(&f.owner, "Plan.pdf", &content).await.unwrap_err();
    assert!(matches!(error, UploadError::QuotaExceeded), "{error:?}");
    assert_eq!(error.code().as_str(), "validation-failed");
    let codes: Vec<_> = error
        .field_errors()
        .iter()
        .map(|entry| entry.code)
        .collect();
    assert_eq!(codes, ["quota-exceeded"]);
    assert_eq!(
        f.garage.keys().await,
        [f.test
            .scalar::<String>("SELECT blob_key FROM document_version")
            .await]
    );
    assert_eq!(
        get_document(&f.owner, document.id, f.stores())
            .await
            .unwrap(),
        document
    );
}

#[tokio::test]
async fn a_viewer_can_download_but_cannot_upload() {
    let f = Fixture::start().await;
    let document = f
        .upload(&f.owner, "Programm.pdf", &pdf("Programm"))
        .await
        .unwrap();
    let viewer = f.with_role(EventRole::EventViewer).await;

    assert_eq!(f.read(&viewer, &document).await, pdf("Programm"));
    let error = f
        .upload(&viewer, "Plan.pdf", &pdf("Plan"))
        .await
        .unwrap_err();
    assert!(matches!(error, UploadError::Forbidden), "{error:?}");
    let error = upload_version(
        &viewer,
        document.id,
        "Programm.pdf",
        body(&pdf("Neu")),
        LIMIT,
        f.stores(),
        &SystemClock,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, UploadError::Forbidden), "{error:?}");
    assert_eq!(f.garage.keys().await.len(), 1);

    let contributor = f.with_role(EventRole::EventContributor).await;
    f.upload(&contributor, "Plan.pdf", &pdf("Plan"))
        .await
        .unwrap();
}

#[tokio::test]
async fn another_organization_cannot_read_or_change_a_document() {
    let f = Fixture::start().await;
    let document = f
        .upload(&f.owner, "Programm.pdf", &pdf("Programm"))
        .await
        .unwrap();
    let (_, stranger) = member(&f.test, "musterhausen", OrganizationRole::Owner).await;

    let not_found =
        |result: Result<(), ReadDocumentError>| matches!(result, Err(ReadDocumentError::NotFound));
    assert!(not_found(
        get_document(&stranger, document.id, f.stores())
            .await
            .map(drop)
    ));
    assert!(not_found(
        list_versions(&stranger, document.id, f.stores())
            .await
            .map(drop)
    ));
    assert!(not_found(
        download(&stranger, document.newest_version.id, f.stores())
            .await
            .map(drop)
    ));
    assert!(not_found(
        list_documents(
            &stranger,
            f.event,
            None,
            None,
            PageLimit::DEFAULT,
            f.stores()
        )
        .await
        .map(drop)
    ));
    let error = upload_version(
        &stranger,
        document.id,
        "Programm.pdf",
        body(&pdf("Fremd")),
        LIMIT,
        f.stores(),
        &SystemClock,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, UploadError::NotFound), "{error:?}");
}

#[tokio::test]
async fn a_member_without_an_event_role_cannot_see_the_documents() {
    let f = Fixture::start().await;
    let document = f
        .upload(&f.owner, "Programm.pdf", &pdf("Programm"))
        .await
        .unwrap();
    let (_, outsider) = member(&f.test, "testwil", OrganizationRole::Member).await;
    assert!(matches!(
        get_document(&outsider, document.id, f.stores()).await,
        Err(ReadDocumentError::NotFound)
    ));
    assert!(matches!(
        f.upload(&outsider, "Plan.pdf", &pdf("Plan")).await,
        Err(UploadError::NotFound)
    ));
}

#[tokio::test]
async fn finds_documents_by_name_and_stores_the_text_of_a_text_file() {
    let f = Fixture::start().await;
    f.upload(&f.owner, "Programm.pdf", &pdf("Programm"))
        .await
        .unwrap();
    let notes = f
        .upload(
            &f.owner,
            "Notizen\u{202E}.md",
            b"# Notizen\r\nFlugshow um 14 Uhr\r\n",
        )
        .await
        .unwrap();
    assert_eq!(notes.readable_id(), "DOC-002");
    assert_eq!(
        notes.name, "Notizen.md",
        "the name has no bidirectional control"
    );

    let found = list_documents(
        &f.owner,
        f.event,
        Some(" notiz "),
        None,
        PageLimit::DEFAULT,
        f.stores(),
    )
    .await
    .unwrap();
    assert_eq!(found.items, std::slice::from_ref(&notes));
    let page = list_documents(
        &f.owner,
        f.event,
        None,
        None,
        PageLimit::new(1).unwrap(),
        f.stores(),
    )
    .await
    .unwrap();
    assert_eq!(page.items, std::slice::from_ref(&notes));
    let rest = list_documents(
        &f.owner,
        f.event,
        None,
        page.next,
        PageLimit::new(1).unwrap(),
        f.stores(),
    )
    .await
    .unwrap();
    assert_eq!(rest.items[0].readable_id(), "DOC-001");
    assert_eq!(rest.next, None);

    // The text of a text file is searchable and citable, in its normalized form (OP10).
    let text: String = f
        .test
        .scalar(&format!(
            "SELECT text FROM source_version WHERE id = '{}'",
            notes.newest_version.source_version_id
        ))
        .await;
    assert_eq!(text, "# Notizen\nFlugshow um 14 Uhr\n");
}
