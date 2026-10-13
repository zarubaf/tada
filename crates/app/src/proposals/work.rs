//! The checks of the proposals that create or change work records and parties (ADR 0068, ADR 0069).
//!
//! The same rules as the direct commands apply: the owner of a work record is a contributor or a manager of its event,
//! a new record takes only an active workstream of its event, and a status changes only along its transitions.

use jiff::Timestamp;
use tada_domain::ids::{DecisionId, EventId, MilestoneId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::proposals::{NewRecord, NewWorkRecord, Operation, Proposal};
use tada_domain::records::DecisionStatus;
use tada_domain::work::{CommitmentStatus, RecordReason, WorkstreamName, dates_in_order};

use super::ProposeStores;
use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::problem::FieldError;
use crate::records::{START_AFTER_DUE, text_error_code};
use crate::store::StoreError;
use crate::work::{is_possible_owner, milestone_refusal};
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

/// The field errors of the work proposals of a changeset, with paths from the changeset.
pub(super) async fn check_work(
    scope: OrgScope,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
    now: Timestamp,
) -> Result<Vec<FieldError>, StoreError> {
    let mut errors = Vec::new();
    for (index, proposal) in proposals.iter().enumerate() {
        let operation = |field: &str| format!("proposals/{index}/operation/{field}");
        let mut refuse =
            |field: String, code: &'static str| errors.push(FieldError::new(field, code));
        if let Operation::CreateAction {
            event_id,
            milestone: Some(milestone),
            ..
        }
        | Operation::ChangeActionMilestone {
            event_id,
            milestone: Some(milestone),
            ..
        } = &proposal.operation
            && let Some(code) =
                milestone_refusal_in(scope, *event_id, *milestone, proposals, stores, now).await?
        {
            refuse(operation("milestone"), code);
        }
        if let Some(NewWorkRecord {
            event_id,
            owner,
            workstream,
        }) = proposal.operation.new_work_record()
        {
            // The owner of a new workstream is its lead.
            let owner_field = if matches!(proposal.operation, Operation::CreateWorkstream { .. }) {
                "lead"
            } else {
                "owner"
            };
            if let Some(code) = owner_refusal(scope, event_id, owner, stores.identity).await? {
                refuse(operation(owner_field), code);
            }
            if let Some(code) = new_record_workstream_refusal(
                scope,
                event_id,
                workstream,
                proposals,
                stores.workstreams,
            )
            .await?
            {
                refuse(operation("workstream"), code);
            }
        }
        match &proposal.operation {
            Operation::CreateAction { .. } | Operation::CreateMilestone { .. } => {}
            Operation::CreateCommitment { promisor, .. } => {
                if !party_exists(scope, *promisor, proposals, stores).await? {
                    refuse(operation("promisor"), "unknown-record");
                }
            }
            Operation::CreateDecision { decided_by, .. } => {
                for (position, party) in decided_by.with_input_positions() {
                    if !party_exists(scope, *party, proposals, stores).await? {
                        refuse(
                            operation(&format!("decided_by/{position}")),
                            "unknown-record",
                        );
                    }
                }
            }
            Operation::SupersedeDecision {
                event_id,
                decision_id,
                successor,
                ..
            } => {
                match stores
                    .decisions
                    .decision(scope, *event_id, *decision_id)
                    .await?
                {
                    None => refuse(operation("decision_id"), "unknown-record"),
                    Some(current) if current.state.status() != DecisionStatus::InForce => {
                        refuse(operation("decision_id"), "invalid-transition");
                    }
                    Some(_) => {}
                }
                if !successor_usable(
                    scope,
                    *event_id,
                    *decision_id,
                    *successor,
                    proposals,
                    stores,
                )
                .await?
                {
                    refuse(operation("successor"), "unknown-record");
                }
            }
            Operation::RevokeDecision {
                event_id,
                decision_id,
                ..
            } => match stores
                .decisions
                .decision(scope, *event_id, *decision_id)
                .await?
            {
                None => refuse(operation("decision_id"), "unknown-record"),
                Some(current) if current.state.status() != DecisionStatus::InForce => {
                    refuse(operation("decision_id"), "invalid-transition");
                }
                Some(_) => {}
            },
            Operation::CreateWorkstream { event_id, name, .. } => {
                if name_taken(scope, *event_id, name, proposals[..index].iter(), stores).await? {
                    refuse(operation("name"), "taken");
                }
            }
            Operation::ChangeActionStatus {
                event_id,
                action_id,
                status,
                ..
            } => match stores.work.action(scope, *event_id, *action_id).await? {
                None => refuse(operation("action_id"), "unknown-record"),
                Some(current) if current.fields.status.change_to(*status).is_err() => {
                    refuse(operation("status"), "invalid-transition");
                }
                Some(_) => {}
            },
            Operation::ChangeActionDue {
                event_id,
                action_id,
                due_date,
                ..
            } => match stores.work.action(scope, *event_id, *action_id).await? {
                None => refuse(operation("action_id"), "unknown-record"),
                // The start date of the action is not after the new due date (ADR 0080).
                Some(current) if !dates_in_order(current.fields.start_date, *due_date) => {
                    refuse(operation("due_date"), START_AFTER_DUE);
                }
                Some(_) => {}
            },
            Operation::ChangeActionMilestone {
                event_id,
                action_id,
                ..
            } => {
                if stores
                    .work
                    .action(scope, *event_id, *action_id)
                    .await?
                    .is_none()
                {
                    refuse(operation("action_id"), "unknown-record");
                }
            }
            Operation::ChangeCommitmentStatus {
                event_id,
                commitment_id,
                status,
                ..
            } => {
                match stores
                    .work
                    .commitment(scope, *event_id, *commitment_id)
                    .await?
                {
                    None => refuse(operation("commitment_id"), "unknown-record"),
                    Some(current) if current.fields.status.change_to(*status).is_err() => {
                        refuse(operation("status"), "invalid-transition");
                    }
                    Some(_) => {}
                }
                // The reason of the proposal becomes the reason of the firm commitment (ADR 0068).
                if *status == CommitmentStatus::Firm
                    && let Err(error) = RecordReason::parse(proposal.reason.as_str())
                {
                    refuse(format!("proposals/{index}/reason"), text_error_code(error));
                }
            }
            Operation::ChangeMilestoneStatus {
                event_id,
                milestone_id,
                status,
                ..
            } => match stores
                .milestones
                .milestone(scope, *event_id, *milestone_id, now)
                .await?
            {
                None => refuse(operation("milestone_id"), "unknown-record"),
                Some(current) if current.fields.status.change_to(*status).is_err() => {
                    refuse(operation("status"), "invalid-transition");
                }
                Some(_) => {}
            },
            // These operations create or change no work record: other checks of the changeset cover them.
            Operation::CreateEvent { .. }
            | Operation::SetFact { .. }
            | Operation::AddFieldDefinition { .. }
            | Operation::AddChoiceValue { .. }
            | Operation::DeprecateField { .. }
            | Operation::CreateOpenQuestion { .. }
            | Operation::CreateDocumentDraft { .. }
            | Operation::CreatePerson { .. }
            | Operation::CreateInstitution { .. } => {}
        }
    }
    Ok(errors)
}

/// The code that refuses `owner` as the owner of a new work record of the event, or `None`.
pub(crate) async fn owner_refusal(
    scope: OrgScope,
    event: EventId,
    owner: UserId,
    identity: &dyn IdentityStore,
) -> Result<Option<&'static str>, StoreError> {
    Ok((!is_possible_owner(scope, event, owner, identity).await?).then_some("unknown-member"))
}

/// The code that refuses `workstream` for a new work record of the event (`unknown-record` or `closed`), or `None`.
pub(crate) async fn workstream_refusal(
    scope: OrgScope,
    event: EventId,
    workstream: Option<WorkstreamId>,
    store: &dyn WorkstreamStore,
) -> Result<Option<&'static str>, StoreError> {
    let Some(workstream) = workstream else {
        return Ok(None);
    };
    match active_workstream(store, scope, event, workstream).await {
        Ok(_) => Ok(None),
        Err(ActiveWorkstreamError::Refused(error)) => Ok(Some(error.code)),
        Err(ActiveWorkstreamError::Store(error)) => Err(error),
    }
}

/// Like `workstream_refusal`, but a workstream that a proposal of the changeset creates in the event is fine.
/// `check_structure` checks that the proposal depends on the proposal that creates it.
/// The apply checks that the workstream is active when it comes.
async fn new_record_workstream_refusal(
    scope: OrgScope,
    event: EventId,
    workstream: Option<WorkstreamId>,
    proposals: &[Proposal],
    store: &dyn WorkstreamStore,
) -> Result<Option<&'static str>, StoreError> {
    if workstream.is_some_and(|id| created_in(proposals, Some(event), NewRecord::Workstream(id))) {
        return Ok(None);
    }
    workstream_refusal(scope, event, workstream, store).await
}

/// True if a proposal of the changeset creates `record` in `event`. A record of another event of the same
/// organization changeset does not count.
fn created_in(proposals: &[Proposal], event: Option<EventId>, record: NewRecord) -> bool {
    proposals.iter().any(|proposal| {
        proposal.operation.event_id() == event && proposal.operation.new_record() == Some(record)
    })
}

/// True if a workstream of the event, or one of the `earlier` proposals, has the name, whatever its case.
async fn name_taken<'a>(
    scope: OrgScope,
    event: EventId,
    name: &WorkstreamName,
    earlier: impl Iterator<Item = &'a Proposal>,
    stores: ProposeStores<'_>,
) -> Result<bool, StoreError> {
    let wanted = name.as_str().to_lowercase();
    let same = |other: &str| other.to_lowercase() == wanted;
    let proposed = earlier.into_iter().any(|proposal| {
        matches!(
            &proposal.operation,
            Operation::CreateWorkstream { event_id, name, .. }
                if *event_id == event && same(name.as_str())
        )
    });
    if proposed {
        return Ok(true);
    }
    Ok(stores
        .workstreams
        .list(scope, event)
        .await?
        .iter()
        .any(|workstream| {
            workstream
                .name
                .as_present()
                .is_some_and(|n| same(n.as_str()))
        }))
}

/// The code that refuses `milestone` for an action of the event (`unknown-record`), or `None`: a new milestone of
/// the event in the changeset passes; `check_structure` checks that the proposal depends on the proposal that creates it.
async fn milestone_refusal_in(
    scope: OrgScope,
    event: EventId,
    milestone: MilestoneId,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
    now: Timestamp,
) -> Result<Option<&'static str>, StoreError> {
    if created_in(proposals, Some(event), NewRecord::Milestone(milestone)) {
        return Ok(None);
    }
    milestone_refusal(scope, event, milestone, stores.milestones, now).await
}

/// True if the successor of a superseded decision can take its place (ADR 0080): another decision of the event in
/// force, or a new decision of the event in the changeset. A decision of another event, the decision itself, an
/// unknown one and one that ended are refused alike. `check_structure` checks that the proposal depends on the
/// proposal that creates a new successor; the apply checks the successor again.
async fn successor_usable(
    scope: OrgScope,
    event: EventId,
    old: DecisionId,
    successor: DecisionId,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<bool, StoreError> {
    if successor == old {
        return Ok(false);
    }
    if created_in(proposals, Some(event), NewRecord::Decision(successor)) {
        return Ok(true);
    }
    Ok(stores
        .decisions
        .decision(scope, event, successor)
        .await?
        .is_some_and(|decision| decision.state.status() == DecisionStatus::InForce))
}

/// True if the party is a person or an institution of the organization, or a new one of the changeset: the promisor
/// of a commitment or a party of a decision. `check_structure` checks that the proposal depends on the proposal that
/// creates it.
async fn party_exists(
    scope: OrgScope,
    party: Party,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<bool, StoreError> {
    let created = proposals.iter().any(|proposal| {
        matches!(
            (proposal.operation.new_record(), party),
            (Some(NewRecord::Person(new)), Party::Person(id)) if new == id
        ) || matches!(
            (proposal.operation.new_record(), party),
            (Some(NewRecord::Institution(new)), Party::Institution(id)) if new == id
        )
    });
    if created {
        return Ok(true);
    }
    Ok(match party {
        Party::Person(id) => stores.parties.person(scope, id).await?.is_some(),
        Party::Institution(id) => stores.parties.institution(scope, id).await?.is_some(),
    })
}
