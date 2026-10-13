//! The apply of the proposals that create or change work records and parties (ADR 0068, ADR 0069).
//!
//! Each write copies the passages of the proposal into `record_evidence` with the record version that it produced.
//! A new record takes the next readable number of its scope: `ACT`, `COM` and `MIL` in the event, `PER` and `INS` in
//! the organization.

use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{EventId, LocalIdKind, MilestoneId, SourceVersionId, WorkstreamId};
use tada_app::domain::parties::Party;
use tada_app::domain::proposals::Operation;
use tada_app::domain::records::MilestoneStatus;
use tada_app::domain::redaction::Redactable;
use tada_app::domain::work::{ActionStatus, CommitmentStatus, WorkstreamStatus, dates_in_order};
use tada_app::parties::{InstitutionFields, PersonFields};
use tada_app::records::RecordRef;
use tada_app::review::{ApplyPlan, ApplyStep, LocalRecord, NewLocalId, StepEvidence};
use tada_app::work::{
    ActionFields, CommitmentFields, MilestoneFields, NewActionRecord, NewCommitmentRecord,
    NewMilestoneRecord,
};
use tada_app::workstreams::Workstream;

use super::insert_review_text;
use crate::evidence::insert_evidence;
use crate::parties::{insert_institution, insert_person};
use crate::proposals::operation_to_json;
use crate::work::milestones::{insert_milestone, milestone_in, update_milestone};
use crate::work::{
    action_in, commitment_in, insert_action, insert_commitment, update_action, update_commitment,
};
use crate::workstreams::insert_workstream;

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
                &operation_to_json(&step.operation).to_string(),
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
        StepEvidence::Link(_) => {
            return Err(sqlx::Error::Protocol("a link writes no record".into()));
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
                // A proposal never links an account (ADR 0069).
                user_id: None,
            };
            let number = insert_person(conn, scope, *id, &fields, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Person(*id), number)?);
            (RecordRef::Person(*id), 1)
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
            (RecordRef::Institution(*id), 1)
        }
        Operation::CreateAction {
            id,
            event_id,
            title,
            description,
            owner,
            workstream,
            start_date,
            due_date,
            milestone,
        } => {
            let action = NewActionRecord {
                id: *id,
                event_id: *event_id,
                fields: ActionFields {
                    title: Redactable::Present(title.clone()),
                    description: description.clone(),
                    owner: *owner,
                    workstream_id: *workstream,
                    start_date: *start_date,
                    due_date: *due_date,
                    milestone_id: *milestone,
                    status: ActionStatus::Open,
                },
            };
            let number = insert_action(conn, scope, &action, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Action(*id), number)?);
            (RecordRef::Action(*id), 1)
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
                    text: Redactable::Present(text.clone()),
                    owner: *owner,
                    workstream_id: *workstream,
                    due_date: *due_date,
                    status: CommitmentStatus::initial(condition.as_ref()),
                    firm_reason: None,
                },
            };
            let number = insert_commitment(conn, scope, &commitment, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Commitment(*id), number)?);
            (RecordRef::Commitment(*id), 1)
        }
        Operation::CreateWorkstream {
            id,
            event_id,
            name,
            lead,
        } => {
            let workstream = Workstream {
                id: *id,
                event_id: *event_id,
                name: Redactable::Present(name.clone()),
                lead: *lead,
                status: WorkstreamStatus::Active,
                version: RecordVersion::FIRST,
            };
            insert_workstream(conn, scope, &workstream, plan.now).await?;
            // A workstream has no readable ID and keeps no evidence: the proposal holds it (ADR 0067).
            return Ok(written);
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
            (RecordRef::Action(*action_id), version)
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
            // The start date of the action is not after the new due date (ADR 0080); a refusal is a conflict.
            if !dates_in_order(fields.start_date, fields.due_date) {
                return Err(sqlx::Error::RowNotFound);
            }
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
            (RecordRef::Action(*action_id), version)
        }
        Operation::ChangeActionMilestone {
            event_id,
            action_id,
            milestone,
            expected_version,
        } => {
            let current = action_in(conn, scope, *event_id, *action_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            let fields = ActionFields {
                milestone_id: *milestone,
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
            (RecordRef::Action(*action_id), version)
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
            (RecordRef::Commitment(*commitment_id), version)
        }
        Operation::CreateMilestone {
            id,
            event_id,
            title,
            description,
            date,
            owner,
            workstream,
        } => {
            let milestone = NewMilestoneRecord {
                id: *id,
                event_id: *event_id,
                fields: MilestoneFields {
                    title: Redactable::Present(title.clone()),
                    description: description.clone(),
                    date: *date,
                    owner: *owner,
                    workstream_id: *workstream,
                    status: MilestoneStatus::Open,
                },
            };
            let number = insert_milestone(conn, scope, &milestone, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Milestone(*id), number)?);
            (RecordRef::Milestone(*id), 1)
        }
        Operation::ChangeMilestoneStatus {
            event_id,
            milestone_id,
            status,
            expected_version,
        } => {
            let current = milestone_in(conn, scope, *event_id, *milestone_id, plan.now)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            // As for an action: a refused transition of a changed record is a conflict.
            let status = current
                .fields
                .status
                .change_to(*status)
                .map_err(|_| sqlx::Error::RowNotFound)?;
            let fields = MilestoneFields {
                status,
                ..current.fields
            };
            let version = update_milestone(
                conn,
                scope,
                *event_id,
                *milestone_id,
                &fields,
                *expected_version,
                plan.now,
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
            (RecordRef::Milestone(*milestone_id), version)
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
            milestone,
            ..
        } => {
            let free = !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM action WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            free && takes_records(conn, scope, *event_id, *workstream, new_records).await?
                && takes_actions(conn, scope, *event_id, *milestone, new_records).await?
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
            free && promisor_exists
                && takes_records(conn, scope, *event_id, *workstream, new_records).await?
        }
        Operation::CreateWorkstream {
            id, event_id, name, ..
        } => {
            sqlx::query_scalar!(
                r#"SELECT NOT EXISTS (
                       SELECT 1 FROM workstream
                       WHERE id = $1 OR (event_id = $2 AND lower(name) = lower($3))
                   ) AS "free!""#,
                id.as_uuid(),
                event_id.as_uuid(),
                name.as_str(),
            )
            .fetch_one(&mut *conn)
            .await?
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
        }
        | Operation::ChangeActionMilestone {
            event_id,
            action_id,
            expected_version,
            ..
        } => {
            if let Operation::ChangeActionMilestone { milestone, .. } = operation
                && !takes_actions(conn, scope, *event_id, *milestone, new_records).await?
            {
                return Ok(false);
            }
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
        Operation::CreateMilestone {
            id,
            event_id,
            workstream,
            ..
        } => {
            let free = !sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM milestone WHERE id = $1) AS "exists!""#,
                id.as_uuid(),
            )
            .fetch_one(&mut *conn)
            .await?;
            free && takes_records(conn, scope, *event_id, *workstream, new_records).await?
        }
        Operation::ChangeMilestoneStatus {
            event_id,
            milestone_id,
            expected_version,
            ..
        } => {
            let current = match versions.get(&milestone_id.as_uuid()) {
                Some(version) => Some(*version),
                None => {
                    sqlx::query_scalar!(
                        "SELECT version FROM milestone WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                        organization,
                        event_id.as_uuid(),
                        milestone_id.as_uuid(),
                    )
                    .fetch_optional(&mut *conn)
                    .await?
                }
            };
            versions.insert(milestone_id.as_uuid(), expected_version.get() + 1);
            current == Some(expected_version.get())
        }
        _ => false,
    })
}

/// True if a new record of the event can have the workstream: none, a new workstream of an earlier step of the plan,
/// or an active workstream of the event (ADR 0067).
/// `lock_targets` locked the workstream, so it cannot close before the commit.
async fn takes_records(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    workstream: Option<WorkstreamId>,
    new_records: &std::collections::HashSet<Uuid>,
) -> Result<bool, sqlx::Error> {
    let Some(workstream) = workstream else {
        return Ok(true);
    };
    if new_records.contains(&workstream.as_uuid()) {
        return Ok(true);
    }
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

/// True if a new action of the event, or an action that changes its milestone, can name `milestone` (ADR 0080):
/// none, a new milestone of the plan, or a milestone of the event that is not canceled.
/// `lock_targets` locked the milestone, so it cannot be canceled before the commit.
async fn takes_actions(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    milestone: Option<MilestoneId>,
    new_records: &std::collections::HashSet<Uuid>,
) -> Result<bool, sqlx::Error> {
    let Some(milestone) = milestone else {
        return Ok(true);
    };
    if new_records.contains(&milestone.as_uuid()) {
        return Ok(true);
    }
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM milestone
               WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND status <> 'canceled'
           ) AS "usable!""#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        milestone.as_uuid(),
    )
    .fetch_one(&mut *conn)
    .await
}

pub(super) async fn party_exists(
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
    pub milestones: Vec<Uuid>,
    /// The milestones that actions of the plan name. They stay open for the transaction.
    pub named_milestones: Vec<Uuid>,
}

impl WorkLocks {
    /// The named milestones that no step changes. A changed milestone takes only the update lock: a share lock first
    /// would be upgraded later, and two applies that upgrade the same row deadlock.
    fn shared_milestones(&self) -> Vec<Uuid> {
        self.named_milestones
            .iter()
            .filter(|id| !self.milestones.contains(id))
            .copied()
            .collect()
    }

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
                milestone,
                ..
            } => {
                self.workstreams
                    .extend(workstream.map(WorkstreamId::as_uuid));
                self.named_milestones
                    .extend(milestone.map(MilestoneId::as_uuid));
                Some((event_id.as_uuid(), LocalIdKind::Action.prefix()))
            }
            Operation::ChangeActionMilestone {
                action_id,
                milestone,
                ..
            } => {
                self.actions.push(action_id.as_uuid());
                self.named_milestones
                    .extend(milestone.map(MilestoneId::as_uuid));
                None
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
            Operation::CreateMilestone {
                event_id,
                workstream,
                ..
            } => {
                self.workstreams
                    .extend(workstream.map(WorkstreamId::as_uuid));
                Some((event_id.as_uuid(), LocalIdKind::Milestone.prefix()))
            }
            Operation::ChangeMilestoneStatus { milestone_id, .. } => {
                self.milestones.push(milestone_id.as_uuid());
                None
            }
            _ => None,
        }
    }

    /// Locks the workstreams and the named milestones for share, then the actions, the commitments and the changed
    /// milestones for update, each in the order of its IDs.
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
            "SELECT id FROM milestone WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR SHARE",
            organization,
            &self.shared_milestones(),
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
        sqlx::query_scalar!(
            "SELECT id FROM milestone WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
            organization,
            &self.milestones,
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_milestone_is_not_share_locked() {
        let (changed, named) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let locks = WorkLocks {
            milestones: vec![changed],
            named_milestones: vec![changed, named],
            ..WorkLocks::default()
        };
        assert_eq!(locks.shared_milestones(), [named]);
    }
}
