//! The apply of the proposals that create or change work records and parties (ADR 0068, ADR 0069).
//!
//! Each write copies the passages of the proposal into `record_evidence` with the record version that it produced.
//! A new record takes the next readable number of its scope: `ACT`, `COM`, `MIL`, `DEC` and `RSK` in the event, `PER` and
//! `INS` in the organization.

use std::collections::HashMap;

use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{
    DecisionId, EventId, LocalIdKind, MilestoneId, SourceVersionId, WorkstreamId,
};
use tada_app::domain::parties::Party;
use tada_app::domain::proposals::{NewRecord, Operation};
use tada_app::domain::records::{DecisionChange, DecisionStatus, MilestoneStatus, RecordKind};
use tada_app::domain::redaction::Redactable;
use tada_app::domain::work::{ActionStatus, CommitmentStatus, WorkstreamStatus, dates_in_order};
use tada_app::parties::{InstitutionFields, PersonFields};
use tada_app::records::RecordRef;
use tada_app::review::{ApplyPlan, ApplyStep, LocalRecord, NewLocalId, StepEvidence};
use tada_app::work::{
    ActionFields, CommitmentFields, DecisionFields, MilestoneFields, NewActionRecord,
    NewCommitmentRecord, NewDecisionRecord, NewMilestoneRecord,
};
use tada_app::workstreams::Workstream;

use super::insert_review_text;
use crate::evidence::{ids_of, insert_evidence};
use crate::parties::{insert_institution, insert_person};
use crate::proposals::operation_to_json;
use crate::work::decisions::{decision_in, end_decision, insert_decision};
use crate::work::milestones::{insert_milestone, milestone_in, update_milestone};
use crate::work::risks::{RiskEdit, change_risk_in_plan, create_risk_in_plan};
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
            // The start date of the action is not after the new due date (ADR 0081); a refusal is a conflict.
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
        Operation::CreateDecision {
            id,
            event_id,
            wording,
            decided_on,
            decided_by,
            owner,
            workstream,
        } => {
            let decision = NewDecisionRecord {
                id: *id,
                event_id: *event_id,
                wording: wording.clone(),
                decided_on: *decided_on,
                parties: decided_by.clone(),
                fields: DecisionFields {
                    owner: *owner,
                    workstream_id: *workstream,
                },
            };
            let number = insert_decision(conn, scope, &decision, plan.now).await?;
            written.local_id = Some(local_id(LocalRecord::Decision(*id), number)?);
            (RecordRef::Decision(*id), 1)
        }
        Operation::SupersedeDecision {
            event_id,
            decision_id,
            successor,
            expected_version,
        } => {
            // The successor must still be in force when the old decision ends: an earlier step of the plan can end
            // it, and `lock_targets` locked it against the other writers. A refusal is a conflict.
            let usable = *successor != *decision_id
                && decision_in(conn, scope, *event_id, *successor)
                    .await?
                    .is_some_and(|decision| decision.state.status() == DecisionStatus::InForce);
            if !usable {
                return Err(sqlx::Error::RowNotFound);
            }
            let change = DecisionChange::Supersede {
                successor: *successor,
            };
            let version = end_of_decision(
                conn,
                scope,
                plan,
                *event_id,
                *decision_id,
                &change,
                *expected_version,
            )
            .await?;
            (RecordRef::Decision(*decision_id), version)
        }
        Operation::RevokeDecision {
            event_id,
            decision_id,
            reason,
            expected_version,
        } => {
            let change = DecisionChange::Revoke {
                reason: reason.clone(),
            };
            let version = end_of_decision(
                conn,
                scope,
                plan,
                *event_id,
                *decision_id,
                &change,
                *expected_version,
            )
            .await?;
            (RecordRef::Decision(*decision_id), version)
        }
        Operation::CreateRisk {
            id,
            event_id,
            title,
            description,
            rating,
            mitigation,
            owner,
            workstream,
        } => {
            let number = create_risk_in_plan(
                conn,
                scope,
                (*id, *event_id),
                title,
                description.as_ref(),
                *rating,
                mitigation.as_ref(),
                (*owner, *workstream),
                plan.now,
            )
            .await?;
            written.local_id = Some(local_id(LocalRecord::Risk(*id), number)?);
            (RecordRef::Risk(*id), 1)
        }
        Operation::ChangeRiskStatus { .. } | Operation::ChangeRiskRating { .. } => {
            let (event_id, risk_id, edit, expected_version) = RiskEdit::of(&step.operation)
                .ok_or_else(|| sqlx::Error::Protocol("the step changes no risk".into()))?;
            let version = change_risk_in_plan(
                conn,
                scope,
                event_id,
                risk_id,
                edit,
                expected_version,
                plan.now,
            )
            .await?;
            (RecordRef::Risk(risk_id), version)
        }
        Operation::CreateEvent { .. }
        | Operation::SetFact { .. }
        | Operation::AddFieldDefinition { .. }
        | Operation::AddChoiceValue { .. }
        | Operation::DeprecateField { .. }
        | Operation::CreateOpenQuestion { .. }
        | Operation::CreateDocumentDraft { .. } => {
            return Err(sqlx::Error::Protocol(
                "the step does not write a work record".into(),
            ));
        }
    };
    insert_evidence(conn, scope, record, version, step.proposal_id, &evidence).await?;
    Ok(written)
}

/// Ends a decision of the event with `change` and returns its new version. As for an action, a refused transition
/// of a changed decision and another version are conflicts.
async fn end_of_decision(
    conn: &mut PgConnection,
    scope: OrgScope,
    plan: &ApplyPlan,
    event: EventId,
    id: DecisionId,
    change: &DecisionChange,
    expected: RecordVersion,
) -> Result<i64, sqlx::Error> {
    let current = decision_in(conn, scope, event, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    current
        .state
        .status()
        .apply(change)
        .map_err(|_| sqlx::Error::RowNotFound)?;
    end_decision(conn, scope, event, id, change, expected, plan.now)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

fn local_id(record: LocalRecord, number: i64) -> Result<NewLocalId, sqlx::Error> {
    Ok(NewLocalId {
        record,
        local_number: u64::try_from(number)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
    })
}

/// The records that the steps of a plan create, each with its event. A person or an institution has no event.
pub(super) type NewRecords = HashMap<Uuid, Option<EventId>>;

/// The version check of `check_versions` for one work step: a new record whose ID is free and whose workstream
/// and promisor can take it, or a changed record with the expected version.
/// `new_records` holds the records that earlier steps of the plan create; `versions` the versions that earlier
/// steps of the plan give.
pub(super) async fn matches(
    conn: &mut PgConnection,
    scope: OrgScope,
    operation: &Operation,
    new_records: &NewRecords,
    versions: &mut HashMap<Uuid, i64>,
) -> Result<bool, sqlx::Error> {
    if let Some(record) = operation.new_record()
        && !id_free(conn, record).await?
    {
        return Ok(false);
    }
    let fits = match operation {
        Operation::CreatePerson { .. } | Operation::CreateInstitution { .. } => true,
        Operation::CreateAction {
            event_id,
            workstream,
            milestone,
            ..
        } => {
            takes_records(conn, scope, *event_id, *workstream, new_records).await?
                && takes_actions(conn, scope, *event_id, *milestone, new_records).await?
        }
        Operation::CreateCommitment {
            event_id,
            workstream,
            promisor,
            ..
        } => {
            let promisor_exists = new_records.contains_key(&promisor.as_uuid())
                || party_exists(conn, scope, *promisor).await?;
            promisor_exists
                && takes_records(conn, scope, *event_id, *workstream, new_records).await?
        }
        Operation::CreateWorkstream { event_id, name, .. } => {
            sqlx::query_scalar!(
                r#"SELECT NOT EXISTS (
                       SELECT 1 FROM workstream WHERE event_id = $1 AND lower(name) = lower($2)
                   ) AS "free!""#,
                event_id.as_uuid(),
                name.as_str(),
            )
            .fetch_one(&mut *conn)
            .await?
        }
        Operation::CreateMilestone {
            event_id,
            workstream,
            ..
        } => takes_records(conn, scope, *event_id, *workstream, new_records).await?,
        Operation::ChangeActionMilestone {
            event_id,
            milestone,
            ..
        } => takes_actions(conn, scope, *event_id, *milestone, new_records).await?,
        Operation::CreateDecision {
            event_id,
            workstream,
            decided_by,
            ..
        } => {
            let mut parties_exist = true;
            for party in decided_by.as_slice() {
                parties_exist &= new_records.contains_key(&party.as_uuid())
                    || party_exists(conn, scope, *party).await?;
            }
            parties_exist && takes_records(conn, scope, *event_id, *workstream, new_records).await?
        }
        Operation::SupersedeDecision {
            event_id,
            decision_id,
            successor,
            ..
        } => {
            successor != decision_id
                && takes_successor(conn, scope, *event_id, *successor, new_records).await?
        }
        Operation::CreateRisk {
            event_id,
            workstream,
            ..
        } => takes_records(conn, scope, *event_id, *workstream, new_records).await?,
        Operation::RevokeDecision { .. }
        | Operation::ChangeActionStatus { .. }
        | Operation::ChangeActionDue { .. }
        | Operation::ChangeCommitmentStatus { .. }
        | Operation::ChangeMilestoneStatus { .. }
        | Operation::ChangeRiskStatus { .. }
        | Operation::ChangeRiskRating { .. } => true,
        Operation::CreateEvent { .. }
        | Operation::SetFact { .. }
        | Operation::AddFieldDefinition { .. }
        | Operation::AddChoiceValue { .. }
        | Operation::DeprecateField { .. }
        | Operation::CreateOpenQuestion { .. }
        | Operation::CreateDocumentDraft { .. } => false,
    };
    if !fits {
        return Ok(false);
    }
    // A record that an earlier step of the plan changes counts with its new version.
    let (Some(record), Some(expected), Some(event)) = (
        operation.changed_record(),
        operation.expected_version(),
        operation.event_id(),
    ) else {
        return Ok(true);
    };
    let current = match versions.get(&record.as_uuid()) {
        Some(version) => Some(*version),
        None => record_version(conn, scope, event, record).await?,
    };
    versions.insert(record.as_uuid(), expected.get() + 1);
    Ok(current == Some(expected.get()))
}

/// True if no record has the ID of the new record. IDs are unique in the whole installation (ADR 0038).
pub(super) async fn id_free(
    conn: &mut PgConnection,
    record: NewRecord,
) -> Result<bool, sqlx::Error> {
    let id = record.as_uuid();
    let taken = match record {
        NewRecord::Event(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM event WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Field(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM field_definition WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::OpenQuestion(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM open_question WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Document(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM document WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Person(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM person WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Institution(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM institution WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Action(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM action WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Commitment(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM commitment WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Workstream(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM workstream WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Milestone(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM milestone WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Decision(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM decision WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
        NewRecord::Risk(_) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM risk WHERE id = $1) AS "exists!""#,
                id,
            )
            .fetch_one(&mut *conn)
            .await?
        }
    };
    Ok(!taken)
}

/// The current version of an existing record of the event, or `None`.
/// A person or an institution belongs to no event, and no proposal changes one.
async fn record_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    record: RecordRef,
) -> Result<Option<i64>, sqlx::Error> {
    let (organization, event, id) = (
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        record.as_uuid(),
    );
    match record.kind() {
        RecordKind::Action => {
            sqlx::query_scalar!(
                "SELECT version FROM action WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        RecordKind::Commitment => {
            sqlx::query_scalar!(
                "SELECT version FROM commitment WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        RecordKind::Milestone => {
            sqlx::query_scalar!(
                "SELECT version FROM milestone WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        RecordKind::Decision => {
            sqlx::query_scalar!(
                "SELECT version FROM decision WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        RecordKind::Risk => {
            sqlx::query_scalar!(
                "SELECT version FROM risk WHERE organization_id = $1 AND event_id = $2 AND id = $3",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        RecordKind::Person | RecordKind::Institution => Err(sqlx::Error::Protocol(
            "no proposal changes a person or an institution".into(),
        )),
    }
}

/// True if a new record of the event can have the workstream: none, a new workstream of the event in an earlier step
/// of the plan,
/// or an active workstream of the event (ADR 0067).
/// `lock_targets` locked the workstream, so it cannot close before the commit.
async fn takes_records(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    workstream: Option<WorkstreamId>,
    new_records: &NewRecords,
) -> Result<bool, sqlx::Error> {
    let Some(workstream) = workstream else {
        return Ok(true);
    };
    if new_records.get(&workstream.as_uuid()) == Some(&Some(event)) {
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

/// True if a new action of the event, or an action that changes its milestone, can name `milestone` (ADR 0081):
/// none, a new milestone of the event in the plan, or a milestone of the event that is not canceled.
/// `lock_targets` locked the milestone, so it cannot be canceled before the commit.
async fn takes_actions(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    milestone: Option<MilestoneId>,
    new_records: &NewRecords,
) -> Result<bool, sqlx::Error> {
    let Some(milestone) = milestone else {
        return Ok(true);
    };
    if new_records.get(&milestone.as_uuid()) == Some(&Some(event)) {
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

/// True if a superseded decision of the event can name `successor` (ADR 0081): a new decision of the event in the
/// plan, or a decision of the event in force. `lock_targets` locked the successor, so it cannot end before the commit.
async fn takes_successor(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    successor: DecisionId,
    new_records: &NewRecords,
) -> Result<bool, sqlx::Error> {
    if new_records.get(&successor.as_uuid()) == Some(&Some(event)) {
        return Ok(true);
    }
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM decision
               WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND status = 'in-force'
           ) AS "in_force!""#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        successor.as_uuid(),
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

/// The rows that `lock_targets` locks for the work steps of a plan.
#[derive(Debug, Default)]
pub(super) struct WorkLocks {
    pub workstreams: Vec<Uuid>,
    /// The existing records that the steps change.
    pub changed: Vec<RecordRef>,
    /// The milestones that actions of the plan name. They stay open for the transaction.
    pub named_milestones: Vec<Uuid>,
    /// The successors that superseded decisions of the plan name. They stay in force for the transaction.
    pub successors: Vec<Uuid>,
}

impl WorkLocks {
    /// The named milestones that no step changes. A changed milestone takes only the update lock: a share lock first
    /// would be upgraded later, and two applies that upgrade the same row deadlock.
    fn shared_milestones(&self) -> Vec<Uuid> {
        let changed = ids_of(&self.changed, RecordKind::Milestone);
        self.named_milestones
            .iter()
            .filter(|id| !changed.contains(id))
            .copied()
            .collect()
    }

    /// Adds the targets of one step. Returns the counter `(scope, kind)` that a new record moves.
    pub fn add(&mut self, scope: OrgScope, operation: &Operation) -> Option<(Uuid, &'static str)> {
        self.changed.extend(operation.changed_record());
        if let Some(workstream) = operation.workstream() {
            self.workstreams.push(workstream.as_uuid());
        }
        let organization = scope.organization_id().as_uuid();
        match operation {
            Operation::CreatePerson { .. } => Some((organization, LocalIdKind::Person.prefix())),
            Operation::CreateInstitution { .. } => {
                Some((organization, LocalIdKind::Institution.prefix()))
            }
            Operation::CreateAction {
                event_id,
                milestone,
                ..
            } => {
                self.named_milestones
                    .extend(milestone.map(MilestoneId::as_uuid));
                Some((event_id.as_uuid(), LocalIdKind::Action.prefix()))
            }
            Operation::ChangeActionMilestone { milestone, .. } => {
                self.named_milestones
                    .extend(milestone.map(MilestoneId::as_uuid));
                None
            }
            Operation::CreateCommitment { event_id, .. } => {
                Some((event_id.as_uuid(), LocalIdKind::Commitment.prefix()))
            }
            Operation::CreateMilestone { event_id, .. } => {
                Some((event_id.as_uuid(), LocalIdKind::Milestone.prefix()))
            }
            Operation::CreateDecision { event_id, .. } => {
                Some((event_id.as_uuid(), LocalIdKind::Decision.prefix()))
            }
            Operation::SupersedeDecision { successor, .. } => {
                self.successors.push(successor.as_uuid());
                None
            }
            Operation::CreateRisk { event_id, .. } => {
                Some((event_id.as_uuid(), LocalIdKind::Risk.prefix()))
            }
            Operation::RevokeDecision { .. }
            | Operation::ChangeRiskStatus { .. }
            | Operation::ChangeRiskRating { .. }
            | Operation::ChangeActionStatus { .. }
            | Operation::ChangeActionDue { .. }
            | Operation::ChangeCommitmentStatus { .. }
            | Operation::ChangeMilestoneStatus { .. }
            | Operation::CreateWorkstream { .. }
            | Operation::CreateEvent { .. }
            | Operation::SetFact { .. }
            | Operation::AddFieldDefinition { .. }
            | Operation::AddChoiceValue { .. }
            | Operation::DeprecateField { .. }
            | Operation::CreateOpenQuestion { .. }
            | Operation::CreateDocumentDraft { .. } => None,
        }
    }

    /// Locks the workstreams and the named milestones for share, then the changed records for update:
    /// kind by kind, each kind in the order of its IDs. The successors of decisions take the update lock together with
    /// the changed decisions, in one order: two applies that supersede decisions with each other then wait, and do not
    /// deadlock on a share lock that each would have to upgrade.
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
        for kind in RecordKind::ALL {
            let mut ids = ids_of(&self.changed, kind);
            if kind == RecordKind::Decision {
                ids.extend(&self.successors);
            }
            if !ids.is_empty() {
                lock_for_update(conn, organization, kind, &ids).await?;
            }
        }
        Ok(())
    }
}

/// Locks the records of `kind` with the IDs for update, in the order of the IDs.
async fn lock_for_update(
    conn: &mut PgConnection,
    organization: Uuid,
    kind: RecordKind,
    ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    match kind {
        RecordKind::Action => {
            sqlx::query_scalar!(
                "SELECT id FROM action WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Commitment => {
            sqlx::query_scalar!(
                "SELECT id FROM commitment WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Person => {
            sqlx::query_scalar!(
                "SELECT id FROM person WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Institution => {
            sqlx::query_scalar!(
                "SELECT id FROM institution WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Milestone => {
            sqlx::query_scalar!(
                "SELECT id FROM milestone WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Decision => {
            sqlx::query_scalar!(
                "SELECT id FROM decision WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
        RecordKind::Risk => {
            sqlx::query_scalar!(
                "SELECT id FROM risk WHERE organization_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
                organization,
                ids,
            )
            .fetch_all(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_milestone_is_not_share_locked() {
        let (changed, named) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let locks = WorkLocks {
            changed: vec![RecordRef::Milestone(MilestoneId::from_uuid(changed))],
            named_milestones: vec![changed, named],
            ..WorkLocks::default()
        };
        assert_eq!(locks.shared_milestones(), [named]);
    }
}
