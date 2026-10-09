//! The apply of the proposals that create or change work records and parties (ADR 0068, ADR 0069).
//!
//! Each write copies the passages of the proposal into `record_evidence` with the record version that it produced.
//! A new record takes the next readable number of its scope: `ACT` and `COM` in the event, `PER` and `INS` in the organization.

use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::ids::{EventId, LocalIdKind, ProposalId, SourceVersionId, WorkstreamId};
use tada_app::domain::parties::Party;
use tada_app::domain::proposals::Operation;
use tada_app::domain::sources::Evidence;
use tada_app::domain::work::{ActionStatus, CommitmentStatus};
use tada_app::parties::{InstitutionFields, PersonFields};
use tada_app::review::{ApplyPlan, ApplyStep, LocalRecord, NewLocalId, StepEvidence};
use tada_app::work::{ActionFields, CommitmentFields, NewActionRecord, NewCommitmentRecord};

use super::insert_review_text;
use crate::parties::{insert_institution, insert_person};
use crate::proposals::operation_to_json;
use crate::work::{
    action_in, commitment_in, insert_action, insert_commitment, update_action, update_commitment,
};

/// The record that a passage of `record_evidence` supports.
#[derive(Debug, Clone, Copy)]
enum RecordRef {
    Action(Uuid),
    Commitment(Uuid),
    Person(Uuid),
    Institution(Uuid),
}

/// What a step wrote besides the record: the readable ID of a new record, and the review text of an edit.
#[derive(Debug, Default)]
pub(super) struct Written {
    pub local_id: Option<NewLocalId>,
    pub edit_source: Option<SourceVersionId>,
}

/// Writes the record of one step and its evidence.
/// A change repeats the expected version, so a changed record updates no row (`RowNotFound`), which is a conflict.
pub(super) async fn write_record(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    step: &ApplyStep,
) -> Result<Written, sqlx::Error> {
    let mut written = Written::default();
    let evidence = match &step.evidence {
        StepEvidence::Proposal(evidence) => evidence.clone(),
        StepEvidence::RecordEdit(evidence) => {
            let review = insert_review_text(
                conn,
                scope,
                plan,
                step.operation.event_id().or(plan.event_id),
                &operation_to_json(&step.operation),
            )
            .await?;
            written.edit_source = Some(review.source_version_id);
            evidence.iter().cloned().chain([review]).collect()
        }
        StepEvidence::Edit => {
            return Err(sqlx::Error::Protocol(
                "a work record has no edited fact state".into(),
            ));
        }
    };
    let (record, version) = match &step.operation {
        Operation::CreatePerson {
            id,
            name,
            email,
            phone,
        } => {
            let fields = PersonFields {
                name: name.clone(),
                email: email.clone(),
                phone: phone.clone(),
            };
            let number = insert_person(conn, scope, *id, &fields, None, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Person(*id), number)?);
            (RecordRef::Person(id.as_uuid()), 1)
        }
        Operation::CreateInstitution {
            id,
            name,
            kind,
            email,
            phone,
        } => {
            let fields = InstitutionFields {
                name: name.clone(),
                kind: *kind,
                email: email.clone(),
                phone: phone.clone(),
            };
            let number = insert_institution(conn, scope, *id, &fields, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Institution(*id), number)?);
            (RecordRef::Institution(id.as_uuid()), 1)
        }
        Operation::CreateAction {
            id,
            event_id,
            title,
            description,
            owner,
            workstream,
            due_date,
        } => {
            let action = NewActionRecord {
                id: *id,
                event_id: *event_id,
                fields: ActionFields {
                    title: title.clone(),
                    description: description.clone(),
                    owner: *owner,
                    workstream_id: *workstream,
                    due_date: *due_date,
                    status: ActionStatus::Open,
                },
            };
            let number = insert_action(conn, scope, &action, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Action(*id), number)?);
            (RecordRef::Action(id.as_uuid()), 1)
        }
        Operation::CreateCommitment {
            id,
            event_id,
            text,
            promisor,
            owner,
            workstream,
            due_date,
            condition,
        } => {
            let commitment = NewCommitmentRecord {
                id: *id,
                event_id: *event_id,
                condition: condition.clone(),
                promisor: *promisor,
                fields: CommitmentFields {
                    text: text.clone(),
                    owner: *owner,
                    workstream_id: *workstream,
                    due_date: *due_date,
                    status: CommitmentStatus::initial(condition.as_ref()),
                    firm_reason: None,
                },
            };
            let number = insert_commitment(conn, scope, &commitment, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Commitment(*id), number)?);
            (RecordRef::Commitment(id.as_uuid()), 1)
        }
        Operation::ChangeActionStatus {
            event_id,
            action_id,
            status,
            expected_version,
        } => {
            let current = action_in(conn, scope, *event_id, *action_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            // The status of the record decides the transition (ADR 0068). A record that changed after the check
            // of the proposal has another version, so this refusal is a conflict like a version mismatch.
            let status = current
                .fields
                .status
                .change_to(*status)
                .map_err(|_| sqlx::Error::RowNotFound)?;
            let fields = ActionFields {
                status,
                ..current.fields
            };
            let version = update_action(
                conn,
                scope,
                *event_id,
                *action_id,
                &fields,
                *expected_version,
                plan.now,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            (RecordRef::Action(action_id.as_uuid()), version)
        }
        Operation::ChangeActionDue {
            event_id,
            action_id,
            due_date,
            expected_version,
        } => {
            let current = action_in(conn, scope, *event_id, *action_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            let fields = ActionFields {
                due_date: *due_date,
                ..current.fields
            };
            let version = update_action(
                conn,
                scope,
                *event_id,
                *action_id,
                &fields,
                *expected_version,
                plan.now,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            (RecordRef::Action(action_id.as_uuid()), version)
        }
        Operation::ChangeCommitmentStatus {
            event_id,
            commitment_id,
            status,
            expected_version,
        } => {
            // A proposal that makes the commitment firm gives its reason (ADR 0068). The condition stays as history.
            if *status == CommitmentStatus::Firm && step.firm_reason.is_none() {
                return Err(sqlx::Error::Protocol(
                    "a change to firm needs a reason".into(),
                ));
            }
            let current = commitment_in(conn, scope, *event_id, *commitment_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            let status = current
                .fields
                .status
                .change_to(*status)
                .map_err(|_| sqlx::Error::RowNotFound)?;
            let fields = CommitmentFields {
                status,
                firm_reason: step.firm_reason.clone().or(current.fields.firm_reason),
                ..current.fields
            };
            let version = update_commitment(
                conn,
                scope,
                *event_id,
                *commitment_id,
                &fields,
                *expected_version,
                plan.now,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            (RecordRef::Commitment(commitment_id.as_uuid()), version)
        }
        _ => {
            return Err(sqlx::Error::Protocol(
                "the step does not write a work record".into(),
            ));
        }
    };
    insert_evidence(conn, scope, record, version, step.proposal_id, &evidence).await?;
    Ok(written)
}

fn local_id(record: LocalRecord, number: i64) -> Result<NewLocalId, sqlx::Error> {
    Ok(NewLocalId {
        record,
        local_number: u64::try_from(number)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
    })
}

/// One row of `record_evidence` for each passage, with the record version that the step produced.
async fn insert_evidence(
    conn: &mut PgConnection,
    scope: OrgScope,
    record: RecordRef,
    version: i64,
    proposal: ProposalId,
    evidence: &[Evidence],
) -> Result<(), sqlx::Error> {
    let (action, commitment, person, institution) = match record {
        RecordRef::Action(id) => (Some(id), None, None, None),
        RecordRef::Commitment(id) => (None, Some(id), None, None),
        RecordRef::Person(id) => (None, None, Some(id), None),
        RecordRef::Institution(id) => (None, None, None, Some(id)),
    };
    let offset =
        |value: u32| i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)));
    for Evidence {
        source_version_id,
        passage,
    } in evidence
    {
        sqlx::query!(
            "INSERT INTO record_evidence
                 (id, organization_id, action_id, commitment_id, person_id, institution_id, record_version,
                  proposal_id, source_version_id, start_offset, end_offset, quote, page)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            Uuid::now_v7(),
            scope.organization_id().as_uuid(),
            action,
            commitment,
            person,
            institution,
            version,
            proposal.as_uuid(),
            source_version_id.as_uuid(),
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

/// The version check of `check_versions` for one work step: a new record whose ID is free and whose workstream
/// and promisor can take it, or a changed record with the expected version.
/// `new_records` holds the records that earlier steps of the plan create; `versions` the versions that earlier
/// steps of the plan give.
pub(super) async fn matches(
    conn: &mut PgConnection,
    scope: OrgScope,
    operation: &Operation,
    new_records: &std::collections::HashSet<Uuid>,
    versions: &mut std::collections::HashMap<Uuid, i64>,
) -> Result<bool, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    Ok(match operation {
        Operation::CreatePerson { id, .. } => {
            !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM person WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?
        }
        Operation::CreateInstitution { id, .. } => {
            !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM institution WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?
        }
        Operation::CreateAction {
            id,
            event_id,
            workstream,
            ..
        } => {
            let free = !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM action WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            free && takes_records(conn, scope, *event_id, *workstream).await?
        }
        Operation::CreateCommitment {
            id,
            event_id,
            workstream,
            promisor,
            ..
        } => {
            let free = !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM commitment WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            let promisor_exists = new_records.contains(&promisor.as_uuid())
                || party_exists(conn, scope, *promisor).await?;
            free && promisor_exists && takes_records(conn, scope, *event_id, *workstream).await?
        }
        Operation::ChangeActionStatus {
            event_id,
            action_id,
            expected_version,
            ..
        }
        | Operation::ChangeActionDue {
            event_id,
            action_id,
            expected_version,
            ..
        } => {
            let current = match versions.get(&action_id.as_uuid()) {
                Some(version) => Some(*version),
                None => {
                    sqlx::query_scalar!(
                        "SELECT version FROM action WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                        organization,
                        event_id.as_uuid(),
                        action_id.as_uuid(),
                    )
                    .fetch_optional(&mut *conn)
                    .await?
                }
            };
            versions.insert(action_id.as_uuid(), expected_version.get() + 1);
            current == Some(expected_version.get())
        }
        Operation::ChangeCommitmentStatus {
            event_id,
            commitment_id,
            expected_version,
            ..
        } => {
            let current = match versions.get(&commitment_id.as_uuid()) {
                Some(version) => Some(*version),
                None => {
                    sqlx::query_scalar!(
                        "SELECT version FROM commitment WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                        organization,
                        event_id.as_uuid(),
                        commitment_id.as_uuid(),
                    )
                    .fetch_optional(&mut *conn)
                    .await?
                }
            };
            versions.insert(commitment_id.as_uuid(), expected_version.get() + 1);
            current == Some(expected_version.get())
        }
        _ => false,
    })
}

/// True if a new record of the event can have the workstream: none, or an active workstream of the event (ADR 0067).
/// `lock_targets` locked the workstream, so it cannot close before the commit.
async fn takes_records(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    workstream: Option<WorkstreamId>,
) -> Result<bool, sqlx::Error> {
    let Some(workstream) = workstream else {
        return Ok(true);
    };
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM workstream
               WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND status = 'active'
           ) AS "active!""#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        workstream.as_uuid(),
    )
    .fetch_one(&mut *conn)
    .await
}

async fn party_exists(
    conn: &mut PgConnection,
    scope: OrgScope,
    party: Party,
) -> Result<bool, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    match party {
        Party::Person(id) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM person WHERE organization_id = $1 AND id = $2) AS "exists!""#,
                organization,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await
        }
        Party::Institution(id) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM institution WHERE organization_id = $1 AND id = $2) AS "exists!""#,
                organization,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await
        }
    }
}

/// The IDs that `lock_targets` locks for the work steps of a plan.
#[derive(Debug, Default)]
pub(super) struct WorkLocks {
    pub workstreams: Vec<Uuid>,
    pub actions: Vec<Uuid>,
    pub commitments: Vec<Uuid>,
}

impl WorkLocks {
    /// Adds the targets of one step. Returns the counter `(scope, kind)` that a new record moves.
    pub fn add(&mut self, scope: OrgScope, operation: &Operation) -> Option<(Uuid, &'static str)> {
        let organization = scope.organization_id().as_uuid();
        match operation {
            Operation::CreatePerson { .. } => Some((organization, LocalIdKind::Person.prefix())),
            Operation::CreateInstitution { .. } => {
                Some((organization, LocalIdKind::Institution.prefix()))
            }
            Operation::CreateAction {
                event_id,
                workstream,
                ..
            } => {
                self.workstreams
                    .extend(workstream.map(WorkstreamId::as_uuid));
                Some((event_id.as_uuid(), LocalIdKind::Action.prefix()))
            }
            Operation::CreateCommitment {
                event_id,
                workstream,
                ..
            } => {
                self.workstreams
                    .extend(workstream.map(WorkstreamId::as_uuid));
                Some((event_id.as_uuid(), LocalIdKind::Commitment.prefix()))
            }
            Operation::ChangeActionStatus { action_id, .. }
            | Operation::ChangeActionDue { action_id, .. } => {
                self.actions.push(action_id.as_uuid());
                None
            }
            Operation::ChangeCommitmentStatus { commitment_id, .. } => {
                self.commitments.push(commitment_id.as_uuid());
                None
            }
            _ => None,
        }
    }

    /// Locks the workstreams for share, then the actions and the commitments for update, each in the order of its IDs.
    pub async fn lock(&self, conn: &mut PgConnection, scope: OrgScope) -> Result<(), sqlx::Error> {
        let organization = scope.organization_id().as_uuid();
        sqlx::query_scalar!(
            "SELECT id FROM workstream WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR SHARE",
            organization,
            &self.workstreams,
        )
        .fetch_all(&mut *conn)
        .await?;
        sqlx::query_scalar!(
            "SELECT id FROM action WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
            organization,
            &self.actions,
        )
        .fetch_all(&mut *conn)
        .await?;
        sqlx::query_scalar!(
            "SELECT id FROM commitment WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
            organization,
            &self.commitments,
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(())
    }
}
