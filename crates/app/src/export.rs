//! The structured export of one organization (ADR 0059): JSON Lines and CSV for each table,
//! the original file of each upload version, and a manifest with the hash of each file and the
//! relationships of the tables.
//!
//! The writer reads through `ExportSource` and `BlobStore` and writes through `ExportSink`.
//! It holds no I/O of its own.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{TryStreamExt, stream};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tada_domain::identity::OrganizationSlug;
use tada_domain::ids::OrganizationId;

use crate::blobs::{BlobError, BlobKey, BlobStore, ByteStream};
use crate::caller::{Exporter, OrgScope, ServiceCaller};
use crate::clock::Clock;
use crate::store::StoreError;

/// The version of the layout of an export. A reader checks it before it reads the files.
pub const FORMAT_VERSION: u32 = 1;

/// The path of the manifest in an export.
pub const MANIFEST: &str = "manifest.json";

/// A foreign key of an exported table: a relationship to another table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    /// The table that the columns refer to.
    pub table: String,
    /// The columns of `table` that the columns refer to, in the same order.
    pub references: Vec<String>,
}

/// The exported rows of one table. Each row has one value for each column, in the same order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRows {
    pub name: String,
    pub columns: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    /// The values in the PostgreSQL JSON form, for example a `bytea` as `"\\x00ff"`.
    pub rows: Vec<Vec<Value>>,
}

/// The object of an upload version and the SHA-256 of its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
    pub key: BlobKey,
    pub sha256: [u8; 32],
}

/// All exported data of one organization, from one consistent read.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Each table comes after the tables that it refers to.
    pub tables: Vec<TableRows>,
    pub blobs: Vec<StoredBlob>,
}

/// The repository port of the export.
#[async_trait]
pub trait ExportSource: Send + Sync {
    /// The organization with the slug, if one exists.
    /// Infrastructure query (ADR 0039): the slug of the operator names the organization.
    async fn organization_by_slug(
        &self,
        caller: &ServiceCaller<Exporter>,
        slug: &OrganizationSlug,
    ) -> Result<Option<OrganizationId>, StoreError>;

    /// All exported rows of the organization and the objects of its upload versions, in one
    /// read-only transaction. Fails if the schema has a table without an export decision.
    async fn snapshot(&self, scope: OrgScope) -> Result<Snapshot, StoreError>;
}

/// The place where an export goes, for example a directory.
#[async_trait]
pub trait ExportSink: Send {
    /// Writes a new file. `path` is relative to the export and uses `/`, for example `tables/event.csv`.
    async fn write(&mut self, path: &str, content: ByteStream) -> io::Result<()>;
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("no organization has this slug")]
    UnknownOrganization,
    /// The object storage has no object for an upload version.
    #[error("the object {key} of an upload version is missing")]
    MissingBlob { key: BlobKey },
    /// The content of an object does not have the hash of its upload version.
    #[error("the object {key} does not match the hash of its upload version")]
    HashMismatch { key: BlobKey },
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Blob(#[from] BlobError),
    #[error("cannot write the export")]
    Write(#[source] io::Error),
}

/// What an export contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportSummary {
    pub organization_id: OrganizationId,
    pub tables: usize,
    pub rows: usize,
    pub blobs: usize,
}

#[derive(Debug, Serialize)]
struct Manifest<'a> {
    format_version: u32,
    tada_version: &'a str,
    created_at: jiff::Timestamp,
    organization_id: uuid::Uuid,
    tables: Vec<TableEntry<'a>>,
    files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
struct TableEntry<'a> {
    name: &'a str,
    columns: &'a [String],
    row_count: usize,
    foreign_keys: &'a [ForeignKey],
}

#[derive(Debug, Serialize)]
struct FileEntry {
    path: String,
    size_bytes: u64,
    sha256: String,
}

/// Writes all data of the organization with the slug into the sink (ADR 0059).
/// `tada_version` is the version of the binary, for the manifest.
pub async fn export_organization(
    caller: &ServiceCaller<Exporter>,
    slug: &OrganizationSlug,
    source: &dyn ExportSource,
    blobs: &dyn BlobStore,
    sink: &mut dyn ExportSink,
    clock: &dyn Clock,
    tada_version: &str,
) -> Result<ExportSummary, ExportError> {
    let organization_id = source
        .organization_by_slug(caller, slug)
        .await?
        .ok_or(ExportError::UnknownOrganization)?;
    let snapshot = source.snapshot(caller.scope(organization_id)).await?;

    let mut files = Vec::new();
    for table in &snapshot.tables {
        let jsonl = format!("tables/{}.jsonl", table.name);
        files.push(write_bytes(sink, jsonl, json_lines(table)).await?);
        let csv = format!("tables/{}.csv", table.name);
        files.push(write_bytes(sink, csv, csv_text(table)).await?);
    }
    let mut written = BTreeSet::new();
    for blob in &snapshot.blobs {
        if written.insert(blob.sha256) {
            files.push(write_blob(sink, blobs, blob).await?);
        }
    }

    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        tada_version,
        created_at: clock.now(),
        organization_id: organization_id.as_uuid(),
        tables: snapshot
            .tables
            .iter()
            .map(|table| TableEntry {
                name: &table.name,
                columns: &table.columns,
                row_count: table.rows.len(),
                foreign_keys: &table.foreign_keys,
            })
            .collect(),
        files,
    };
    let mut manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|error| ExportError::Write(error.into()))?;
    manifest.push(b'\n');
    write_bytes(sink, MANIFEST.to_owned(), manifest).await?;

    Ok(ExportSummary {
        organization_id,
        tables: snapshot.tables.len(),
        rows: snapshot.tables.iter().map(|table| table.rows.len()).sum(),
        blobs: written.len(),
    })
}

async fn write_bytes(
    sink: &mut dyn ExportSink,
    path: String,
    content: Vec<u8>,
) -> Result<FileEntry, ExportError> {
    let entry = FileEntry {
        size_bytes: content.len() as u64,
        sha256: hex(&Sha256::digest(&content).into()),
        path,
    };
    let body: ByteStream = Box::pin(stream::once(async { Ok(Bytes::from(content)) }));
    sink.write(&entry.path, body)
        .await
        .map_err(ExportError::Write)?;
    Ok(entry)
}

/// Copies the object of an upload version to `blobs/<sha256>` and checks its hash on the way.
async fn write_blob(
    sink: &mut dyn ExportSink,
    blobs: &dyn BlobStore,
    blob: &StoredBlob,
) -> Result<FileEntry, ExportError> {
    let content = blobs
        .get(&blob.key)
        .await?
        .ok_or_else(|| ExportError::MissingBlob {
            key: blob.key.clone(),
        })?;
    let digest = Arc::new(Mutex::new((Sha256::new(), 0_u64)));
    let counted = digest.clone();
    let content: ByteStream = Box::pin(content.inspect_ok(move |chunk| {
        let mut state = counted.lock().unwrap_or_else(PoisonError::into_inner);
        state.0.update(chunk);
        state.1 += chunk.len() as u64;
    }));
    let path = format!("blobs/{}", hex(&blob.sha256));
    sink.write(&path, content)
        .await
        .map_err(ExportError::Write)?;
    let (sha256, size_bytes): ([u8; 32], u64) = {
        let state = digest.lock().unwrap_or_else(PoisonError::into_inner);
        (state.0.clone().finalize().into(), state.1)
    };
    if sha256 != blob.sha256 {
        return Err(ExportError::HashMismatch {
            key: blob.key.clone(),
        });
    }
    Ok(FileEntry {
        path,
        size_bytes,
        sha256: hex(&sha256),
    })
}

/// One JSON object for each row, with the columns in their order.
fn json_lines(table: &TableRows) -> Vec<u8> {
    let mut out = String::new();
    for row in &table.rows {
        out.push('{');
        for (index, (column, value)) in table.columns.iter().zip(row).enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(out, "{}:{value}", Value::from(column.as_str()));
        }
        out.push_str("}\n");
    }
    out.into_bytes()
}

/// RFC 4180 CSV with a header line. A null is an empty field; a string is its text; other
/// values are their JSON text.
fn csv_text(table: &TableRows) -> Vec<u8> {
    let mut out = String::new();
    let header: Vec<&str> = table.columns.iter().map(String::as_str).collect();
    csv_line(&mut out, header.into_iter().map(std::borrow::Cow::Borrowed));
    for row in &table.rows {
        csv_line(
            &mut out,
            row.iter().map(|value| match value {
                Value::Null => "".into(),
                Value::String(text) => text.as_str().into(),
                other => other.to_string().into(),
            }),
        );
    }
    out.into_bytes()
}

fn csv_line<'a>(out: &mut String, fields: impl Iterator<Item = std::borrow::Cow<'a, str>>) {
    for (index, field) in fields.enumerate() {
        if index > 0 {
            out.push(',');
        }
        if field.contains([',', '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&field.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&field);
        }
    }
    out.push_str("\r\n");
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use futures::StreamExt;
    use jiff::Timestamp;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    const NOW: &str = "2030-05-18T08:00:00Z";

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            NOW.parse().unwrap()
        }
    }

    fn organization() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(7))
    }

    fn key() -> BlobKey {
        BlobKey::restore(format!("{}/1", organization()))
    }

    struct Source(Snapshot);

    #[async_trait]
    impl ExportSource for Source {
        async fn organization_by_slug(
            &self,
            _: &ServiceCaller<Exporter>,
            slug: &OrganizationSlug,
        ) -> Result<Option<OrganizationId>, StoreError> {
            Ok((slug.as_str() == "testwil").then(organization))
        }

        async fn snapshot(&self, scope: OrgScope) -> Result<Snapshot, StoreError> {
            assert_eq!(scope.organization_id(), organization());
            Ok(self.0.clone())
        }
    }

    #[derive(Debug, Default)]
    struct Blobs(BTreeMap<String, Vec<u8>>);

    #[async_trait]
    impl BlobStore for Blobs {
        async fn put(&self, _: &BlobKey, _: ByteStream, _: u64) -> Result<u64, BlobError> {
            unreachable!("the export never writes objects")
        }

        async fn get(&self, key: &BlobKey) -> Result<Option<ByteStream>, BlobError> {
            Ok(self.0.get(key.as_str()).map(|content| {
                let chunks: Vec<Result<Bytes, io::Error>> = content
                    .chunks(3)
                    .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
                    .collect();
                Box::pin(stream::iter(chunks)) as ByteStream
            }))
        }

        async fn head(&self, _: &BlobKey) -> Result<Option<u64>, BlobError> {
            unreachable!("the export never reads sizes")
        }

        async fn delete(&self, _: &BlobKey) -> Result<(), BlobError> {
            unreachable!("the export never deletes objects")
        }
    }

    #[derive(Debug, Default)]
    struct Sink(BTreeMap<String, Vec<u8>>);

    #[async_trait]
    impl ExportSink for Sink {
        async fn write(&mut self, path: &str, mut content: ByteStream) -> io::Result<()> {
            let mut file = Vec::new();
            while let Some(chunk) = content.next().await {
                file.extend_from_slice(&chunk?);
            }
            assert!(self.0.insert(path.to_owned(), file).is_none(), "{path}");
            Ok(())
        }
    }

    fn snapshot(content: &[u8]) -> Snapshot {
        Snapshot {
            tables: vec![TableRows {
                name: "event".to_owned(),
                columns: vec!["id".to_owned(), "name".to_owned(), "version".to_owned()],
                foreign_keys: vec![ForeignKey {
                    columns: vec!["organization_id".to_owned()],
                    table: "organization".to_owned(),
                    references: vec!["id".to_owned()],
                }],
                rows: vec![
                    vec![json!("e1"), json!("Open Day, \"Testwil\""), json!(1)],
                    vec![json!("e2"), Value::Null, json!({"a": [1]})],
                ],
            }],
            blobs: vec![
                StoredBlob {
                    key: key(),
                    sha256: Sha256::digest(content).into(),
                },
                StoredBlob {
                    key: key(),
                    sha256: Sha256::digest(content).into(),
                },
            ],
        }
    }

    async fn run(
        source: &Source,
        blobs: &Blobs,
        slug: &str,
    ) -> (Result<ExportSummary, ExportError>, Sink) {
        let mut sink = Sink::default();
        let result = export_organization(
            &ServiceCaller::new(),
            &OrganizationSlug::parse(slug).unwrap(),
            source,
            blobs,
            &mut sink,
            &FixedClock,
            "1.2.3",
        )
        .await;
        (result, sink)
    }

    fn blobs(content: &[u8]) -> Blobs {
        Blobs(BTreeMap::from([(
            key().as_str().to_owned(),
            content.to_vec(),
        )]))
    }

    #[tokio::test]
    async fn an_export_writes_json_lines_csv_the_originals_and_a_manifest() {
        let content = b"%PDF-1.4 Programm";
        let (result, sink) = run(&Source(snapshot(content)), &blobs(content), "testwil").await;
        let summary = result.unwrap();
        assert_eq!(
            summary,
            ExportSummary {
                organization_id: organization(),
                tables: 1,
                rows: 2,
                blobs: 1,
            }
        );

        let jsonl = String::from_utf8(sink.0["tables/event.jsonl"].clone()).unwrap();
        assert_eq!(
            jsonl,
            "{\"id\":\"e1\",\"name\":\"Open Day, \\\"Testwil\\\"\",\"version\":1}\n\
             {\"id\":\"e2\",\"name\":null,\"version\":{\"a\":[1]}}\n"
        );
        let csv = String::from_utf8(sink.0["tables/event.csv"].clone()).unwrap();
        assert_eq!(
            csv,
            "id,name,version\r\ne1,\"Open Day, \"\"Testwil\"\"\",1\r\ne2,,\"{\"\"a\"\":[1]}\"\r\n"
        );

        let hash = hex(&Sha256::digest(content).into());
        assert_eq!(sink.0[&format!("blobs/{hash}")], content);

        let manifest: Value = serde_json::from_slice(&sink.0[MANIFEST]).unwrap();
        assert_eq!(manifest["format_version"], 1);
        assert_eq!(manifest["tada_version"], "1.2.3");
        assert_eq!(manifest["created_at"], NOW);
        assert_eq!(manifest["organization_id"], organization().to_string());
        assert_eq!(
            manifest["tables"],
            json!([{
                "name": "event",
                "columns": ["id", "name", "version"],
                "row_count": 2,
                "foreign_keys": [{"columns": ["organization_id"], "table": "organization", "references": ["id"]}],
            }])
        );
        let files = manifest["files"].as_array().unwrap();
        assert_eq!(
            files.len(),
            sink.0.len() - 1,
            "each file except the manifest"
        );
        for file in files {
            let content = &sink.0[file["path"].as_str().unwrap()];
            assert_eq!(file["size_bytes"], content.len());
            assert_eq!(file["sha256"], hex(&Sha256::digest(content).into()));
        }
    }

    #[tokio::test]
    async fn an_object_with_another_hash_fails_the_export() {
        let (result, _) = run(
            &Source(snapshot(b"the stored file")),
            &blobs(b"another file"),
            "testwil",
        )
        .await;
        assert!(
            matches!(result, Err(ExportError::HashMismatch { ref key }) if *key == super::tests::key()),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn a_missing_object_fails_the_export() {
        let (result, _) = run(&Source(snapshot(b"file")), &Blobs::default(), "testwil").await;
        assert!(
            matches!(result, Err(ExportError::MissingBlob { .. })),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn an_unknown_slug_fails_and_writes_nothing() {
        let (result, sink) = run(&Source(snapshot(b"file")), &blobs(b"file"), "musterhausen").await;
        assert!(
            matches!(result, Err(ExportError::UnknownOrganization)),
            "{result:?}"
        );
        assert!(sink.0.is_empty());
    }
}
