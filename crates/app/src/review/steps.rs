//! The steps of an apply: the operation, the evidence and the audit events of each accepted proposal.

use tada_domain::proposals::{Operation, Proposal};
use tada_domain::work::{CommitmentStatus, FirmReason};

use super::checks::{Edited, Edits};
use super::links::Links;
use super::{ApplyError, ApplyStep, ProposalStatus, StepEvidence};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::MemberCaller;
use crate::problem::FieldError;
use crate::proposals::text_error_code;

/// The step of one proposal. A change to firm needs a valid reason: the commitment keeps it (ADR 0068).
/// A linked proposal creates no record; each other proposal uses the linked records (ADR 0069).
pub(super) fn step(
    proposal: &Proposal,
    edits: &Edits,
    links: &Links,
) -> Result<ApplyStep, ApplyError> {
    if let Some(linked) = links.get(proposal.id) {
        return Ok(ApplyStep {
            proposal_id: proposal.id,
            operation: proposal.operation.clone(),
            evidence: StepEvidence::Link(linked.clone()),
            firm_reason: None,
        });
    }
    let firm_reason = match proposal.operation {
        Operation::ChangeCommitmentStatus {
            status: CommitmentStatus::Firm,
            ..
        } => Some(
            FirmReason::parse(proposal.reason.as_str()).map_err(|error| {
                ApplyError::Invalid(vec![FieldError::new(
                    format!("proposals/{}/reason", proposal.id),
                    text_error_code(error),
                )])
            })?,
        ),
        _ => None,
    };
    let (operation, evidence) = match (&proposal.operation, edits.get(&proposal.id)) {
        (
            Operation::SetFact {
                event_id,
                field_id,
                expected_version,
                ..
            },
            Some((_, Edited::State(state))),
        ) => (
            Operation::SetFact {
                event_id: *event_id,
                field_id: *field_id,
                state: state.clone(),
                expected_version: *expected_version,
            },
            StepEvidence::Edit,
        ),
        (_, Some((_, Edited::Record(edited)))) => (
            edited.clone(),
            StepEvidence::RecordEdit(proposal.evidence.clone()),
        ),
        _ => (
            proposal.operation.clone(),
            StepEvidence::Proposal(proposal.evidence.clone()),
        ),
    };
    Ok(ApplyStep {
        proposal_id: proposal.id,
        operation: links.resolved(operation),
        evidence,
        firm_reason,
    })
}

pub(super) fn step_status(step: &ApplyStep) -> ProposalStatus {
    match step.evidence {
        StepEvidence::Edit | StepEvidence::RecordEdit(_) | StepEvidence::Link(_) => {
            ProposalStatus::AcceptedWithEdit
        }
        StepEvidence::Proposal(_) => ProposalStatus::Accepted,
    }
}

/// The audit events of one step: the acceptance and, for a new event, the event and its first event manager,
/// and for a work record or a party its creation or change. A link creates no record, so it has the acceptance only.
pub(super) fn audit_of(caller: &MemberCaller, step: &ApplyStep) -> Vec<AuditEvent> {
    let scope = Some(caller.scope());
    let mut events = vec![AuditEvent::new(
        caller.actor(),
        AuditAction::ProposalAccept,
        Some(step.proposal_id.as_uuid()),
        scope,
    )];
    if let StepEvidence::Link(_) = step.evidence {
        return events;
    }
    let record = match step.operation {
        Operation::CreateEvent { id, .. } => {
            events.extend(crate::events::creation_audit(caller, id));
            None
        }
        Operation::CreatePerson { id, .. } => Some((AuditAction::PersonCreate, id.as_uuid())),
        Operation::CreateInstitution { id, .. } => {
            Some((AuditAction::InstitutionCreate, id.as_uuid()))
        }
        Operation::CreateAction { id, .. } => Some((AuditAction::ActionCreate, id.as_uuid())),
        Operation::CreateCommitment { id, .. } => {
            Some((AuditAction::CommitmentCreate, id.as_uuid()))
        }
        Operation::ChangeActionStatus { action_id, .. }
        | Operation::ChangeActionDue { action_id, .. } => {
            Some((AuditAction::ActionChange, action_id.as_uuid()))
        }
        Operation::ChangeCommitmentStatus {
            commitment_id,
            status,
            ..
        } => Some((
            if status == CommitmentStatus::Firm {
                AuditAction::CommitmentFirm
            } else {
                AuditAction::CommitmentChange
            },
            commitment_id.as_uuid(),
        )),
        _ => None,
    };
    if let Some((action, id)) = record {
        events.push(AuditEvent::new(caller.actor(), action, Some(id), scope));
    }
    events
}
