//! The schema upgrade test (ADR 0006): data at an old schema keeps its values through all later migrations.
//!
//! The test applies the migrations up to `CUT`, loads the fixture of that schema, and then upgrades the database
//! as `tada migrate` does.
//! Each later slice moves the cut to its own last migration and adds a fixture of that schema.
//! The old fixtures stay, so each schema of a release keeps a test.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, PgPool};
use tada_app::domain::facts::{CORE_CATALOG_VERSION, core_catalog};
use tada_store_pg::testing::TestDatabase;

/// The last migration of the fixture schema.
/// Documents come in migration 0014 and drafts in 0015, so the fixture cannot be older.
const CUT: i64 = 15;

/// Invented data of one organization at the schema of `CUT`.
const FIXTURE: &str = include_str!("fixtures/0015.sql");

/// The rows of each table, as JSON text in a fixed order, with the columns of each table at the cut only.
type Snapshot = BTreeMap<String, Vec<String>>;

/// The columns of each table of the schema, without the migration table of sqlx.
async fn columns(pool: &PgPool) -> BTreeMap<String, Vec<String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT quote_ident(table_name), quote_ident(column_name)
         FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations'
         ORDER BY table_name, ordinal_position",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut tables = BTreeMap::<String, Vec<String>>::new();
    for (table, column) in rows {
        tables.entry(table).or_default().push(column);
    }
    tables
}

/// The rows of the tables, with the given columns of each table.
async fn snapshot(pool: &PgPool, tables: &BTreeMap<String, Vec<String>>) -> Snapshot {
    let mut snapshot = Snapshot::new();
    for (table, columns) in tables {
        // The names come from the catalog, quoted by `quote_ident`.
        let rows: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT row_to_json(r)::text FROM (SELECT {} FROM {table}) AS r ORDER BY 1",
            columns.join(", ")
        )))
        .fetch_all(pool)
        .await
        .unwrap();
        snapshot.insert(table.clone(), rows);
    }
    snapshot
}

async fn text(pool: &PgPool, sql: &'static str) -> Vec<String> {
    sqlx::query_scalar(sql).fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn an_upgrade_keeps_the_concept_the_sources_the_relationships_and_the_old_versions() {
    let test = TestDatabase::start_empty().await;
    let pool = test.pool();

    // The migrations of the cut, filtered by version: the numbers of the migrations have gaps.
    let all = sqlx::migrate!();
    let old = Migrator::with_migrations(
        all.iter()
            .filter(|migration| migration.version <= CUT)
            .cloned()
            .collect(),
    );
    assert!(
        old.iter().any(|migration| migration.version == CUT),
        "the cut names a migration"
    );
    assert!(
        all.iter().any(|migration| migration.version > CUT),
        "the upgrade applies at least one migration"
    );
    old.run(pool).await.unwrap();
    sqlx::raw_sql(FIXTURE).execute(pool).await.unwrap();

    let old_columns = columns(pool).await;
    let before = snapshot(pool, &old_columns).await;

    // The upgrade of `tada migrate`.
    test.database.migrate().await.unwrap();
    test.database
        .sync_catalog(&core_catalog(), CORE_CATALOG_VERSION)
        .await
        .unwrap();

    let applied: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(applied, all.iter().map(|m| m.version).max().unwrap());

    // Each row of the fixture stays, with the values of the columns of the old schema.
    // The only new rows are the shipped fields that the catalog sync adds: field definitions without an event.
    let after = snapshot(pool, &old_columns).await;
    for (table, rows) in &before {
        for row in rows {
            assert!(
                after[table].contains(row),
                "the upgrade changed or removed a row of {table}: {row}"
            );
        }
        for row in after[table].iter().filter(|row| !rows.contains(row)) {
            let row_value: serde_json::Value = serde_json::from_str(row).unwrap();
            let shipped_field = table == "field_definition"
                && row_value.get("event_id") == Some(&serde_json::Value::Null);
            assert!(shipped_field, "the upgrade added a row to {table}: {row}");
        }
    }

    // The concept: the old version and the approved version, with their Markdown.
    assert_eq!(
        text(
            pool,
            "SELECT d.name || ' ' || v.number || ' ' || v.status || ' ' || v.markdown
             FROM document d JOIN document_version v ON v.document_id = d.id
             WHERE v.kind = 'draft' ORDER BY v.number"
        )
        .await,
        [
            "Vorläufiges Konzept 1 superseded # Vorläufiges Konzept\n\n\
             Das Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=1).\n\
             Der Anlass heisst [Open Day Testwil](tada:source/0190a000-0000-7000-8000-000000000012#14-30).\n",
            "Vorläufiges Konzept 2 approved # Vorläufiges Konzept\n\n\
             Das Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=2).\n\
             Die Quelle sagt [Flieg mit uns](tada:source/0190a000-0000-7000-8000-000000000013#19-32).\n",
        ]
    );

    // The manifest of each concept version names the exact fact version and source passage.
    assert_eq!(
        text(
            pool,
            "SELECT v.number || ': ' || (f.value ->> 'text') || ' / ' || substr(s.text, ms.start_offset + 1,
                    ms.end_offset - ms.start_offset)
             FROM document_version v
             JOIN document_manifest_fact mf ON mf.document_version_id = v.id
             JOIN fact_version f ON f.fact_id = mf.fact_id AND f.number = mf.fact_version_number
             JOIN document_manifest_source ms ON ms.document_version_id = v.id
             JOIN source_version s ON s.id = ms.source_version_id
             ORDER BY v.number"
        )
        .await,
        [
            "1: Flieg mit Testwil / Open Day Testwil",
            "2: Flieg mit uns / Flieg mit uns"
        ]
    );

    // The sources: both versions of the member text and both versions of the upload.
    assert_eq!(
        text(
            pool,
            "SELECT kind || ' ' || coalesce(text, encode(sha256, 'hex'))
             FROM source_version ORDER BY captured_at"
        )
        .await,
        [
            "member-text Das Motto des Open Day Testwil ist Flieg mit Testwil.",
            "upload 42492b58eb3472305338d85669bef334e06009ff3509a3a5ccf831cabe3e5284",
            "member-text Neu: Das Motto ist Flieg mit uns.",
            "upload 3864af8faebd4fd52df9bb240247c958031918cb7d0dad7cd54461b048157daa",
        ]
    );

    // The relationships: the dependency of the motto on its field, and the evidence of each fact version.
    assert_eq!(
        text(
            pool,
            "SELECT (p.operation ->> 'kind') || ' needs ' || (d.operation ->> 'kind')
             FROM proposal_dependency pd
             JOIN proposal p ON p.id = pd.proposal_id
             JOIN proposal d ON d.id = pd.depends_on"
        )
        .await,
        ["set_fact needs add_field_definition"]
    );

    // The old versions: the old motto with its evidence, and the old upload.
    assert_eq!(
        text(
            pool,
            "SELECT f.number || ' ' || (f.value ->> 'text') || ' ' || e.quote || ' ' || (fact.version = f.number)
             FROM fact_version f
             JOIN fact ON fact.id = f.fact_id
             JOIN evidence_link e ON e.fact_version_id = f.id
             ORDER BY f.number"
        )
        .await,
        [
            "1 Flieg mit Testwil Flieg mit Testwil false",
            "2 Flieg mit uns Flieg mit uns true"
        ]
    );
    assert_eq!(
        text(
            pool,
            "SELECT number || ' ' || blob_key FROM document_version WHERE kind = 'upload' ORDER BY number"
        )
        .await,
        [
            "1 0190a000-0000-7000-8000-000000000001/0190a000-0000-7000-8000-000000000192",
            "2 0190a000-0000-7000-8000-000000000001/0190a000-0000-7000-8000-000000000193",
        ]
    );
}
