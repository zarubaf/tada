//! The `redactions replay` command: applies the deletion journal again after a restore of the database (ADR 0072).

use anyhow::Context;
use tada_app::redaction::replay;
use tada_store_pg::Database;

use crate::settings::RedactionsSettings;

pub async fn run((database, storage): RedactionsSettings) -> anyhow::Result<()> {
    let db = Database::connect_lazy(&database.url, &database.password)
        .context("invalid database settings")?;
    let result = replay(&db, &storage.open()).await;
    db.close().await;
    let report = result.context("the replay of the deletion journal failed")?;
    if report.undeleted_objects > 0 {
        tracing::warn!(
            undeleted_objects = report.undeleted_objects,
            "some objects of redacted versions stay until the sweep of orphan objects"
        );
    }
    tracing::info!(
        read = report.read,
        applied = report.applied,
        already = report.already,
        missing = report.missing,
        "the replay of the deletion journal is complete"
    );
    Ok(())
}
