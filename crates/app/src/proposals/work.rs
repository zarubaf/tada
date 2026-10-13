//! The checks of the proposals that create or change work records and parties (ADR 0068, ADR 0069).
//!
//! The same rules as the direct commands apply: the owner of a work record is a contributor or a manager of its event,
//! a new record takes only an active workstream of its event, and a status changes only along its transitions.

use tada_domain::ids::{EventId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::proposals::{NewRecord, Operation, Proposal};
use tada_domain::work::{CommitmentStatus, FirmReason, WorkstreamName};

use super::ProposeStores;
use super::input::text_error_code;
use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::problem::FieldError;
use crate::store::StoreError;
use crate::work::is_possible_owner;
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

/// The field errors of the work proposals of a changeset, with paths from the changeset.
pub(super) async fn check_work(
    scope: OrgScope,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<Vec<FieldError>, StoreError> {
    let mut errors = Vec::new();
    for (index, proposal) in proposals.iter().enumerate() {
        let operation = |field: &str| format!("proposals/{index}/operation/{field}");
        let mut refuse =
            |field: String, code: &'static str| errors.push(FieldError::new(field, code));
        match &proposal.operation {
            Operation::CreateAction {
                event_id,
                owner,
                workstream,
                ..
            } => {
                if let Some(code) = owner_refusal(scope, *event_id, *owner, stores.identity).await?
                {
                    refuse(operation("owner"), code);
                }
                if let Some(code) = new_record_workstream_refusal(
                    scope,
                    *event_id,
                    *workstream,
                    proposals,
                    stores.workstreams,
                )
                .await?
                {
                    refuse(operation("workstream"), code);
                }
            }
            Operation::CreateCommitment {
                event_id,
                promisor,
                owner,
                workstream,
                ..
            } => {
                if let Some(code) = owner_refusal(scope, *event_id, *owner, stores.identity).await?
                {
                    refuse(operation("owner"), code);
                }
                if let Some(code) = new_record_workstream_refusal(
                    scope,
                    *event_id,
                    *workstream,
                    proposals,
                    stores.workstreams,
                )
                .await?
                {
                    refuse(operation("workstream"), code);
                }
                if !promisor_exists(scope, *promisor, proposals, stores).await? {
                    refuse(operation("promisor"), "unknown-record");
                }
            }
            Operation::CreateWorkstream {
                event_id,
                name,
                lead,
                ..
            } => {
                if let Some(code) = owner_refusal(scope, *event_id, *lead, stores.identity).await? {
                    refuse(operation("lead"), code);
                }
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
                    && let Err(error) = FirmReason::parse(proposal.reason.as_str())
                {
                    refuse(format!("proposals/{index}/reason"), text_error_code(error));
                }
            }
            _ => {}
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
    let created = workstream.is_some_and(|id| {
        proposals.iter().any(|proposal| {
            proposal.operation.event_id() == Some(event)
                && proposal.operation.new_record() == Some(NewRecord::Workstream(id))
        })
    });
    if created {
        return Ok(None);
    }
    workstream_refusal(scope, event, workstream, store).await
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

/// True if the promisor is a person or an institution of the organization, or a new one of the changeset.
/// `check_structure` checks that the proposal depends on the proposal that creates it.
async fn promisor_exists(
    scope: OrgScope,
    promisor: Party,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<bool, StoreError> {
    let created = proposals.iter().any(|proposal| {
        matches!(
            (proposal.operation.new_record(), promisor),
            (Some(NewRecord::Person(new)), Party::Person(id)) if new == id
        ) || matches!(
            (proposal.operation.new_record(), promisor),
            (Some(NewRecord::Institution(new)), Party::Institution(id)) if new == id
        )
    });
    if created {
        return Ok(true);
    }
    Ok(match promisor {
        Party::Person(id) => stores.parties.person(scope, id).await?.is_some(),
        Party::Institution(id) => stores.parties.institution(scope, id).await?.is_some(),
    })
}
