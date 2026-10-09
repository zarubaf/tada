//! The selection of an apply or a rejection: the proposals with their dependencies or dependents, in apply order.

use std::collections::{HashMap, HashSet};

use tada_domain::ids::ProposalId;
use tada_domain::proposals::Proposal;

use super::{ApplyError, ProposalStatus, ReviewRecord, finish, status};
use crate::problem::FieldError;
use crate::proposals::Changeset;

pub(super) fn proposal_status(results: &[ReviewRecord], id: ProposalId) -> ProposalStatus {
    let own: Vec<ReviewRecord> = results
        .iter()
        .filter(|record| record.proposal_id == id)
        .cloned()
        .collect();
    status(&own)
}

/// Checks that `ids` are proposals of the changeset. `field` names the list in the errors.
pub(super) fn selection(
    changeset: &Changeset,
    ids: &[ProposalId],
    field: &str,
) -> Result<HashSet<ProposalId>, ApplyError> {
    if ids.is_empty() {
        return Err(ApplyError::Invalid(vec![FieldError::new(
            field.to_owned(),
            "empty",
        )]));
    }
    let known: HashSet<ProposalId> = changeset.proposals.iter().map(|p| p.id).collect();
    finish(
        ids.iter()
            .enumerate()
            .filter(|(_, id)| !known.contains(id))
            .map(|(index, _)| FieldError::new(format!("{field}/{index}"), "unknown"))
            .collect(),
    )?;
    Ok(ids.iter().copied().collect())
}

/// The proposals of `selected` and all their dependencies.
pub(super) fn with_dependencies(
    proposals: &[Proposal],
    selected: &HashSet<ProposalId>,
) -> HashSet<ProposalId> {
    let depends_on: HashMap<ProposalId, &[ProposalId]> = proposals
        .iter()
        .map(|proposal| (proposal.id, proposal.depends_on.as_slice()))
        .collect();
    closure(selected, |id| {
        depends_on.get(&id).copied().unwrap_or_default().to_vec()
    })
}

/// The proposals to apply: the selected ones and their dependencies, without the dependencies that an earlier apply
/// accepted, for example the new event of a fact. Each of them must be open, else `invalid-transition`.
pub(super) fn to_apply(
    proposals: &[Proposal],
    given: &HashSet<ProposalId>,
    results: &[ReviewRecord],
) -> Result<HashSet<ProposalId>, ApplyError> {
    let mut selected = HashSet::new();
    for id in with_dependencies(proposals, given) {
        match proposal_status(results, id) {
            ProposalStatus::Open => {
                selected.insert(id);
            }
            ProposalStatus::Accepted | ProposalStatus::AcceptedWithEdit if !given.contains(&id) => {
            }
            _ => return Err(ApplyError::InvalidTransition),
        }
    }
    Ok(selected)
}

/// The proposals of `given` and all proposals that depend on them.
pub(super) fn with_dependents(
    proposals: &[Proposal],
    given: &HashSet<ProposalId>,
) -> Vec<ProposalId> {
    let mut dependents: HashMap<ProposalId, Vec<ProposalId>> = HashMap::new();
    for proposal in proposals {
        for dependency in &proposal.depends_on {
            dependents.entry(*dependency).or_default().push(proposal.id);
        }
    }
    let all = closure(given, |id| dependents.get(&id).cloned().unwrap_or_default());
    // In the order of the changeset, so the result does not depend on a hash order.
    proposals
        .iter()
        .map(|proposal| proposal.id)
        .filter(|id| all.contains(id))
        .collect()
}

pub(super) fn closure(
    start: &HashSet<ProposalId>,
    next: impl Fn(ProposalId) -> Vec<ProposalId>,
) -> HashSet<ProposalId> {
    let mut all = start.clone();
    let mut todo: Vec<ProposalId> = start.iter().copied().collect();
    while let Some(id) = todo.pop() {
        for other in next(id) {
            if all.insert(other) {
                todo.push(other);
            }
        }
    }
    all
}

/// The selected proposals in an order where each proposal follows its dependencies.
/// The dependencies of a stored changeset have no cycle (ADR 0050). Ties keep the order of the changeset.
pub(super) fn apply_order<'a>(
    proposals: &'a [Proposal],
    selected: &HashSet<ProposalId>,
) -> Vec<&'a Proposal> {
    let mut pending: Vec<&Proposal> = proposals
        .iter()
        .filter(|proposal| selected.contains(&proposal.id))
        .collect();
    let mut done = HashSet::new();
    let mut order = Vec::new();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|proposal| {
            // A dependency outside the selection was accepted by an earlier apply.
            let ready = proposal
                .depends_on
                .iter()
                .all(|id| done.contains(id) || !selected.contains(id));
            if ready {
                done.insert(proposal.id);
                order.push(*proposal);
            }
            !ready
        });
        assert!(
            pending.len() < before,
            "the dependencies of a changeset have no cycle"
        );
    }
    order
}
