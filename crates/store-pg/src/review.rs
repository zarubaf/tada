//! The `ReviewStore` adapter (ADR 0050): review results, the apply of a changeset and the Review Inbox.
//!
//! Each write locks its changeset first, so two reviews of one changeset run one after the other.
//! Review results are append-only: the store never updates or deletes them.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::{Actor, OrgScope};
use tada_app::domain::RecordVersion;
use tada_app::domain::documents::DraftMarkdown;
use tada_app::domain::events::Event;
use tada_app::domain::facts::{ChoiceValue, FactState, FieldStatus, Label, ValueType, Valued};
use tada_app::domain::ids::{
    ChangesetId, DocumentId, EventId, FieldDefinitionId, OpenQuestionId, ProposalId,
    SourceVersionId,
};
use tada_app::domain::proposals::{DraftDocument, Operation};
use tada_app::domain::sources::{Passage, SourceText};
use tada_app::review::{
    ApplyOutcome, ApplyPlan, ApplyStep, LocalRecord, NewLocalId, OpenChangeset, Recorded,
    ReviewBatch, ReviewOutcome, ReviewRecord, ReviewStore, StepEvidence,
};
use tada_app::store::StoreError;

use crate::Database;
use crate::documents::{DOCUMENT_COUNTER, DRAFT};
use crate::error::{InvalidRow, store_error};
use crate::sources::{TextItem, TextKind};
use crate::{actor, audit, drafts, events, sources, values};

/// The kind of the event-local IDs of open questions (ADR 0038).
const OPEN_QUESTION_PREFIX: &str = "QST";

/// The constraints that a concurrent apply of another record with the same key breaks.
const KEY_CONSTRAINTS: &[&str] = &["event_key_unique", "field_definition_event_key_unique"];

/// The constraints that a concurrent apply of the same record breaks, after the version checks.
const RECORD_CONSTRAINTS: &[&str] = &[
    "event_pkey",
    "field_definition_pkey",
    "open_question_pkey",
    "fact_event_id_field_id_key",
    "document_pkey",
];

#[async_trait]
impl ReviewStore for Database {
    async fn results(
        &self,
        scope: OrgScope,
        changeset: ChangesetId,
    ) -> Result<Vec<ReviewRecord>, StoreError> {
        let rows = sqlx::query!(
            r#"SELECT r.proposal_id, r.result, r.created_at AS "created_at: jiff_sqlx::Timestamp"
               FROM review_result r
               JOIN proposal p ON p.organization_id = r.organization_id AND p.id = r.proposal_id
               WHERE r.organization_id = $1 AND p.changeset_id = $2
               ORDER BY r.created_at, r.id"#,
            scope.organization_id().as_uuid(),
            changeset.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(ReviewRecord {
                    proposal_id: ProposalId::from_uuid(row.proposal_id),
                    outcome: ReviewOutcome::parse(&row.result)
                        .ok_or(InvalidRow("review_result.result"))?,
                    created_at: row.created_at.to_jiff(),
                })
            })
            .collect()
    }

    async fn apply(&self, scope: OrgScope, plan: &ApplyPlan) -> Result<ApplyOutcome, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let proposals: Vec<ProposalId> = plan.steps.iter().map(|step| step.proposal_id).collect();
        if !lock_open(&mut tx, scope, plan.changeset_id, &proposals).await? {
            return Ok(ApplyOutcome::NotOpen);
        }
        lock_targets(&mut tx, scope, &plan.steps)
            .await
            .map_err(store_error)?;
        let conflicts = check_versions(&mut tx, scope, &plan.steps)
            .await
            .map_err(store_error)?;
        if !conflicts.is_empty() {
            return Ok(ApplyOutcome::Conflict(conflicts));
        }
        let mut local_ids = Vec::new();
        for step in &plan.steps {
            match write_step(&mut tx, scope, plan, step).await {
                Ok(local_id) => local_ids.extend(local_id),
                Err(sqlx::Error::Database(error))
                    if error
                        .constraint()
                        .is_some_and(|name| KEY_CONSTRAINTS.contains(&name)) =>
                {
                    return Ok(ApplyOutcome::KeyTaken);
                }
                // A concurrent apply created the record after the check.
                Err(sqlx::Error::Database(error))
                    if error
                        .constraint()
                        .is_some_and(|name| RECORD_CONSTRAINTS.contains(&name)) =>
                {
                    return Ok(ApplyOutcome::Conflict(vec![step.proposal_id]));
                }
                // The target of the step changed after the check.
                Err(sqlx::Error::RowNotFound) => {
                    return Ok(ApplyOutcome::Conflict(vec![step.proposal_id]));
                }
                Err(error) => return Err(store_error(error)),
            }
        }
        for entry in &plan.audit {
            audit::record(&mut tx, entry).await.map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(ApplyOutcome::Applied(local_ids))
    }

    async fn record(&self, scope: OrgScope, batch: &ReviewBatch) -> Result<Recorded, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        if !lock_open(&mut tx, scope, batch.changeset_id, &batch.proposals).await? {
            return Ok(Recorded::NotOpen);
        }
        for proposal in &batch.proposals {
            insert_result(
                &mut tx,
                scope,
                *proposal,
                batch.outcome.review_outcome(),
                None,
                &batch.reviewer,
                batch.now,
            )
            .await
            .map_err(store_error)?;
        }
        for entry in &batch.audit {
            audit::record(&mut tx, entry).await.map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(Recorded::Recorded)
    }

    async fn open_changesets(
        &self,
        scope: OrgScope,
        event: Option<EventId>,
    ) -> Result<Vec<OpenChangeset>, StoreError> {
        let rows = sqlx::query!(
            r#"SELECT c.id, c.event_id, c.author, c.created_at AS "created_at: jiff_sqlx::Timestamp",
                      count(*) AS "open_proposals!"
               FROM changeset c
               JOIN proposal p ON p.organization_id = c.organization_id AND p.changeset_id = c.id
               WHERE c.organization_id = $1 AND ($2::uuid IS NULL OR c.event_id = $2)
                 AND NOT EXISTS (
                     SELECT 1 FROM review_result r
                     WHERE r.organization_id = p.organization_id AND r.proposal_id = p.id
                 )
               GROUP BY c.id
               ORDER BY c.created_at, c.id"#,
            scope.organization_id().as_uuid(),
            event.map(EventId::as_uuid) as Option<Uuid>,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(OpenChangeset {
                    id: ChangesetId::from_uuid(row.id),
                    event_id: row.event_id.map(EventId::from_uuid),
                    author: actor::from_json(&row.author)?,
                    created_at: row.created_at.to_jiff(),
                    open_proposals: u32::try_from(row.open_proposals)
                        .map_err(|_| InvalidRow("proposal"))?,
                })
            })
            .collect()
    }
}

/// Locks the changeset for the rest of the transaction, then checks that none of `proposals` has a review result.
async fn lock_open(
    conn: &mut PgConnection,
    scope: OrgScope,
    changeset: ChangesetId,
    proposals: &[ProposalId],
) -> Result<bool, StoreError> {
    let organization = scope.organization_id().as_uuid();
    let locked = sqlx::query_scalar!(
        "SELECT id FROM changeset WHERE organization_id = $1 AND id = $2 FOR UPDATE",
        organization,
        changeset.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_error)?;
    if locked.is_none() {
        return Ok(false);
    }
    let ids: Vec<Uuid> = proposals.iter().map(|id| id.as_uuid()).collect();
    let reviewed = sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM review_result WHERE organization_id = $1 AND proposal_id = ANY($2)
           ) AS "reviewed!""#,
        organization,
        &ids,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(store_error)?;
    Ok(!reviewed)
}

/// Locks the existing field definitions and facts that the plan uses or changes, the local ID counters
/// that it moves and the existing documents of its drafts, before any check. It keeps the lock order of the crate
/// documentation: fields, then facts, then counters, then documents, each kind in the order of its keys. So two applies take their row locks in the same order and
/// cannot deadlock on them. The plan order follows the dependencies and the client IDs.
/// A missing counter row is inserted here, so the lock covers it; a rollback removes it again.
/// Inserts of other new rows can still wait on each other; a deadlock there maps to `Unavailable`,
/// so the client retries.
async fn lock_targets(
    conn: &mut PgConnection,
    scope: OrgScope,
    steps: &[ApplyStep],
) -> Result<(), sqlx::Error> {
    let mut fields = Vec::new();
    let (mut fact_events, mut fact_fields) = (Vec::new(), Vec::new());
    // The counters to move, as (scope, kind): `QST` in an event, `DOC` in the organization (ADR 0038).
    let (mut counter_scopes, mut counter_kinds) = (Vec::new(), Vec::new());
    let mut documents = Vec::new();
    let organization = scope.organization_id().as_uuid();
    for step in steps {
        match &step.operation {
            Operation::SetFact {
                event_id, field_id, ..
            } => {
                fields.push(field_id.as_uuid());
                fact_events.push(event_id.as_uuid());
                fact_fields.push(field_id.as_uuid());
            }
            Operation::AddChoiceValue { field_id, .. }
            | Operation::DeprecateField { field_id, .. } => {
                fields.push(field_id.as_uuid());
            }
            Operation::CreateOpenQuestion { event_id, .. } => {
                counter_scopes.push(event_id.as_uuid());
                counter_kinds.push(OPEN_QUESTION_PREFIX);
            }
            Operation::CreateDocumentDraft { document, .. } => match document {
                DraftDocument::New { .. } => {
                    counter_scopes.push(organization);
                    counter_kinds.push(DOCUMENT_COUNTER);
                }
                DraftDocument::Existing { document_id, .. } => {
                    documents.push(document_id.as_uuid());
                }
            },
            Operation::CreateEvent { .. } | Operation::AddFieldDefinition { .. } => {}
        }
    }
    // Shipped fields have no organization and change only with `tada migrate`, so they need no lock.
    sqlx::query_scalar!(
        "SELECT id FROM field_definition WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        organization,
        &fields,
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query_scalar!(
        "SELECT f.id FROM fact f
         JOIN unnest($2::uuid[], $3::uuid[]) AS t (event_id, field_id)
           ON f.event_id = t.event_id AND f.field_id = t.field_id
         WHERE f.organization_id = $1
         ORDER BY f.id
         FOR UPDATE OF f",
        organization,
        &fact_events,
        &fact_fields,
    )
    .fetch_all(&mut *conn)
    .await?;
    let counter_kinds: Vec<String> = counter_kinds.into_iter().map(str::to_owned).collect();
    // A new counter starts at 1, the value that `next_local_number` would give.
    sqlx::query!(
        "INSERT INTO local_id_counter (organization_id, scope_id, kind, next)
         SELECT DISTINCT $1::uuid, t.scope_id, t.kind, 1::bigint
         FROM unnest($2::uuid[], $3::text[]) AS t (scope_id, kind)
         ORDER BY t.scope_id, t.kind
         ON CONFLICT (organization_id, scope_id, kind) DO NOTHING",
        organization,
        &counter_scopes,
        &counter_kinds,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query_scalar!(
        "SELECT c.scope_id FROM local_id_counter c
         JOIN unnest($2::uuid[], $3::text[]) AS t (scope_id, kind)
           ON c.scope_id = t.scope_id AND c.kind = t.kind
         WHERE c.organization_id = $1
         ORDER BY c.scope_id, c.kind
         FOR UPDATE OF c",
        organization,
        &counter_scopes,
        &counter_kinds,
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query_scalar!(
        "SELECT id FROM document WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        organization,
        &documents,
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(())
}

/// The proposals whose targets do not have the expected versions (ADR 0050).
/// A record that an earlier step of the plan creates counts as new, and a fact that an earlier step sets
/// counts with its new version, so the checks of all steps run before the first write.
async fn check_versions(
    conn: &mut PgConnection,
    scope: OrgScope,
    steps: &[ApplyStep],
) -> Result<Vec<ProposalId>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let mut new_records: HashSet<Uuid> = HashSet::new();
    let mut fact_versions: HashMap<(EventId, FieldDefinitionId), Option<i64>> = HashMap::new();
    let mut document_versions: HashMap<DocumentId, i64> = HashMap::new();
    let mut conflicts = Vec::new();
    for step in steps {
        let matches = match &step.operation {
            Operation::CreateEvent { id, .. } => {
                // An event ID is unique in the whole installation (ADR 0038).
                !sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM event WHERE id = $1) AS "exists!""#,
                    id.as_uuid(),
                )
                .fetch_one(&mut *conn)
                .await?
            }
            Operation::AddFieldDefinition { id, .. } => {
                !sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM field_definition WHERE id = $1) AS "exists!""#,
                    id.as_uuid(),
                )
                .fetch_one(&mut *conn)
                .await?
            }
            Operation::CreateOpenQuestion { id, .. } => {
                !sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM open_question WHERE id = $1) AS "exists!""#,
                    id.as_uuid(),
                )
                .fetch_one(&mut *conn)
                .await?
            }
            Operation::SetFact {
                event_id,
                field_id,
                expected_version,
                ..
            } => {
                let new_scope = new_records.contains(&event_id.as_uuid())
                    || new_records.contains(&field_id.as_uuid());
                let current = match fact_versions.get(&(*event_id, *field_id)) {
                    Some(version) => *version,
                    None if new_scope => None,
                    None => {
                        sqlx::query_scalar!(
                            "SELECT version FROM fact
                             WHERE organization_id = $1 AND event_id = $2 AND field_id = $3
                             FOR UPDATE",
                            organization,
                            event_id.as_uuid(),
                            field_id.as_uuid(),
                        )
                        .fetch_optional(&mut *conn)
                        .await?
                    }
                };
                // A deprecated field takes no new facts (ADR 0049). `lock_targets` locked a field of the event.
                let active = new_records.contains(&field_id.as_uuid())
                        || sqlx::query_scalar!(
                            r#"SELECT EXISTS (
                                   SELECT 1 FROM field_definition
                                   WHERE id = $1 AND status = 'active'
                                     AND (event_id IS NULL OR (organization_id = $2 AND event_id = $3))
                               ) AS "active!""#,
                            field_id.as_uuid(),
                            organization,
                            event_id.as_uuid(),
                        )
                        .fetch_one(&mut *conn)
                        .await?;
                fact_versions.insert((*event_id, *field_id), Some(current.unwrap_or(0) + 1));
                active && current == expected_version.map(RecordVersion::get)
            }
            Operation::AddChoiceValue {
                event_id,
                field_id,
                key,
                ..
            } => {
                // A deprecated field takes no new choices either.
                if new_records.contains(&field_id.as_uuid()) {
                    true
                } else {
                    match field_of_event(conn, scope, *event_id, *field_id).await? {
                        Some((ValueType::Choice { values, .. }, FieldStatus::Active)) => {
                            !values.iter().any(|value| &value.key == key)
                        }
                        _ => false,
                    }
                }
            }
            Operation::DeprecateField { event_id, field_id } => {
                new_records.contains(&field_id.as_uuid())
                    || matches!(
                        field_of_event(conn, scope, *event_id, *field_id).await?,
                        Some((_, FieldStatus::Active))
                    )
            }
            Operation::CreateDocumentDraft {
                event_id, document, ..
            } => match document {
                DraftDocument::New { id, .. } => {
                    document_versions.insert(*id, 1);
                    !sqlx::query_scalar!(
                        r#"SELECT EXISTS (SELECT 1 FROM document WHERE id = $1) AS "exists!""#,
                        id.as_uuid(),
                    )
                    .fetch_one(&mut *conn)
                    .await?
                }
                DraftDocument::Existing {
                    document_id,
                    expected_version,
                } => {
                    // A document that an earlier step of the plan changes counts with its new version.
                    let current = match document_versions.get(document_id) {
                        Some(version) => Some(*version),
                        None => {
                            sqlx::query_scalar!(
                                "SELECT version FROM document
                                 WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                                organization,
                                event_id.as_uuid(),
                                document_id.as_uuid(),
                            )
                            .fetch_optional(&mut *conn)
                            .await?
                        }
                    };
                    document_versions.insert(*document_id, expected_version.get() + 1);
                    current == Some(expected_version.get())
                }
            },
        };
        if let Some(record) = step.operation.new_record() {
            new_records.insert(record.as_uuid());
        }
        if !matches {
            conflicts.push(step.proposal_id);
        }
    }
    Ok(conflicts)
}

/// The value type and the status of the field of the event, locked for the rest of the transaction, or `None`.
async fn field_of_event(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    field: FieldDefinitionId,
) -> Result<Option<(ValueType, FieldStatus)>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT value_type, status FROM field_definition
         WHERE organization_id = $1 AND event_id = $2 AND id = $3
         FOR UPDATE",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        field.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    let decode = |error: InvalidRow| sqlx::Error::Decode(Box::new(error));
    row.map(|row| {
        let status = match row.status.as_str() {
            "active" => FieldStatus::Active,
            "deprecated" => FieldStatus::Deprecated,
            _ => return Err(decode(InvalidRow("field_definition.status"))),
        };
        Ok((
            values::value_type_from_json(&row.value_type).map_err(decode)?,
            status,
        ))
    })
    .transpose()
}

/// Writes one step and its review result. Returns the event-local ID of a new open question.
async fn write_step(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    step: &ApplyStep,
) -> Result<Option<NewLocalId>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let now = plan.now.to_sqlx();
    let mut local_id = None;
    let mut edit_source = None;
    match &step.operation {
        Operation::CreateEvent {
            id,
            key,
            name,
            time_zone,
        } => {
            let event = Event {
                id: *id,
                organization_id: scope.organization_id(),
                key: key.clone(),
                name: name.clone(),
                time_zone: time_zone.clone(),
                version: RecordVersion::FIRST,
                created_at: plan.now,
            };
            // The reviewer who accepts the new event is its first event manager (ADR 0052).
            events::insert_event(conn, scope, &event, plan.manager).await?;
        }
        Operation::AddFieldDefinition {
            id,
            event_id,
            key,
            label,
            value_type,
            description,
            module,
        } => {
            sqlx::query!(
                "INSERT INTO field_definition (id, organization_id, event_id, key, label_text, value_type,
                                               description, module, status, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', $9)",
                id.as_uuid(),
                organization,
                event_id.as_uuid(),
                key.as_str(),
                label.as_str(),
                values::value_type_to_json(value_type),
                description.as_str(),
                module.as_str(),
                now as _,
            )
            .execute(&mut *conn)
            .await?;
        }
        Operation::AddChoiceValue {
            event_id,
            field_id,
            key,
            label,
        } => {
            let Some((
                ValueType::Choice {
                    mut values,
                    multiple,
                },
                _,
            )) = field_of_event(conn, scope, *event_id, *field_id).await?
            else {
                return Err(sqlx::Error::RowNotFound);
            };
            values.push(ChoiceValue {
                key: key.clone(),
                label: Label::Text(label.clone()),
            });
            sqlx::query!(
                "UPDATE field_definition SET value_type = $4
                 WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event_id.as_uuid(),
                field_id.as_uuid(),
                values::value_type_to_json(&ValueType::Choice { values, multiple }),
            )
            .execute(&mut *conn)
            .await?;
        }
        Operation::DeprecateField { event_id, field_id } => {
            sqlx::query!(
                "UPDATE field_definition SET status = 'deprecated'
                 WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event_id.as_uuid(),
                field_id.as_uuid(),
            )
            .execute(&mut *conn)
            .await?;
        }
        Operation::CreateOpenQuestion {
            id,
            event_id,
            text,
            owner,
        } => {
            let number =
                next_local_number(conn, scope, event_id.as_uuid(), OPEN_QUESTION_PREFIX).await?;
            sqlx::query!(
                "INSERT INTO open_question
                     (id, organization_id, event_id, local_number, text, owner_user_id, status, version, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, 'open', 1, $7)",
                id.as_uuid(),
                organization,
                event_id.as_uuid(),
                number,
                text.as_str(),
                owner.as_uuid(),
                now as _,
            )
            .execute(&mut *conn)
            .await?;
            local_id = Some(NewLocalId {
                record: LocalRecord::OpenQuestion(OpenQuestionId::from_uuid(id.as_uuid())),
                local_number: u64::try_from(number)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            });
        }
        Operation::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } => {
            let (document, number, new_local_id) =
                draft_target(conn, scope, plan, *event_id, document).await?;
            local_id = new_local_id;
            insert_draft(
                conn,
                scope,
                plan,
                step.proposal_id,
                document,
                number,
                markdown,
            )
            .await?;
        }
        Operation::SetFact {
            event_id,
            field_id,
            state,
            expected_version,
        } => {
            let (source, passages) = match &step.evidence {
                StepEvidence::Proposal(passages) => (plan.source_version_id, passages.clone()),
                StepEvidence::Edit => {
                    let (source, passage) =
                        insert_review_text(conn, scope, plan, *event_id, state).await?;
                    edit_source = Some(source);
                    (source, vec![passage])
                }
            };
            let fact = FactWrite {
                event: *event_id,
                field: *field_id,
                expected: *expected_version,
                state,
                source,
                passages: &passages,
            };
            insert_fact_version(conn, scope, plan, step, fact).await?;
        }
    }
    let outcome = match step.evidence {
        StepEvidence::Edit => ReviewOutcome::AcceptedWithEdit,
        StepEvidence::Proposal(_) => ReviewOutcome::Accepted,
    };
    insert_result(
        conn,
        scope,
        step.proposal_id,
        outcome,
        edit_source,
        &plan.reviewer,
        plan.now,
    )
    .await?;
    Ok(local_id)
}

/// The next local number of `kind` in its scope: an event, or the organization (ADR 0038).
/// The counter changes in the transaction of the caller, so a rollback takes no number and a number is never given twice.
async fn next_local_number(
    conn: &mut PgConnection,
    scope: OrgScope,
    counter_scope: Uuid,
    kind: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"INSERT INTO local_id_counter (organization_id, scope_id, kind, next)
           VALUES ($1, $2, $3, 2)
           ON CONFLICT (organization_id, scope_id, kind) DO UPDATE SET next = local_id_counter.next + 1
           RETURNING next - 1 AS "number!""#,
        scope.organization_id().as_uuid(),
        counter_scope,
        kind,
    )
    .fetch_one(&mut *conn)
    .await
}

/// The document of a draft step and the number of its new version, with the `DOC-<n>` of a new document.
/// A new document gets the next number of the organization; the reviewer owns it (ADR 0038, ADR 0051).
/// An existing document gets its next record version; the write repeats the expected version,
/// so a changed document updates no row (`RowNotFound`), which is a conflict.
async fn draft_target(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    event: EventId,
    document: &DraftDocument,
) -> Result<(DocumentId, i32, Option<NewLocalId>), sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    match document {
        DraftDocument::New { id, name } => {
            let number = next_local_number(conn, scope, organization, DOCUMENT_COUNTER).await?;
            sqlx::query!(
                "INSERT INTO document
                     (id, organization_id, event_id, local_number, name, owner_user_id, created_at, version)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
                id.as_uuid(),
                organization,
                event.as_uuid(),
                number,
                name.as_str(),
                plan.manager.as_uuid(),
                plan.now.to_sqlx() as _,
            )
            .execute(&mut *conn)
            .await?;
            let local_id = NewLocalId {
                record: LocalRecord::Document(*id),
                local_number: u64::try_from(number)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            };
            Ok((*id, 1, Some(local_id)))
        }
        DraftDocument::Existing {
            document_id,
            expected_version,
        } => {
            sqlx::query_scalar!(
                "UPDATE document SET version = version + 1
                 WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
                 RETURNING id",
                organization,
                event.as_uuid(),
                document_id.as_uuid(),
                expected_version.get(),
            )
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            let number = sqlx::query_scalar!(
                r#"SELECT coalesce(max(number), 0) + 1 AS "number!"
                   FROM document_version WHERE organization_id = $1 AND document_id = $2"#,
                organization,
                document_id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            Ok((*document_id, number, None))
        }
    }
}

/// Adds the draft version `number` of the document with its Markdown, and copies the provenance manifest that
/// the proposal fixed at its creation into the manifest rows of the version (ADR 0051).
/// The member who accepts the proposal adds the version.
async fn insert_draft(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    proposal: ProposalId,
    document: DocumentId,
    number: i32,
    markdown: &DraftMarkdown,
) -> Result<(), sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let version = Uuid::now_v7();
    let sha256: [u8; 32] = Sha256::digest(markdown.as_str().as_bytes()).into();
    sqlx::query!(
        "INSERT INTO document_version
             (id, organization_id, document_id, number, kind, sha256, uploaded_by, status, created_at, markdown)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $5, $8, $9)",
        version,
        organization,
        document.as_uuid(),
        number,
        DRAFT,
        &sha256[..],
        plan.manager.as_uuid(),
        plan.now.to_sqlx() as _,
        markdown.as_str(),
    )
    .execute(&mut *conn)
    .await?;
    // The manifest that the proposal fixed at its creation, read with the one codec of its format.
    let stored = sqlx::query_scalar!(
        r#"SELECT manifest AS "manifest!" FROM proposal
           WHERE organization_id = $1 AND id = $2 AND manifest IS NOT NULL"#,
        organization,
        proposal.as_uuid(),
    )
    .fetch_one(&mut *conn)
    .await?;
    let manifest = drafts::manifest_from_json(&stored)
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    let (facts, numbers): (Vec<Uuid>, Vec<i64>) = manifest
        .facts
        .iter()
        .map(|fact| (fact.fact_id.as_uuid(), fact.version.get()))
        .unzip();
    sqlx::query!(
        "INSERT INTO document_manifest_fact (organization_id, document_version_id, fact_id, fact_version_number)
         SELECT $1, $2, t.fact_id, t.number FROM unnest($3::uuid[], $4::bigint[]) AS t (fact_id, number)",
        organization,
        version,
        &facts,
        &numbers,
    )
    .execute(&mut *conn)
    .await?;
    let offset =
        |value: u32| i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)));
    let mut sources = Vec::new();
    let (mut starts, mut ends) = (Vec::new(), Vec::new());
    for cited in &manifest.sources {
        sources.push(cited.source_version_id.as_uuid());
        starts.push(offset(cited.passage.start)?);
        ends.push(offset(cited.passage.end)?);
    }
    sqlx::query!(
        "INSERT INTO document_manifest_source
             (organization_id, document_version_id, source_version_id, start_offset, end_offset)
         SELECT $1, $2, t.source_version_id, t.start_offset, t.end_offset
         FROM unnest($3::uuid[], $4::int[], $5::int[]) AS t (source_version_id, start_offset, end_offset)",
        organization,
        version,
        &sources,
        &starts,
        &ends,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Stores an edited state as a source version of the kind `review` with the reviewer as author (ADR 0050).
/// Returns it with the passage of its whole text, the evidence of the edited value.
///
/// The text is the JSON object `{"state", "value", "approximate"}` with the columns of `values::fact_state_to_columns`.
/// A source version never changes, so this format is a stable contract: a change of the value codec must keep it,
/// or add a new format next to it that readers tell apart.
async fn insert_review_text(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    event: EventId,
    state: &FactState<Valued>,
) -> Result<(SourceVersionId, Passage), sqlx::Error> {
    let (state, value, approximate) = values::fact_state_to_columns(state);
    let json = serde_json::json!({"state": state, "value": value, "approximate": approximate});
    let text = SourceText::normalize(&json.to_string());
    let version = SourceVersionId::from_uuid(Uuid::now_v7());
    let item = TextItem {
        kind: TextKind::Review,
        event: Some(event),
        version,
    };
    sources::insert_text(conn, scope, item, &text, &plan.reviewer, plan.now).await?;
    let length = u32::try_from(text.as_str().chars().count())
        .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    Ok((
        version,
        Passage {
            start: 0,
            end: length,
            quote: text.as_str().to_owned(),
            page: None,
        },
    ))
}

/// The next state of the fact of `field` in `event`, with its evidence: passages of the source version `source`.
struct FactWrite<'a> {
    event: EventId,
    field: FieldDefinitionId,
    /// The current version that the step expects; `None` means that the fact does not exist yet.
    expected: Option<RecordVersion>,
    state: &'a FactState<Valued>,
    source: SourceVersionId,
    passages: &'a [Passage],
}

/// Adds the next version of a fact, with its evidence.
async fn insert_fact_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    step: &ApplyStep,
    fact: FactWrite<'_>,
) -> Result<(), sqlx::Error> {
    let FactWrite {
        event,
        field,
        expected,
        state,
        source,
        passages,
    } = fact;
    let organization = scope.organization_id().as_uuid();
    let now = plan.now.to_sqlx();
    // The write repeats the expected version: a fact that a concurrent apply created after the check breaks the
    // unique constraint of the insert, and a changed version updates no row (`RowNotFound`). Both are conflicts.
    let (fact, number) = match expected {
        Some(expected) => {
            let number = expected.get() + 1;
            let fact = sqlx::query_scalar!(
                "UPDATE fact SET version = $5
                 WHERE organization_id = $1 AND event_id = $2 AND field_id = $3 AND version = $4
                 RETURNING id",
                organization,
                event.as_uuid(),
                field.as_uuid(),
                expected.get(),
                number,
            )
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            (fact, number)
        }
        None => {
            let id = Uuid::now_v7();
            sqlx::query!(
                "INSERT INTO fact (id, organization_id, event_id, field_id, version) VALUES ($1, $2, $3, $4, 1)",
                id,
                organization,
                event.as_uuid(),
                field.as_uuid(),
            )
            .execute(&mut *conn)
            .await?;
            (id, 1)
        }
    };
    let (state, value, approximate) = values::fact_state_to_columns(state);
    let version = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO fact_version
             (id, organization_id, fact_id, number, state, value, approximate, created_at, accepted_by, proposal_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        version,
        organization,
        fact,
        number,
        state,
        value,
        approximate,
        now as _,
        actor::to_json(&plan.reviewer),
        step.proposal_id.as_uuid(),
    )
    .execute(&mut *conn)
    .await?;
    let offset =
        |value: u32| i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)));
    for passage in passages {
        sqlx::query!(
            "INSERT INTO evidence_link
                 (id, organization_id, fact_version_id, source_version_id, start_offset, end_offset, quote, page)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            Uuid::now_v7(),
            organization,
            version,
            source.as_uuid(),
            offset(passage.start)?,
            offset(passage.end)?,
            passage.quote,
            passage.page.map(offset).transpose()?,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn insert_result(
    conn: &mut PgConnection,
    scope: OrgScope,
    proposal: ProposalId,
    outcome: ReviewOutcome,
    edit_source: Option<SourceVersionId>,
    reviewer: &Actor,
    now: Timestamp,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, edit_source_version_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        Uuid::now_v7(),
        scope.organization_id().as_uuid(),
        proposal.as_uuid(),
        outcome.as_str(),
        actor::to_json(reviewer),
        edit_source.map(SourceVersionId::as_uuid),
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests;
