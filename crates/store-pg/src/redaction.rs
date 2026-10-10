//! Legal redaction (ADR 0072): the tombstones of the targets and of the content that copies them.
//!
//! This is the only code that sets `tada.redaction`. The setting is transaction-local (`SET LOCAL`), and the
//! immutability triggers accept one UPDATE shape with it: an unredacted row gets `redacted_at`, its content
//! columns become NULL, and nothing else changes. The mutable tables have CHECK constraints that keep a
//! redacted row without content.
//!
//! What one target covers:
//!
//! - source item: all its versions, and the quotes that copy them;
//! - document version: its content; an upload version also its source version, which holds the same file;
//! - a source version of an upload: also its document version and the object of the file;
//! - fact version, proposal: the row and the quotes that the row owns;
//! - person, institution: the row, the quotes that the row owns, and the proposals that created it;
//! - event: all content of the event (ADR 0072); its name, labels, statuses, dates, numbers and IDs stay;
//! - each redacted source item, fact version, document version and proposal: the dispute notes about it.
//!
//! An open proposal that a redaction empties is rejected in the same transaction, so it does not stay open.
//!
//! The queries are checked at run time: a redaction runs one statement for each table, with the IDs as arrays.

use async_trait::async_trait;
use jiff_sqlx::{Timestamp as SqlxTimestamp, ToSqlx};
use serde_json::json;
use sqlx::types::Uuid;
use sqlx::{PgConnection, Row};
use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::blobs::BlobKey;
use tada_app::caller::{Actor, ActorKind, OrgScope};
use tada_app::domain::ids::RedactionId;
use tada_app::domain::redaction::{RedactionReason, RedactionTarget, RedactionTargetKind};
use tada_app::jobs::NewJob;
use tada_app::redaction::{
    JOURNAL_JOB, Redaction, RedactionOutcome, RedactionStore, Refused, Replayed,
};
use tada_app::store::StoreError;

use crate::Database;
use crate::actor;
use crate::audit::record;
use crate::error::{InvalidRow, store_error};
use crate::jobs::enqueue;

/// The IDs of the rows that one target covers, for each table.
#[derive(Debug, Default)]
struct Rows {
    source_items: Vec<Uuid>,
    source_versions: Vec<Uuid>,
    fact_versions: Vec<Uuid>,
    proposals: Vec<Uuid>,
    documents: Vec<Uuid>,
    document_versions: Vec<Uuid>,
    persons: Vec<Uuid>,
    institutions: Vec<Uuid>,
    open_questions: Vec<Uuid>,
    workstreams: Vec<Uuid>,
    actions: Vec<Uuid>,
    commitments: Vec<Uuid>,
    events: Vec<Uuid>,
}

/// Locks the target row and tells if it is redacted. `None` if the row does not exist in the organization.
/// `FOR UPDATE` conflicts with the `FOR SHARE` of each writer that copies or cites the row (`share_sources`,
/// `share_proposals`), so no writer keeps a copy that it read before the redaction.
fn state_sql(kind: RedactionTargetKind) -> &'static str {
    match kind {
        RedactionTargetKind::SourceItem => {
            "SELECT redacted_at IS NOT NULL FROM source_item WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
        RedactionTargetKind::FactVersion => {
            "SELECT redacted_at IS NOT NULL FROM fact_version WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
        RedactionTargetKind::DocumentVersion => {
            "SELECT redacted_at IS NOT NULL FROM document_version WHERE organization_id = $1 AND id = $2
             FOR UPDATE"
        }
        RedactionTargetKind::Proposal => {
            "SELECT redacted_at IS NOT NULL FROM proposal WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
        RedactionTargetKind::Person => {
            "SELECT redacted_at IS NOT NULL FROM person WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
        RedactionTargetKind::Institution => {
            "SELECT redacted_at IS NOT NULL FROM institution WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
        RedactionTargetKind::Event => {
            "SELECT redacted_at IS NOT NULL FROM event WHERE organization_id = $1 AND id = $2 FOR UPDATE"
        }
    }
}

async fn state(
    conn: &mut PgConnection,
    organization: Uuid,
    target: RedactionTarget,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar(state_sql(target.kind))
        .bind(organization)
        .bind(target.id)
        .fetch_optional(conn)
        .await
}

/// The IDs that `sql` returns for the organization and the IDs `ids`.
async fn ids(
    conn: &mut PgConnection,
    sql: &'static str,
    organization: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(sql)
        .bind(organization)
        .bind(ids)
        .fetch_all(conn)
        .await
}

/// Locks the source versions `ids` `FOR SHARE` for a writer that copies or cites them, and returns the redacted ones.
/// Each writer calls it in its transaction before its insert: a redaction that runs waits for the writer, and a
/// writer that comes later waits for the redaction and then sees the tombstone (ADR 0072).
pub(crate) async fn share_sources(
    conn: &mut PgConnection,
    organization: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, redacted_at IS NOT NULL FROM source_version
         WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR SHARE",
    )
    .bind(organization)
    .bind(ids)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, redacted)| redacted.then_some(id))
        .collect())
}

/// Locks the source item `item` `FOR SHARE` for a writer that adds a version to it, and tells if it is redacted.
pub(crate) async fn share_source_item(
    conn: &mut PgConnection,
    organization: Uuid,
    item: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT redacted_at IS NOT NULL FROM source_item WHERE organization_id = $1 AND id = $2 FOR SHARE",
    )
    .bind(organization)
    .bind(item)
    .fetch_one(conn)
    .await
}

/// Locks the proposals `ids` `FOR SHARE` for an apply that copies their evidence, and tells if one is redacted.
pub(crate) async fn share_proposals(
    conn: &mut PgConnection,
    organization: Uuid,
    ids: &[Uuid],
) -> Result<bool, sqlx::Error> {
    let redacted: Vec<bool> = sqlx::query_scalar(
        "SELECT redacted_at IS NOT NULL FROM proposal
         WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR SHARE",
    )
    .bind(organization)
    .bind(ids)
    .fetch_all(conn)
    .await?;
    Ok(redacted.into_iter().any(|redacted| redacted))
}

/// Adds the IDs that are not in `list` yet.
fn extend(list: &mut Vec<Uuid>, more: Vec<Uuid>) {
    for id in more {
        if !list.contains(&id) {
            list.push(id);
        }
    }
}

/// The proposals that created a person: they copy its name, email address and phone number.
const PARTY_PROPOSALS_PERSON: &str = "SELECT id FROM proposal
     WHERE organization_id = $1 AND target_kind = 'person' AND target_id = ANY($2) ORDER BY id FOR UPDATE";

/// The proposals that created an institution, as for a person.
const PARTY_PROPOSALS_INSTITUTION: &str = "SELECT id FROM proposal
     WHERE organization_id = $1 AND target_kind = 'institution' AND target_id = ANY($2) ORDER BY id FOR UPDATE";

/// The rows of the target, before the links between uploads and their source versions.
async fn rows_of(
    conn: &mut PgConnection,
    organization: Uuid,
    target: RedactionTarget,
) -> Result<Rows, sqlx::Error> {
    let id = vec![target.id];
    let mut rows = Rows::default();
    match target.kind {
        RedactionTargetKind::SourceItem => rows.source_items = id,
        RedactionTargetKind::FactVersion => rows.fact_versions = id,
        RedactionTargetKind::DocumentVersion => rows.document_versions = id,
        RedactionTargetKind::Proposal => rows.proposals = id,
        RedactionTargetKind::Person => {
            rows.proposals = ids(conn, PARTY_PROPOSALS_PERSON, organization, &id).await?;
            rows.persons = id;
        }
        RedactionTargetKind::Institution => {
            rows.proposals = ids(conn, PARTY_PROPOSALS_INSTITUTION, organization, &id).await?;
            rows.institutions = id;
        }
        RedactionTargetKind::Event => {
            rows.source_items = ids(
                conn,
                "SELECT id FROM source_item WHERE organization_id = $1 AND event_id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                &id,
            )
            .await?;
            rows.fact_versions = ids(
                conn,
                "SELECT v.id FROM fact_version v
                 JOIN fact f ON f.organization_id = v.organization_id AND f.id = v.fact_id
                 WHERE f.organization_id = $1 AND f.event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.proposals = ids(
                conn,
                "SELECT id FROM proposal WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.documents = ids(
                conn,
                "SELECT id FROM document WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.open_questions = ids(
                conn,
                "SELECT id FROM open_question WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.workstreams = ids(
                conn,
                "SELECT id FROM workstream WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.actions = ids(
                conn,
                "SELECT id FROM action WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.commitments = ids(
                conn,
                "SELECT id FROM commitment WHERE organization_id = $1 AND event_id = ANY($2)",
                organization,
                &id,
            )
            .await?;
            rows.document_versions = ids(
                conn,
                "SELECT id FROM document_version WHERE organization_id = $1 AND document_id = ANY($2)",
                organization,
                &rows.documents,
            )
            .await?;
            rows.events = id;
        }
    }
    rows.source_versions = ids(
        conn,
        "SELECT id FROM source_version WHERE organization_id = $1 AND source_item_id = ANY($2) ORDER BY id FOR UPDATE",
        organization,
        &rows.source_items,
    )
    .await?;
    // An upload version and its source version hold the same file: one goes with the other.
    let uploads = ids(
        conn,
        "SELECT id FROM document_version WHERE organization_id = $1 AND source_version_id = ANY($2)",
        organization,
        &rows.source_versions,
    )
    .await?;
    extend(&mut rows.document_versions, uploads);
    let sources = ids(
        conn,
        "SELECT source_version_id FROM document_version
         WHERE organization_id = $1 AND id = ANY($2) AND source_version_id IS NOT NULL",
        organization,
        &rows.document_versions,
    )
    .await?;
    extend(&mut rows.source_versions, sources);
    // The source versions of the uploads are cited roots too: lock them before the quotes are read.
    ids(
        conn,
        "SELECT id FROM source_version WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        organization,
        &rows.source_versions,
    )
    .await?;
    Ok(rows)
}

/// Runs one tombstone statement with the organization and the arrays of IDs.
async fn update(
    conn: &mut PgConnection,
    sql: &'static str,
    organization: Uuid,
    arrays: &[&[Uuid]],
) -> Result<(), sqlx::Error> {
    let mut query = sqlx::query(sql).bind(organization);
    for array in arrays {
        query = query.bind(*array);
    }
    query.execute(conn).await.map(|_| ())
}

/// Writes the tombstones of `rows`, rejects the emptied open proposals, and returns the objects of the
/// redacted upload versions.
async fn tombstone(
    conn: &mut PgConnection,
    scope: OrgScope,
    rows: &Rows,
    actor: &Actor,
) -> Result<Vec<BlobKey>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    update(
        conn,
        "UPDATE source_item SET redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.source_items],
    )
    .await?;
    update(
        conn,
        "UPDATE source_version
         SET text = NULL, url = NULL, sha256 = NULL,
             author_actor = jsonb_build_object('kind', author_actor -> 'kind', 'id', author_actor -> 'id'),
             redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.source_versions],
    )
    .await?;
    update(
        conn,
        "UPDATE fact_version SET value = NULL, redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.fact_versions],
    )
    .await?;
    let emptied: Vec<(Uuid, bool)> = sqlx::query_as(
        "UPDATE proposal p
         SET operation = jsonb_build_object('kind', operation -> 'kind'), reason = NULL, manifest = NULL,
             lint_warnings = NULL, redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL
         RETURNING id, NOT EXISTS (
             SELECT 1 FROM review_result r WHERE r.organization_id = p.organization_id AND r.proposal_id = p.id
         )",
    )
    .bind(organization)
    .bind(&rows.proposals)
    .fetch_all(&mut *conn)
    .await?;
    for (proposal, open) in emptied {
        if open {
            reject(conn, scope, proposal, actor).await?;
        }
    }
    update(
        conn,
        "UPDATE evidence_link SET quote = NULL, redacted_at = now()
         WHERE organization_id = $1 AND redacted_at IS NULL
           AND (source_version_id = ANY($2) OR fact_version_id = ANY($3))",
        organization,
        &[&rows.source_versions, &rows.fact_versions],
    )
    .await?;
    update(
        conn,
        "UPDATE proposal_evidence SET quote = NULL, redacted_at = now()
         WHERE organization_id = $1 AND redacted_at IS NULL
           AND (source_version_id = ANY($2) OR proposal_id = ANY($3))",
        organization,
        &[&rows.source_versions, &rows.proposals],
    )
    .await?;
    update(
        conn,
        "UPDATE record_evidence SET quote = NULL, redacted_at = now()
         WHERE organization_id = $1 AND redacted_at IS NULL
           AND (source_version_id = ANY($2) OR action_id = ANY($3) OR commitment_id = ANY($4)
                OR person_id = ANY($5) OR institution_id = ANY($6))",
        organization,
        &[
            &rows.source_versions,
            &rows.actions,
            &rows.commitments,
            &rows.persons,
            &rows.institutions,
        ],
    )
    .await?;
    let blob_keys: Vec<Option<String>> = sqlx::query_scalar(
        "UPDATE document_version SET markdown = NULL, file_name = NULL, sha256 = NULL, redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL
         RETURNING blob_key",
    )
    .bind(organization)
    .bind(&rows.document_versions)
    .fetch_all(&mut *conn)
    .await?;
    update(
        conn,
        "UPDATE document SET name = NULL, version = version + 1, redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.documents],
    )
    .await?;
    update(
        conn,
        "UPDATE person
         SET name = NULL, email = NULL, phone = NULL, user_id = NULL, version = version + 1, updated_at = now(),
             redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.persons],
    )
    .await?;
    update(
        conn,
        "UPDATE institution
         SET name = NULL, email = NULL, phone = NULL, version = version + 1, updated_at = now(), redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.institutions],
    )
    .await?;
    update(
        conn,
        "UPDATE open_question SET text = NULL, version = version + 1, redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.open_questions],
    )
    .await?;
    update(
        conn,
        "UPDATE workstream SET name = NULL, version = version + 1, updated_at = now(), redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.workstreams],
    )
    .await?;
    update(
        conn,
        "UPDATE action
         SET title = NULL, description = NULL, version = version + 1, updated_at = now(), redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.actions],
    )
    .await?;
    update(
        conn,
        "UPDATE commitment
         SET text = NULL, condition = NULL, firm_reason = NULL, version = version + 1, updated_at = now(),
             redacted_at = now()
         WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.commitments],
    )
    .await?;
    update(
        conn,
        "UPDATE dispute_note SET text = NULL, redacted_at = now()
         WHERE organization_id = $1 AND redacted_at IS NULL
           AND ((target_kind = 'source-item' AND target_id = ANY($2))
                OR (target_kind = 'fact-version' AND target_id = ANY($3))
                OR (target_kind = 'document-version' AND target_id = ANY($4))
                OR (target_kind = 'proposal' AND target_id = ANY($5)))",
        organization,
        &[
            &rows.source_items,
            &rows.fact_versions,
            &rows.document_versions,
            &rows.proposals,
        ],
    )
    .await?;
    update(
        conn,
        "UPDATE event SET redacted_at = now() WHERE organization_id = $1 AND id = ANY($2) AND redacted_at IS NULL",
        organization,
        &[&rows.events],
    )
    .await?;
    Ok(blob_keys
        .into_iter()
        .flatten()
        .map(BlobKey::restore)
        .collect())
}

/// Rejects an open proposal that a redaction emptied, with the actor of the redaction as reviewer.
/// The review result has no free text; the audit event of the redaction tells why.
async fn reject(
    conn: &mut PgConnection,
    scope: OrgScope,
    proposal: Uuid,
    actor: &Actor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
         VALUES ($1, $2, $3, 'rejected', $4, now())",
    )
    .bind(Uuid::now_v7())
    .bind(scope.organization_id().as_uuid())
    .bind(proposal)
    .bind(actor::to_json(actor))
    .execute(&mut *conn)
    .await?;
    record(
        conn,
        &AuditEvent::new(
            *actor,
            AuditAction::ProposalReject,
            Some(proposal),
            Some(scope),
        ),
    )
    .await
}

/// Opens a transaction with the redaction setting. `SET LOCAL` ends with the transaction.
async fn begin(
    database: &Database,
) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
    let mut tx = database.pool.begin().await?;
    sqlx::query("SET LOCAL tada.redaction = 'on'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// Writes the tombstones of one unredacted target, its `redaction` row and its audit event.
async fn apply(
    conn: &mut PgConnection,
    scope: OrgScope,
    redaction: &Redaction,
    actor_id: Option<Uuid>,
    actor: &Actor,
) -> Result<Vec<BlobKey>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let rows = rows_of(conn, organization, redaction.target).await?;
    let blob_keys = tombstone(conn, scope, &rows, actor).await?;
    sqlx::query(
        "INSERT INTO redaction (id, organization_id, target_kind, target_id, reason, actor_id, redacted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(redaction.id.as_uuid())
    .bind(organization)
    .bind(redaction.target.kind.as_str())
    .bind(redaction.target.id)
    .bind(redaction.reason.as_str())
    .bind(actor_id)
    .bind(redaction.redacted_at.to_sqlx())
    .execute(&mut *conn)
    .await?;
    record(
        conn,
        &AuditEvent::new(
            *actor,
            AuditAction::redaction_of(redaction.target.kind),
            Some(redaction.target.id),
            Some(scope),
        )
        .with_reason(redaction.reason),
    )
    .await?;
    Ok(blob_keys)
}

/// True if the event is closed. Only a closed event can be redacted (ADR 0071).
async fn event_is_closed(
    conn: &mut PgConnection,
    organization: Uuid,
    event: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT status = 'closed' FROM event WHERE organization_id = $1 AND id = $2")
        .bind(organization)
        .bind(event)
        .fetch_one(conn)
        .await
}

/// Closes the event of a replayed event redaction if it is open, with the audit event `event.close`.
/// The close time is the time of the redaction, unless the event has one.
async fn close_for_replay(
    conn: &mut PgConnection,
    scope: OrgScope,
    redaction: &Redaction,
    actor: &Actor,
) -> Result<(), sqlx::Error> {
    let closed: Option<Uuid> = sqlx::query_scalar(
        "UPDATE event SET status = 'closed', closed_at = coalesce(closed_at, $3), version = version + 1
         WHERE organization_id = $1 AND id = $2 AND status <> 'closed'
         RETURNING id",
    )
    .bind(scope.organization_id().as_uuid())
    .bind(redaction.target.id)
    .bind(redaction.redacted_at.to_sqlx())
    .fetch_optional(&mut *conn)
    .await?;
    match closed {
        Some(event) => {
            record(
                conn,
                &AuditEvent::new(*actor, AuditAction::EventClose, Some(event), Some(scope)),
            )
            .await
        }
        None => Ok(()),
    }
}

/// The time of the transaction: the time of each tombstone of the redaction.
async fn transaction_time(conn: &mut PgConnection) -> Result<jiff::Timestamp, sqlx::Error> {
    let now: SqlxTimestamp = sqlx::query_scalar("SELECT now()").fetch_one(conn).await?;
    Ok(now.to_jiff())
}

#[async_trait]
impl RedactionStore for Database {
    async fn redact(
        &self,
        scope: OrgScope,
        targets: &[RedactionTarget],
        reason: RedactionReason,
        actor: &Actor,
    ) -> Result<Result<RedactionOutcome, Refused>, StoreError> {
        let organization = scope.organization_id();
        let mut tx = begin(self).await.map_err(store_error)?;
        let redacted_at = transaction_time(&mut tx).await.map_err(store_error)?;
        // The actor of the row is a member, or no one for a service. An AI client cannot redact.
        let actor_id = (actor.kind() == ActorKind::Member).then(|| actor.id());
        let mut outcome = RedactionOutcome {
            applied: Vec::new(),
            already: 0,
            blob_keys: Vec::new(),
        };
        for &target in targets {
            match state(&mut tx, organization.as_uuid(), target)
                .await
                .map_err(store_error)?
            {
                // The transaction rolls back when it drops.
                None => return Ok(Err(Refused::NotFound)),
                Some(true) => outcome.already += 1,
                Some(false)
                    if target.kind == RedactionTargetKind::Event
                        && !event_is_closed(&mut tx, organization.as_uuid(), target.id)
                            .await
                            .map_err(store_error)? =>
                {
                    return Ok(Err(Refused::EventNotClosed));
                }
                Some(false) => {
                    let redaction = Redaction {
                        id: RedactionId::from_uuid(Uuid::now_v7()),
                        organization_id: organization,
                        target,
                        reason,
                        redacted_at,
                    };
                    let keys = apply(&mut tx, scope, &redaction, actor_id, actor)
                        .await
                        .map_err(store_error)?;
                    enqueue(
                        &mut tx,
                        &NewJob {
                            kind: JOURNAL_JOB,
                            version: 1,
                            payload: json!({"redaction_id": redaction.id.as_uuid()}),
                            organization_id: Some(organization),
                            request_id: actor.request_id(),
                            run_at: None,
                        },
                    )
                    .await
                    .map_err(store_error)?;
                    outcome.applied.push(redaction.id);
                    outcome.blob_keys.extend(keys);
                }
            }
        }
        tx.commit().await.map_err(store_error)?;
        Ok(Ok(outcome))
    }

    async fn is_redacted(
        &self,
        scope: OrgScope,
        target: RedactionTarget,
    ) -> Result<bool, StoreError> {
        // The lock of `state` ends with the transaction of this one statement.
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let redacted = state(&mut conn, scope.organization_id().as_uuid(), target)
            .await
            .map_err(store_error)?;
        Ok(redacted.unwrap_or(false))
    }

    async fn redaction(
        &self,
        scope: OrgScope,
        id: RedactionId,
    ) -> Result<Option<Redaction>, StoreError> {
        let row = sqlx::query(
            "SELECT target_kind, target_id, reason, redacted_at FROM redaction
             WHERE organization_id = $1 AND id = $2",
        )
        .bind(scope.organization_id().as_uuid())
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let kind: String = row.try_get("target_kind").map_err(store_error)?;
        let reason: String = row.try_get("reason").map_err(store_error)?;
        let redacted_at: SqlxTimestamp = row.try_get("redacted_at").map_err(store_error)?;
        Ok(Some(Redaction {
            id,
            organization_id: scope.organization_id(),
            target: RedactionTarget {
                kind: kind.parse().map_err(|_| InvalidRow("target_kind"))?,
                id: row.try_get("target_id").map_err(store_error)?,
            },
            reason: reason.parse().map_err(|_| InvalidRow("reason"))?,
            redacted_at: redacted_at.to_jiff(),
        }))
    }

    async fn replay(
        &self,
        scope: OrgScope,
        redaction: &Redaction,
        actor: &Actor,
    ) -> Result<Replayed, StoreError> {
        let mut tx = begin(self).await.map_err(store_error)?;
        let replayed = match state(&mut tx, scope.organization_id().as_uuid(), redaction.target)
            .await
            .map_err(store_error)?
        {
            None => Replayed::Missing,
            Some(true) => Replayed::Already,
            Some(false) => {
                // A retention redaction needs a closed event. A backup from before the close holds the event
                // open, so the replay closes it first, at the time of the redaction if it has no close time.
                if redaction.target.kind == RedactionTargetKind::Event {
                    close_for_replay(&mut tx, scope, redaction, actor)
                        .await
                        .map_err(store_error)?;
                }
                // The journal does not name the member who redacted, so the row names no one.
                Replayed::Applied(
                    apply(&mut tx, scope, redaction, None, actor)
                        .await
                        .map_err(store_error)?,
                )
            }
        };
        tx.commit().await.map_err(store_error)?;
        Ok(replayed)
    }
}
