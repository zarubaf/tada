//! The `export` command: all data of one organization as files in a directory (ADR 0059).

use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use async_trait::async_trait;
use futures::TryStreamExt;
use tada_adapters::clock::SystemClock;
use tada_app::blobs::{BlobStore, ByteStream};
use tada_app::caller::{Exporter, ServiceCaller};
use tada_app::clock::Clock;
use tada_app::domain::identity::OrganizationSlug;
use tada_app::export::{ExportSink, ExportSource, ExportSummary, export_organization};
use tada_store_pg::Database;
use tokio::io::AsyncWriteExt;

use crate::settings::ExportSettings;

/// The arguments of `tada export`.
#[derive(Debug, Clone)]
pub struct ExportCommand {
    pub organization_slug: OrganizationSlug,
    /// A new or empty directory for the files of the export.
    pub output: PathBuf,
}

pub async fn run(
    (database, storage): ExportSettings,
    command: ExportCommand,
) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let result = execute(&db, &storage.open(), &SystemClock, command).await;
    db.close().await;
    let summary = result?;
    tracing::info!(
        organization_id = %summary.organization_id,
        tables = summary.tables,
        rows = summary.rows,
        blobs = summary.blobs,
        "the export is complete"
    );
    Ok(())
}

/// Exports the organization of the command into its output directory.
pub async fn execute(
    source: &dyn ExportSource,
    blobs: &dyn BlobStore,
    clock: &dyn Clock,
    command: ExportCommand,
) -> anyhow::Result<ExportSummary> {
    let mut sink = DirectorySink::create(&command.output)
        .context("cannot use the output directory; it must be new or empty")?;
    let summary = export_organization(
        &ServiceCaller::<Exporter>::new(),
        &command.organization_slug,
        source,
        blobs,
        &mut sink,
        clock,
        env!("CARGO_PKG_VERSION"),
    )
    .await
    .context("the export failed; remove the output directory before the next run")?;
    Ok(summary)
}

/// Writes the files of an export below one directory.
/// The directories have mode 0700 and the files mode 0600: an export holds personal data (ADR 0059).
#[derive(Debug)]
pub struct DirectorySink {
    root: PathBuf,
}

impl DirectorySink {
    /// Creates `root`, or uses it if it is an empty directory.
    pub fn create(root: &Path) -> io::Result<Self> {
        match std::fs::read_dir(root) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the directory is not empty",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                DirBuilder::new().recursive(true).mode(0o700).create(root)?;
            }
            Err(error) => return Err(error),
        }
        Ok(Self {
            root: root.to_owned(),
        })
    }
}

#[async_trait]
impl ExportSink for DirectorySink {
    async fn write(&mut self, path: &str, mut content: ByteStream) -> io::Result<()> {
        let relative = Path::new(path);
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an export path must stay inside the export",
            ));
        }
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .await?;
        while let Some(chunk) = content.try_next().await? {
            file.write_all(&chunk).await?;
        }
        file.sync_all().await
    }
}
