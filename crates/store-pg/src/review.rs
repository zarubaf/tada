//! The `ReviewStore` adapter (ADR 0050): review results, the apply of a changeset and the Review Inbox.
//!
//! Each write locks its changeset first, so two reviews of one changeset run one after the other.
//! Review results are append-only: the store never updates or deletes them.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::{Actor, OrgScope};
use tada_app::domain::RecordVersion;
use tada_app::domain::facts::{ChoiceValue, FactState, Label, ValueType, Valued};
use tada_app::domain::ids::{
    ChangesetId, EventId, FieldDefinitionId, OpenQuestionId, ProposalId, SourceVersionId,
};
use tada_app::domain::proposals::Operation;
use tada_app::domain::sources::{Passage, SourceText};
use tada_app::review::{
    ApplyOutcome, ApplyPlan, ApplyStep, NewLocalId, NewReviewResult, OpenChangeset, Recorded,
    ReviewOutcome, ReviewRecord, ReviewStore, StepEvidence,
};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::sources::{TextItem, TextKind};
use crate::{actor, audit, sources, values};

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
                Err(error) => return Err(store_error(error)),
            }
        }
        for entry in &plan.audit {
            audit::record(&mut tx, entry).await.map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(ApplyOutcome::Applied(local_ids))
    }

    async fn record(
        &self,
        scope: OrgScope,
        changeset: ChangesetId,
        results: &[NewReviewResult],
        reviewer: &Actor,
        now: Timestamp,
        audit: &[AuditEvent],
    ) -> Result<Recorded, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let proposals: Vec<ProposalId> = results.iter().map(|result| result.proposal_id).collect();
        if !lock_open(&mut tx, scope, changeset, &proposals).await? {
            return Ok(Recorded::NotOpen);
        }
        for result in results {
            insert_result(
                &mut tx,
                scope,
                result.proposal_id,
                result.outcome,
                None,
                reviewer,
                now,
            )
            .await
            .map_err(store_error)?;
        }
        for entry in audit {
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
    let mut conflicts = Vec::new();
    for step in steps {
        let matches =
            match &step.operation {
                Operation::CreateEvent { id, .. } => {
                    // An event ID is unique in the whole installation (ADR 0038).
                    !sqlx::query_scalar!(
                        r#"SELECT EXISTS (SELECT 1 FROM event WHERE id = $1) AS "exists!""#,
                        id.as_uuid(),
                    )
                    .fetch_one(&mut *conn)
                    .await?
                }
                Operation::AddFieldDefinition { id, .. } => !sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM field_definition WHERE id = $1) AS "exists!""#,
                    id.as_uuid(),
                )
                .fetch_one(&mut *conn)
                .await?,
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
                    // A deprecated field takes no new facts (ADR 0049).
                    let active = new_records.contains(&field_id.as_uuid())
                        || sqlx::query_scalar!(
                            r#"SELECT EXISTS (
                               SELECT 1 FROM field_definition WHERE id = $1 AND status = 'active'
                           ) AS "active!""#,
                            field_id.as_uuid(),
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
                    if new_records.contains(&field_id.as_uuid()) {
                        true
                    } else {
                        match field_value_type(conn, scope, *event_id, *field_id).await? {
                            Some(ValueType::Choice { values, .. }) => {
                                !values.iter().any(|value| &value.key == key)
                            }
                            _ => false,
                        }
                    }
                }
                Operation::DeprecateField { event_id, field_id } => {
                    new_records.contains(&field_id.as_uuid())
                        || field_value_type(conn, scope, *event_id, *field_id)
                            .await?
                            .is_some()
                }
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

/// The value type of the field of the event, locked for the rest of the transaction, or `None`.
async fn field_value_type(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    field: FieldDefinitionId,
) -> Result<Option<ValueType>, sqlx::Error> {
    let json = sqlx::query_scalar!(
        "SELECT value_type FROM field_definition
         WHERE organization_id = $1 AND event_id = $2 AND id = $3
         FOR UPDATE",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        field.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    json.map(|json| values::value_type_from_json(&json))
        .transpose()
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))
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
            sqlx::query!(
                "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
                 VALUES ($1, $2, $3, $4, $5, 1, $6)",
                id.as_uuid(),
                organization,
                key.as_str(),
                name.as_str(),
                time_zone.as_str(),
                now as _,
            )
            .execute(&mut *conn)
            .await?;
            // An event has at least one event manager: here the reviewer who accepts it (ADR 0052).
            sqlx::query!(
                "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
                 VALUES ($1, $2, $3, 'event-manager', 1, $4)",
                organization,
                id.as_uuid(),
                plan.manager.as_uuid(),
                now as _,
            )
            .execute(&mut *conn)
            .await?;
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
            let Some(ValueType::Choice {
                mut values,
                multiple,
            }) = field_value_type(conn, scope, *event_id, *field_id).await?
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
            let number = next_local_number(conn, scope, *event_id, OPEN_QUESTION_PREFIX).await?;
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
                open_question: OpenQuestionId::from_uuid(id.as_uuid()),
                local_number: u64::try_from(number)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            });
        }
        Operation::SetFact {
            event_id,
            field_id,
            state,
            ..
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
            insert_fact_version(
                conn, scope, plan, step, *event_id, *field_id, state, source, &passages,
            )
            .await?;
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

/// The next event-local number of `kind` in the event (ADR 0038). The counter changes in the transaction of the
/// caller, so a rollback takes no number and a number is never given twice.
async fn next_local_number(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    kind: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"INSERT INTO local_id_counter (organization_id, scope_id, kind, next)
           VALUES ($1, $2, $3, 2)
           ON CONFLICT (organization_id, scope_id, kind) DO UPDATE SET next = local_id_counter.next + 1
           RETURNING next - 1 AS "number!""#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        kind,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Stores an edited state as a source version of the kind `review` with the reviewer as author (ADR 0050).
/// Returns it with the passage of its whole text, the evidence of the edited value.
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

/// Adds the next version of the fact of `field` in the event, with its evidence.
#[allow(clippy::too_many_arguments)]
async fn insert_fact_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    step: &ApplyStep,
    event: EventId,
    field: FieldDefinitionId,
    state: &FactState<Valued>,
    source: SourceVersionId,
    passages: &[Passage],
) -> Result<(), sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let now = plan.now.to_sqlx();
    let current = sqlx::query!(
        "SELECT id, version FROM fact
         WHERE organization_id = $1 AND event_id = $2 AND field_id = $3
         FOR UPDATE",
        organization,
        event.as_uuid(),
        field.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    let (fact, number) = match current {
        Some(fact) => {
            let number = fact.version + 1;
            sqlx::query!(
                "UPDATE fact SET version = $3 WHERE organization_id = $1 AND id = $2",
                organization,
                fact.id,
                number,
            )
            .execute(&mut *conn)
            .await?;
            (fact.id, number)
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
