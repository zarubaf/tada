//! The `ExportSource` adapter (ADR 0059). One list decides for each table of the schema if the
//! export holds it, and how it selects the rows of one organization.

use std::collections::BTreeSet;

use async_trait::async_trait;
use sqlx::types::Uuid;
use sqlx::{AssertSqlSafe, PgConnection};
use tada_app::blobs::BlobKey;
use tada_app::caller::{Exporter, OrgScope, ServiceCaller};
use tada_app::domain::identity::OrganizationSlug;
use tada_app::domain::ids::OrganizationId;
use tada_app::export::{ExportSource, ForeignKey, Snapshot, StoredBlob, TableRows};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};

/// How the export selects the rows of one organization from a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rows {
    /// The row of the organization itself.
    Organization,
    /// The rows with the organization in the column `organization_id`.
    OfOrganization,
    /// The rows of the users that the exported rows refer to, by the user ID in this column.
    OfUsers(&'static str),
}

/// A table that the export holds.
#[derive(Debug)]
pub(crate) struct Exported {
    pub(crate) table: &'static str,
    pub(crate) rows: Rows,
    /// The columns that the export holds, in the order of the table.
    pub(crate) columns: &'static [&'static str],
    /// Columns with secrets, for example a token hash. The export leaves them out (ADR 0059).
    pub(crate) secret_columns: &'static [&'static str],
}

const fn exported(table: &'static str, rows: Rows, columns: &'static [&'static str]) -> Exported {
    Exported {
        table,
        rows,
        columns,
        secret_columns: &[],
    }
}

/// The exported tables, each after the tables that it refers to. An import inserts them in this order.
/// Each column of an exported table is in `columns` or in `secret_columns`.
/// A new column stops the export until a migration author adds it to one of the two lists (ADR 0059).
pub(crate) const EXPORTED: &[Exported] = &[
    exported(
        "organization",
        Rows::Organization,
        &[
            "id",
            "slug",
            "name",
            "created_at",
            "storage_quota_bytes",
            "privacy_notice",
            "privacy_notice_version",
        ],
    ),
    exported(
        "app_user",
        Rows::OfUsers("id"),
        &["id", "display_name", "locale", "created_at"],
    ),
    exported(
        "email_identity",
        Rows::OfUsers("user_id"),
        &["user_id", "email", "created_at"],
    ),
    exported(
        "telegram_identity",
        Rows::OfUsers("user_id"),
        &["telegram_user_id", "user_id", "linked_at"],
    ),
    exported(
        "organization_membership",
        Rows::OfOrganization,
        &[
            "organization_id",
            "user_id",
            "role",
            "version",
            "created_at",
        ],
    ),
    exported(
        "organization_feature",
        Rows::OfOrganization,
        &["organization_id", "feature", "enabled", "version"],
    ),
    exported(
        "event",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "key",
            "name",
            "time_zone",
            "version",
            "created_at",
        ],
    ),
    exported(
        "event_membership",
        Rows::OfOrganization,
        &[
            "organization_id",
            "event_id",
            "user_id",
            "event_role",
            "version",
            "created_at",
        ],
    ),
    exported(
        "invitation",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "email",
            "display_name",
            "role",
            "invited_by",
            "status",
            "created_at",
            "accepted_at",
            "revoked_at",
        ],
    ),
    exported(
        "outbound_intent",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "user_id",
            "invitation_id",
            "purpose",
            "status",
            "message_id",
            "request_id",
            "created_at",
            "finished_at",
        ],
    ),
    exported(
        "audit_event",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "occurred_at",
            "actor_kind",
            "actor_id",
            "principal_id",
            "channel",
            "request_id",
            "action",
            "record_kind",
            "record_id",
            "subject_user_id",
            "detail",
        ],
    ),
    Exported {
        table: "api_token",
        rows: Rows::OfOrganization,
        columns: &[
            "id",
            "organization_id",
            "user_id",
            "name",
            "scope",
            "expires_at",
            "notice_version",
            "notice_confirmed_at",
            "created_at",
            "last_used_at",
            "revoked_at",
        ],
        secret_columns: &["token_hash"],
    },
    exported(
        "local_id_counter",
        Rows::OfOrganization,
        &["organization_id", "scope_id", "kind", "next"],
    ),
    exported(
        "field_definition",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "key",
            "label_text",
            "label_message",
            "value_type",
            "description",
            "module",
            "status",
            "catalog_version",
            "created_at",
        ],
    ),
    exported(
        "source_item",
        Rows::OfOrganization,
        &["id", "organization_id", "event_id", "kind", "created_at"],
    ),
    exported(
        "source_version",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "source_item_id",
            "kind",
            "channel",
            "author_actor",
            "text",
            "sha256",
            "url",
            "retrieved_at",
            "captured_at",
        ],
    ),
    exported(
        "changeset",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "author",
            "source_version_id",
            "created_at",
        ],
    ),
    exported(
        "proposal",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "changeset_id",
            "event_id",
            "operation",
            "operation_version",
            "target_kind",
            "target_id",
            "expected_version",
            "reason",
            "created_at",
            "manifest",
            "lint_warnings",
        ],
    ),
    exported(
        "proposal_dependency",
        Rows::OfOrganization,
        &[
            "organization_id",
            "changeset_id",
            "proposal_id",
            "depends_on",
        ],
    ),
    exported(
        "proposal_evidence",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "proposal_id",
            "source_version_id",
            "start_offset",
            "end_offset",
            "quote",
            "page",
        ],
    ),
    exported(
        "review_result",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "proposal_id",
            "result",
            "reviewer",
            "edit_source_version_id",
            "created_at",
        ],
    ),
    exported(
        "fact",
        Rows::OfOrganization,
        &["id", "organization_id", "event_id", "field_id", "version"],
    ),
    exported(
        "fact_version",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "fact_id",
            "number",
            "state",
            "value",
            "approximate",
            "created_at",
            "accepted_by",
            "proposal_id",
        ],
    ),
    exported(
        "evidence_link",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "fact_version_id",
            "source_version_id",
            "start_offset",
            "end_offset",
            "quote",
            "page",
        ],
    ),
    exported(
        "open_question",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "local_number",
            "text",
            "owner_user_id",
            "status",
            "version",
            "created_at",
        ],
    ),
    exported(
        "document",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "local_number",
            "name",
            "owner_user_id",
            "created_at",
            "version",
        ],
    ),
    exported(
        "document_version",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "document_id",
            "number",
            "kind",
            "blob_key",
            "media_type",
            "size_bytes",
            "sha256",
            "file_name",
            "uploaded_by",
            "source_version_id",
            "status",
            "created_at",
            "approved_by",
            "approved_at",
            "markdown",
        ],
    ),
    exported(
        "document_manifest_fact",
        Rows::OfOrganization,
        &[
            "organization_id",
            "document_version_id",
            "fact_id",
            "fact_version_number",
        ],
    ),
    exported(
        "document_manifest_source",
        Rows::OfOrganization,
        &[
            "organization_id",
            "document_version_id",
            "source_version_id",
            "start_offset",
            "end_offset",
        ],
    ),
    exported(
        "person",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "local_number",
            "name",
            "email",
            "phone",
            "user_id",
            "version",
            "created_at",
            "updated_at",
        ],
    ),
    exported(
        "institution",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "local_number",
            "name",
            "kind",
            "email",
            "phone",
            "version",
            "created_at",
            "updated_at",
        ],
    ),
    exported(
        "workstream",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "name",
            "lead_user_id",
            "status",
            "version",
            "created_at",
            "updated_at",
        ],
    ),
    exported(
        "action",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "local_number",
            "title",
            "description",
            "owner_user_id",
            "workstream_id",
            "due_date",
            "status",
            "version",
            "created_at",
            "updated_at",
        ],
    ),
    exported(
        "commitment",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "event_id",
            "local_number",
            "text",
            "condition",
            "person_id",
            "institution_id",
            "owner_user_id",
            "workstream_id",
            "due_date",
            "status",
            "version",
            "created_at",
            "updated_at",
        ],
    ),
    exported(
        "record_evidence",
        Rows::OfOrganization,
        &[
            "id",
            "organization_id",
            "action_id",
            "commitment_id",
            "person_id",
            "institution_id",
            "record_version",
            "proposal_id",
            "source_version_id",
            "start_offset",
            "end_offset",
            "quote",
            "page",
        ],
    ),
];

/// The tables that the export leaves out, with the reason (ADR 0059).
pub(crate) const NOT_EXPORTED: &[(&str, &str)] = &[
    ("session", "secret: it holds token hashes"),
    ("magic_link", "secret: it holds token hashes"),
    ("invitation_token", "secret: it holds token hashes"),
    (
        "telegram_link_code",
        "secret: it holds the hashes of link codes",
    ),
    (
        "rate_limit_counter",
        "secret: it holds HMAC keys of addresses",
    ),
    ("job", "the queue of the worker: pending work, not a record"),
    (
        "worker_heartbeat",
        "state of the installation, no organization data",
    ),
    (
        "telegram_update",
        "state of the installation, no organization data",
    ),
    (
        "_sqlx_migrations",
        "the schema history; the format and tada versions name the schema",
    ),
];

/// The export decision of the table, if the list has one.
pub(crate) fn exported_table(table: &str) -> Option<&'static Exported> {
    EXPORTED.iter().find(|exported| exported.table == table)
}

/// A table of the schema without an export decision. A new migration must add it to `EXPORTED`
/// or to `NOT_EXPORTED` (ADR 0059).
#[derive(Debug, thiserror::Error)]
#[error("the table {0} has no export decision; add it to EXPORTED or NOT_EXPORTED")]
struct UndecidedTable(String);

/// A column of an exported table without an export decision. A new migration must add it to
/// `columns` or to `secret_columns` of the table (ADR 0059).
#[derive(Debug, thiserror::Error)]
#[error(
    "the column {0}.{1} has no export decision; add it to columns or secret_columns of the table"
)]
struct UndecidedColumn(&'static str, String);

/// A column of the lists that the table does not have, for example after a rename.
#[derive(Debug, thiserror::Error)]
#[error("the column {0}.{1} does not exist")]
struct UnknownColumn(&'static str, &'static str);

/// An identifier in double quotes, for a name from the list or from the catalog.
pub(crate) fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[async_trait]
impl ExportSource for Database {
    async fn organization_by_slug(
        &self,
        _caller: &ServiceCaller<Exporter>,
        slug: &OrganizationSlug,
    ) -> Result<Option<OrganizationId>, StoreError> {
        // An infrastructure query by slug (ADR 0039).
        let id = sqlx::query_scalar!("SELECT id FROM organization WHERE slug = $1", slug.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(store_error)?;
        Ok(id.map(OrganizationId::from_uuid))
    }

    async fn snapshot(&self, scope: OrgScope) -> Result<Snapshot, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        check_decisions(&mut tx).await?;
        let users = users_query(&mut tx).await?;
        let mut tables = Vec::with_capacity(EXPORTED.len());
        for exported in EXPORTED {
            tables.push(table_rows(&mut tx, exported, &users, organization).await?);
        }
        let blobs = sqlx::query!(
            "SELECT blob_key AS \"blob_key!\", sha256 FROM document_version
             WHERE organization_id = $1 AND blob_key IS NOT NULL
             ORDER BY id",
            organization,
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(store_error)?
        .into_iter()
        .map(|row| {
            Ok(StoredBlob {
                key: BlobKey::restore(row.blob_key),
                sha256: row
                    .sha256
                    .try_into()
                    .map_err(|_| InvalidRow("document_version.sha256"))?,
            })
        })
        .collect::<Result<_, StoreError>>()?;
        tx.commit().await.map_err(store_error)?;
        Ok(Snapshot { tables, blobs })
    }
}

/// Fails if a table of the schema is in neither list, or if the columns of an exported table
/// differ from its `columns` and `secret_columns`.
async fn check_decisions(conn: &mut PgConnection) -> Result<(), StoreError> {
    check_tables(conn).await?;
    for exported in EXPORTED {
        check_columns(conn, exported).await?;
    }
    Ok(())
}

/// Fails if a table of the schema is in neither list.
async fn check_tables(conn: &mut PgConnection) -> Result<(), StoreError> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'
         ORDER BY table_name",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let decided: BTreeSet<&str> = EXPORTED
        .iter()
        .map(|exported| exported.table)
        .chain(NOT_EXPORTED.iter().map(|(table, _)| *table))
        .collect();
    match tables
        .into_iter()
        .find(|table| !decided.contains(table.as_str()))
    {
        Some(table) => Err(StoreError::Internal(Box::new(UndecidedTable(table)))),
        None => Ok(()),
    }
}

/// Fails if a column of the table is in neither list, or if a listed column does not exist.
/// A generated column holds only a value that the table computes from its other columns; the
/// import computes it again, so it needs no decision.
async fn check_columns(conn: &mut PgConnection, exported: &Exported) -> Result<(), StoreError> {
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
         WHERE table_schema = current_schema() AND table_name = $1 AND is_generated = 'NEVER'
         ORDER BY ordinal_position",
    )
    .bind(exported.table)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let decided = || exported.columns.iter().chain(exported.secret_columns);
    if let Some(column) = columns
        .iter()
        .find(|column| !decided().any(|decided| decided == column))
    {
        return Err(StoreError::Internal(Box::new(UndecidedColumn(
            exported.table,
            column.clone(),
        ))));
    }
    match decided().find(|decided| !columns.iter().any(|column| column == *decided)) {
        Some(column) => Err(StoreError::Internal(Box::new(UnknownColumn(
            exported.table,
            column,
        )))),
        None => Ok(()),
    }
}

/// The query of the IDs of the users that the exported rows of the organization refer to:
/// each column of an exported organization table with a foreign key to `app_user`.
/// It reads the foreign keys from the catalog, so a new reference to a user needs no change here.
async fn users_query(conn: &mut PgConnection) -> Result<String, StoreError> {
    let references: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.conrelid::regclass::text, a.attname::text
         FROM pg_constraint c
         JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
         WHERE c.contype = 'f' AND c.confrelid = 'app_user'::regclass AND cardinality(c.conkey) = 1
         ORDER BY 1, 2",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let selects: Vec<String> = references
        .iter()
        .filter(|(table, _)| {
            exported_table(table).is_some_and(|exported| exported.rows == Rows::OfOrganization)
        })
        .map(|(table, column)| {
            format!(
                "SELECT {column} FROM {table} WHERE organization_id = $1",
                column = quoted(column),
                table = quoted(table),
            )
        })
        .collect();
    Ok(selects.join(" UNION "))
}

/// The exported columns, foreign keys and rows of one table.
async fn table_rows(
    conn: &mut PgConnection,
    exported: &Exported,
    users: &str,
    organization: Uuid,
) -> Result<TableRows, StoreError> {
    let foreign_keys: Vec<(Vec<String>, String, Vec<String>)> = sqlx::query_as(
        "SELECT
             array(SELECT a.attname::text FROM unnest(c.conkey) WITH ORDINALITY AS k(number, position)
                   JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.number
                   ORDER BY k.position),
             c.confrelid::regclass::text,
             array(SELECT a.attname::text FROM unnest(c.confkey) WITH ORDINALITY AS k(number, position)
                   JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.number
                   ORDER BY k.position)
         FROM pg_constraint c
         WHERE c.contype = 'f' AND c.conrelid = $1::regclass
         ORDER BY c.conname",
    )
    .bind(exported.table)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;

    let list = exported
        .columns
        .iter()
        .map(|column| quoted(column))
        .collect::<Vec<_>>()
        .join(", ");
    let filter = match exported.rows {
        Rows::Organization => "id = $1".to_owned(),
        Rows::OfOrganization => "organization_id = $1".to_owned(),
        Rows::OfUsers(column) => format!("{} IN ({users})", quoted(column)),
    };
    // The names come from the list, quoted; the organization is a bind parameter.
    let rows: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT json_build_array({list})::text FROM (SELECT {list} FROM {table} WHERE {filter}) AS r ORDER BY r",
        table = quoted(exported.table),
    )))
    .bind(organization)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let rows = rows
        .iter()
        .map(|row| match serde_json::from_str(row) {
            Ok(serde_json::Value::Array(values)) => Ok(values),
            _ => Err(InvalidRow("the JSON form of an exported row")),
        })
        .collect::<Result<_, _>>()?;

    Ok(TableRows {
        name: exported.table.to_owned(),
        columns: exported
            .columns
            .iter()
            .map(|&column| column.to_owned())
            .collect(),
        foreign_keys: foreign_keys
            .into_iter()
            .map(|(columns, table, references)| ForeignKey {
                columns,
                table,
                references,
            })
            .collect(),
        rows,
    })
}

#[cfg(test)]
mod tests {
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::ids::UserId;

    use super::*;
    use crate::testing::TestDatabase;

    fn scope(organization: OrganizationId) -> OrgScope {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        )
        .scope()
    }

    #[test]
    fn no_table_is_both_exported_and_left_out() {
        let mut names: Vec<&str> = EXPORTED.iter().map(|exported| exported.table).collect();
        names.extend(NOT_EXPORTED.iter().map(|(table, _)| *table));
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "{names:?}");
    }

    /// Each table of the migrations has an export decision. A new table fails the export until it has one.
    #[tokio::test]
    async fn the_export_refuses_a_table_without_an_export_decision() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let snapshot = test.database.snapshot(scope(organization)).await.unwrap();
        assert_eq!(snapshot.tables.len(), EXPORTED.len());

        sqlx::query(
            "CREATE TABLE note (organization_id uuid NOT NULL REFERENCES organization (id))",
        )
        .execute(&test.database.pool)
        .await
        .unwrap();
        let error = test
            .database
            .snapshot(scope(organization))
            .await
            .unwrap_err();
        assert!(
            format!("{error:?}").contains("UndecidedTable(\"note\")"),
            "{error:?}"
        );
    }

    /// Each column of an exported table is in `columns` or in `secret_columns`, and each listed
    /// column is in the schema. A new column fails the export until it has a decision, so a new
    /// secret column cannot go into the export files.
    #[tokio::test]
    async fn the_export_refuses_a_column_without_an_export_decision() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        let mut conn = test.database.pool.acquire().await.unwrap();
        check_decisions(&mut conn).await.unwrap();
        drop(conn);

        sqlx::query("ALTER TABLE organization ADD COLUMN webhook_secret text")
            .execute(&test.database.pool)
            .await
            .unwrap();
        let error = test
            .database
            .snapshot(scope(organization))
            .await
            .unwrap_err();
        assert!(
            format!("{error:?}").contains("UndecidedColumn(\"organization\", \"webhook_secret\")"),
            "{error:?}"
        );
    }

    /// A secret column that the table no longer has stops the export, so a rename cannot export the secret.
    #[tokio::test]
    async fn the_export_refuses_a_renamed_secret_column() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        sqlx::query("ALTER TABLE api_token RENAME COLUMN token_hash TO secret_hash")
            .execute(&test.database.pool)
            .await
            .unwrap();
        let error = test
            .database
            .snapshot(scope(organization))
            .await
            .unwrap_err();
        assert!(
            format!("{error:?}").contains("UndecidedColumn(\"api_token\", \"secret_hash\")"),
            "{error:?}"
        );
    }

    /// A listed column that the table no longer has stops the export, so the lists stay true.
    #[tokio::test]
    async fn the_export_refuses_a_listed_column_that_does_not_exist() {
        let test = TestDatabase::start().await;
        let organization = test.create_organization("testwil").await;
        sqlx::query("ALTER TABLE app_user DROP COLUMN locale")
            .execute(&test.database.pool)
            .await
            .unwrap();
        let error = test
            .database
            .snapshot(scope(organization))
            .await
            .unwrap_err();
        assert!(
            format!("{error:?}").contains("UnknownColumn(\"app_user\", \"locale\")"),
            "{error:?}"
        );
    }
}
