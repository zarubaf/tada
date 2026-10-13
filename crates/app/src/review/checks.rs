//! The checks of an apply before the store writes: the edits of the reviewer and the owners of new work records.

use std::collections::{HashMap, HashSet};

use jiff::Timestamp;
use tada_domain::facts::{ChoiceValue, FactState, Label, ValueType, Valued};
use tada_domain::ids::{EventId, FieldDefinitionId, MilestoneId, ProposalId};
use tada_domain::proposals::{NewRecord, NewWorkRecord, Operation};

use super::{ApplyError, ApplyStep, Edit, ReviewStores, edits, finish};
use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::problem::FieldError;
use crate::proposals::{
    Changeset, owner_refusal, state_from_input, value_error_code, workstream_refusal,
};
use crate::store::StoreError;
use crate::work::milestone_refusal;

/// The steps that create an action, a commitment, a milestone or a workstream whose owner or lead is no longer a
/// contributor or a manager of the event. An edited owner passed `check_edits`, so only a member who changed role after
/// the proposal shows here.
pub(super) async fn stale_owners(
    scope: OrgScope,
    steps: &[ApplyStep],
    identity: &dyn IdentityStore,
) -> Result<Vec<ProposalId>, StoreError> {
    let mut stale = Vec::new();
    for step in steps {
        if let Some(NewWorkRecord {
            event_id, owner, ..
        }) = step.operation.new_work_record()
            && owner_refusal(scope, event_id, owner, identity)
                .await?
                .is_some()
        {
            stale.push(step.proposal_id);
        }
    }
    Ok(stale)
}

/// The edited value of one proposal.
pub(super) enum Edited {
    /// The state of a `SetFact`.
    State(FactState<Valued>),
    /// The operation of a new record with the edited fields.
    Record(Operation),
}

/// The edited values by proposal, each with the position of its edit in the input.
pub(super) type Edits = HashMap<ProposalId, (usize, Edited)>;

/// Maps the edits to fact states and edited operations. Each edit names a selected proposal at most once:
/// a `state` for a proposal that sets a fact, `fields` for a proposal that creates a work record or a party.
pub(super) fn parse_edits(
    changeset: &Changeset,
    selected: &HashSet<ProposalId>,
    edits: Vec<Edit>,
) -> Result<Edits, ApplyError> {
    let mut errors = Vec::new();
    let mut parsed = HashMap::new();
    for (index, edit) in edits.into_iter().enumerate() {
        let path = |field: &str| format!("edits/{index}/{field}");
        let proposal = changeset
            .proposals
            .iter()
            .find(|proposal| proposal.id == edit.proposal_id);
        let code = match proposal {
            None => Some("unknown"),
            Some(_) if !selected.contains(&edit.proposal_id) => Some("not-selected"),
            Some(proposal)
                if !matches!(proposal.operation, Operation::SetFact { .. })
                    && edits::editable_fields(&proposal.operation).is_empty() =>
            {
                Some("not-editable")
            }
            Some(_) if parsed.contains_key(&edit.proposal_id) => Some("duplicate"),
            Some(_) => None,
        };
        if let Some(code) = code {
            errors.push(FieldError::new(path("proposal_id"), code));
            continue;
        }
        let Some(proposal) = proposal else {
            continue;
        };
        let is_fact = matches!(proposal.operation, Operation::SetFact { .. });
        let nested = |prefix: &str, nested: Vec<FieldError>| {
            nested
                .into_iter()
                .map(|error| FieldError::new(path(&format!("{prefix}{}", error.field)), error.code))
                .collect::<Vec<_>>()
        };
        let edited = match (is_fact, edit.state, edit.fields) {
            (true, Some(state), None) => state_from_input(state)
                .map(Edited::State)
                .map_err(|errors| nested("state/", errors)),
            (false, None, Some(fields)) => edits::edit_record(&proposal.operation, fields)
                .map(Edited::Record)
                .map_err(|errors| nested("", errors)),
            (true, state, fields) => Err([
                state
                    .is_none()
                    .then(|| FieldError::new(path("state"), "missing")),
                fields
                    .is_some()
                    .then(|| FieldError::new(path("fields"), "not-editable")),
            ]
            .into_iter()
            .flatten()
            .collect()),
            (false, state, fields) => Err([
                fields
                    .is_none()
                    .then(|| FieldError::new(path("fields"), "missing")),
                state
                    .is_some()
                    .then(|| FieldError::new(path("state"), "not-editable")),
            ]
            .into_iter()
            .flatten()
            .collect()),
        };
        match edited {
            Ok(edited) => {
                parsed.insert(edit.proposal_id, (index, edited));
            }
            Err(nested) => errors.extend(nested),
        }
    }
    finish(errors)?;
    Ok(parsed)
}

/// Checks each edited value against the value type of its field: a field of the catalog of the event,
/// or a new field of the selection, with the new choices of the selection.
/// Checks each edited owner and workstream of a new record like the direct command does (ADR 0068).
pub(super) async fn check_edits(
    scope: OrgScope,
    changeset: &Changeset,
    selected: &HashSet<ProposalId>,
    edits: &Edits,
    stores: ReviewStores<'_>,
    now: Timestamp,
) -> Result<(), ApplyError> {
    let mut value_types: HashMap<FieldDefinitionId, ValueType> = HashMap::new();
    let mut events = HashSet::new();
    for proposal in &changeset.proposals {
        if let (Some((_, Edited::State(_))), Some(event)) =
            (edits.get(&proposal.id), proposal.operation.event_id())
        {
            events.insert(event);
        }
    }
    for event in events {
        for field in stores.facts.catalog(scope, event).await? {
            value_types.insert(field.id, field.value_type);
        }
    }
    let selection = || {
        changeset
            .proposals
            .iter()
            .filter(|proposal| selected.contains(&proposal.id))
    };
    for proposal in selection() {
        if let Operation::AddFieldDefinition { id, value_type, .. } = &proposal.operation {
            value_types.insert(*id, value_type.clone());
        }
    }
    for proposal in selection() {
        if let Operation::AddChoiceValue {
            field_id,
            key,
            label,
            ..
        } = &proposal.operation
            && let Some(ValueType::Choice { values, .. }) = value_types.get_mut(field_id)
        {
            values.push(ChoiceValue {
                key: key.clone(),
                label: Label::Text(label.clone()),
            });
        }
    }
    // The new milestones that the apply creates, each with its event: an edited action can name one of its event.
    let new_milestones: HashSet<(Option<EventId>, MilestoneId)> = selection()
        .filter_map(|proposal| match proposal.operation.new_record() {
            Some(NewRecord::Milestone(id)) => Some((proposal.operation.event_id(), id)),
            _ => None,
        })
        .collect();
    let mut errors = Vec::new();
    for proposal in &changeset.proposals {
        match (edits.get(&proposal.id), &proposal.operation) {
            (Some((index, Edited::State(state))), Operation::SetFact { field_id, .. }) => {
                let value = match state {
                    FactState::Accepted(valued) | FactState::Assumption(valued) => &valued.value,
                    FactState::Unknown => continue,
                };
                let checked = match value_types.get(field_id) {
                    Some(value_type) => value.check(value_type).map_err(value_error_code),
                    None => Err("unknown-field"),
                };
                if let Err(code) = checked {
                    errors.push(FieldError::new(format!("edits/{index}/state/value"), code));
                }
            }
            (Some((index, Edited::Record(edited))), proposed) => {
                let path = |field: &str| format!("edits/{index}/fields/{field}");
                for (field, code) in edited_work_refusals(scope, proposed, edited, stores).await? {
                    errors.push(FieldError::new(path(field), code));
                }
                if let Some(code) =
                    edited_milestone_refusal(scope, proposed, edited, &new_milestones, stores, now)
                        .await?
                {
                    errors.push(FieldError::new(path("milestone"), code));
                }
            }
            _ => {}
        }
    }
    finish(errors)
}

/// The refusal of an edited milestone of a new action (ADR 0080): it must be a milestone of the event that is not
/// canceled, or a new milestone of the event in the selection. An unchanged milestone passed the check of the proposal.
async fn edited_milestone_refusal(
    scope: OrgScope,
    proposed: &Operation,
    edited: &Operation,
    new_milestones: &HashSet<(Option<EventId>, MilestoneId)>,
    stores: ReviewStores<'_>,
    now: Timestamp,
) -> Result<Option<&'static str>, StoreError> {
    let (
        Operation::CreateAction {
            milestone: old_milestone,
            ..
        },
        Operation::CreateAction {
            event_id,
            milestone: Some(milestone),
            ..
        },
    ) = (proposed, edited)
    else {
        return Ok(None);
    };
    if *old_milestone == Some(*milestone) || new_milestones.contains(&(Some(*event_id), *milestone))
    {
        return Ok(None);
    }
    milestone_refusal(scope, *event_id, *milestone, stores.milestones, now).await
}

/// The refusals of an edited owner and an edited workstream of a new work record, as (field, code).
pub(super) async fn edited_work_refusals(
    scope: OrgScope,
    proposed: &Operation,
    edited: &Operation,
    stores: ReviewStores<'_>,
) -> Result<Vec<(&'static str, &'static str)>, StoreError> {
    let (
        Some(NewWorkRecord {
            event_id: event,
            owner: old_owner,
            workstream: old_workstream,
        }),
        Some(NewWorkRecord {
            owner, workstream, ..
        }),
    ) = (proposed.new_work_record(), edited.new_work_record())
    else {
        return Ok(Vec::new());
    };
    let mut refusals = Vec::new();
    if owner != old_owner
        && let Some(code) = owner_refusal(scope, event, owner, stores.identity).await?
    {
        refusals.push(("owner", code));
    }
    if workstream != old_workstream
        && let Some(code) = workstream_refusal(scope, event, workstream, stores.workstreams).await?
    {
        refusals.push(("workstream", code));
    }
    Ok(refusals)
}
