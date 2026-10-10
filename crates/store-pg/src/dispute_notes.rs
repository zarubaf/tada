//! The `DisputeNoteStore` adapter: dispute notes about immutable evidence (ADR 0073).
//!
//! The queries are checked at run time.

use async_trait::async_trait;
use jiff_sqlx::{Timestamp as SqlxTimestamp, ToSqlx};
use sqlx::postgres::PgRow;
use sqlx::types::Uuid;
use sqlx::{PgConnection, Row};
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::dispute_notes::{
    DisputeNote, DisputeNoteReads, DisputeNoteStore, NoteCreated, NoteKey,
};
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{DisputeNoteId, FactId, SourceVersionId, UserId};
use tada_app::domain::redaction::{DisputeNoteText, DisputeTarget, DisputeTargetKind};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit::record;
use crate::error::{InvalidRow, store_error};

/// The state of the target row: true if it is redacted. `lock` takes `FOR SHARE`, which waits for a redaction
/// of the row and makes a later redaction wait for the note.
fn target_sql(kind: DisputeTargetKind, lock: bool) -> &'static str {
    match (kind, lock) {
        (DisputeTargetKind::SourceItem, false) => {
            "SELECT redacted_at IS NOT NULL FROM source_item WHERE organization_id = $1 AND id = $2"
        }
        (DisputeTargetKind::SourceItem, true) => {
            "SELECT redacted_at IS NOT NULL FROM source_item WHERE organization_id = $1 AND id = $2 FOR SHARE"
        }
        (DisputeTargetKind::FactVersion, false) => {
            "SELECT redacted_at IS NOT NULL FROM fact_version WHERE organization_id = $1 AND id = $2"
        }
        (DisputeTargetKind::FactVersion, true) => {
            "SELECT redacted_at IS NOT NULL FROM fact_version WHERE organization_id = $1 AND id = $2 FOR SHARE"
        }
        (DisputeTargetKind::DocumentVersion, false) => {
            "SELECT redacted_at IS NOT NULL FROM document_version WHERE organization_id = $1 AND id = $2"
        }
        (DisputeTargetKind::DocumentVersion, true) => {
            "SELECT redacted_at IS NOT NULL FROM document_version WHERE organization_id = $1 AND id = $2
             FOR SHARE"
        }
        (DisputeTargetKind::Proposal, false) => {
            "SELECT redacted_at IS NOT NULL FROM proposal WHERE organization_id = $1 AND id = $2"
        }
        (DisputeTargetKind::Proposal, true) => {
            "SELECT redacted_at IS NOT NULL FROM proposal WHERE organization_id = $1 AND id = $2 FOR SHARE"
        }
    }
}

async fn target_state(
    conn: &mut PgConnection,
    scope: OrgScope,
    target: DisputeTarget,
    lock: bool,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar(target_sql(target.kind, lock))
        .bind(scope.organization_id().as_uuid())
        .bind(target.id)
        .fetch_optional(conn)
        .await
}

/// The note of a row with the columns `id`, `target_kind`, `target_id`, `text` and `created_at`.
fn note(row: &PgRow) -> Result<DisputeNote, StoreError> {
    let kind: String = row.try_get("target_kind").map_err(store_error)?;
    let text: String = row.try_get("text").map_err(store_error)?;
    let created_at: SqlxTimestamp = row.try_get("created_at").map_err(store_error)?;
    Ok(DisputeNote {
        id: DisputeNoteId::from_uuid(row.try_get("id").map_err(store_error)?),
        target: DisputeTarget {
            kind: kind
                .parse()
                .map_err(|_| InvalidRow("dispute_note.target_kind"))?,
            id: row.try_get("target_id").map_err(store_error)?,
        },
        text: DisputeNoteText::parse(&text).map_err(|_| InvalidRow("dispute_note.text"))?,
        created_at: created_at.to_jiff(),
    })
}

/// The notes about the targets that a redaction did not remove, oldest first.
pub(crate) async fn notes_of_targets(
    conn: &mut PgConnection,
    scope: OrgScope,
    targets: &[DisputeTarget],
) -> Result<Vec<DisputeNote>, StoreError> {
    let kinds: Vec<&str> = targets.iter().map(|target| target.kind.as_str()).collect();
    let ids: Vec<Uuid> = targets.iter().map(|target| target.id).collect();
    let rows = sqlx::query(
        "SELECT n.id, n.target_kind, n.target_id, n.text, n.created_at
         FROM dispute_note n
         JOIN (SELECT DISTINCT kind, id FROM unnest($2::text[], $3::uuid[]) AS t(kind, id)) AS t
             ON n.target_kind = t.kind AND n.target_id = t.id
         WHERE n.organization_id = $1 AND n.redacted_at IS NULL
         ORDER BY n.created_at, n.id",
    )
    .bind(scope.organization_id().as_uuid())
    .bind(&kinds)
    .bind(&ids)
    .fetch_all(conn)
    .await
    .map_err(store_error)?;
    rows.iter().map(note).collect()
}

#[async_trait]
impl DisputeNoteStore for Database {
    async fn target_exists(
        &self,
        scope: OrgScope,
        target: DisputeTarget,
    ) -> Result<bool, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        Ok(target_state(&mut conn, scope, target, false)
            .await
            .map_err(store_error)?
            .is_some())
    }

    async fn create(
        &self,
        scope: OrgScope,
        note: &DisputeNote,
        author: UserId,
        audit: &AuditEvent,
    ) -> Result<NoteCreated, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match target_state(&mut tx, scope, note.target, true)
            .await
            .map_err(store_error)?
        {
            None => return Ok(NoteCreated::NotFound),
            Some(true) => return Ok(NoteCreated::Redacted),
            Some(false) => {}
        }
        sqlx::query(
            "INSERT INTO dispute_note (id, organization_id, target_kind, target_id, text, author_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(note.id.as_uuid())
        .bind(scope.organization_id().as_uuid())
        .bind(note.target.kind.as_str())
        .bind(note.target.id)
        .bind(note.text.as_str())
        .bind(author.as_uuid())
        .bind(note.created_at.to_sqlx())
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(NoteCreated::Created(note.clone()))
    }
}

#[async_trait]
impl DisputeNoteReads for Database {
    async fn notes(
        &self,
        scope: OrgScope,
        keys: &[NoteKey],
    ) -> Result<Vec<(NoteKey, DisputeNote)>, StoreError> {
        let mut target_kinds = Vec::new();
        let mut target_ids = Vec::new();
        let mut source_versions = Vec::new();
        let mut facts = Vec::new();
        let mut numbers = Vec::new();
        for key in keys {
            match key {
                NoteKey::Target(target) => {
                    target_kinds.push(target.kind.as_str());
                    target_ids.push(target.id);
                }
                NoteKey::SourceVersion(id) => source_versions.push(id.as_uuid()),
                NoteKey::FactVersion { fact, number } => {
                    facts.push(fact.as_uuid());
                    numbers.push(number.get());
                }
            }
        }
        // Each branch names its key: the target itself, a source version of a source item, or a fact version by
        // the number of its fact.
        let rows = sqlx::query(
            "SELECT 'target' AS key, n.target_id AS key_id, NULL::bigint AS key_number,
                    n.id, n.target_kind, n.target_id, n.text, n.created_at
             FROM dispute_note n
             JOIN (SELECT DISTINCT kind, id FROM unnest($2::text[], $3::uuid[]) AS t(kind, id)) AS t
                 ON n.target_kind = t.kind AND n.target_id = t.id
             WHERE n.organization_id = $1 AND n.redacted_at IS NULL
             UNION ALL
             SELECT 'source-version', v.id, NULL, n.id, n.target_kind, n.target_id, n.text, n.created_at
             FROM dispute_note n
             JOIN source_version v ON v.organization_id = n.organization_id AND v.source_item_id = n.target_id
             WHERE n.organization_id = $1 AND n.target_kind = 'source-item' AND n.redacted_at IS NULL
               AND v.id = ANY($4)
             UNION ALL
             SELECT 'fact-version', v.fact_id, v.number, n.id, n.target_kind, n.target_id, n.text, n.created_at
             FROM dispute_note n
             JOIN fact_version v ON v.organization_id = n.organization_id AND v.id = n.target_id
             JOIN (SELECT DISTINCT fact, number FROM unnest($5::uuid[], $6::bigint[]) AS f(fact, number)) AS f
                 ON v.fact_id = f.fact AND v.number = f.number
             WHERE n.organization_id = $1 AND n.target_kind = 'fact-version' AND n.redacted_at IS NULL
             ORDER BY created_at, id",
        )
        .bind(scope.organization_id().as_uuid())
        .bind(&target_kinds)
        .bind(&target_ids)
        .bind(&source_versions)
        .bind(&facts)
        .bind(&numbers)
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.iter()
            .map(|row| {
                let note = note(row)?;
                let key: String = row.try_get("key").map_err(store_error)?;
                let key_id: Uuid = row.try_get("key_id").map_err(store_error)?;
                let key = match key.as_str() {
                    "target" => NoteKey::Target(note.target),
                    "source-version" => NoteKey::SourceVersion(SourceVersionId::from_uuid(key_id)),
                    _ => {
                        let number: i64 = row.try_get("key_number").map_err(store_error)?;
                        NoteKey::FactVersion {
                            fact: FactId::from_uuid(key_id),
                            number: RecordVersion::new(number)
                                .ok_or(InvalidRow("fact_version.number"))?,
                        }
                    }
                };
                Ok((key, note))
            })
            .collect()
    }
}
