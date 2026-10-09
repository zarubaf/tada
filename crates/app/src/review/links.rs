//! Links of proposed persons and institutions to existing records, and the duplicate candidates that a review
//! shows for them (ADR 0069).

use std::collections::{HashMap, HashSet};

use tada_domain::ids::{InstitutionId, PersonId, ProposalId};
use tada_domain::parties::{Party, PartyName, normalized_name};
use tada_domain::proposals::Operation;
use uuid::Uuid;

use super::checks::Edits;
use super::selection::{proposal_status, with_dependents};
use super::{ApplyError, ProposalStatus, ReviewRecord, finish};
use crate::caller::OrgScope;
use crate::parties::{PartyRef, PartyStore};
use crate::problem::FieldError;
use crate::proposals::Changeset;
use crate::store::StoreError;

/// A review shows at most this number of duplicate candidates for each proposal.
pub const MAX_DUPLICATES: usize = 5;

/// The reviewer's choice to use an existing person or institution instead of the one that a proposal creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub proposal_id: ProposalId,
    pub record_id: Uuid,
}

/// The existing record that a linked proposal uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRecord {
    pub party: Party,
    /// The readable ID, for example `PER-007`.
    pub local_id: String,
}

impl LinkedRecord {
    /// The text of the review source version of the link: it names the chosen record (ADR 0069).
    pub fn review_text(&self) -> String {
        format!("linked to {}", self.local_id)
    }
}

/// The valid links of an apply.
#[derive(Debug, Default)]
pub(super) struct Links {
    by_proposal: HashMap<ProposalId, LinkedRecord>,
    /// The linked record of each ID that a linked proposal would give its new record.
    by_proposed: HashMap<Uuid, Party>,
}

impl Links {
    pub(super) fn get(&self, proposal: ProposalId) -> Option<&LinkedRecord> {
        self.by_proposal.get(&proposal)
    }

    /// The operation with each party that a linked proposal creates replaced by its linked record.
    pub(super) fn resolved(&self, mut operation: Operation) -> Operation {
        if let Operation::CreateCommitment { promisor, .. } = &mut operation
            && let Some(record) = self.by_proposed.get(&promisor.as_uuid())
        {
            *promisor = *record;
        }
        operation
    }
}

/// Checks each link: the proposal is a selected `CreatePerson` or `CreateInstitution` without an edit and without
/// another link, and the record is an existing record of the same kind in the organization. Else `invalid-link`.
/// Each open proposal that depends on a linked proposal must be selected too, else `dependents-not-selected`:
/// a later apply would not know the link and would look for the proposed record.
pub(super) async fn check_links(
    scope: OrgScope,
    changeset: &Changeset,
    selected: &HashSet<ProposalId>,
    results: &[ReviewRecord],
    edits: &Edits,
    links: Vec<Link>,
    parties: &dyn PartyStore,
) -> Result<Links, ApplyError> {
    let mut errors = Vec::new();
    let mut checked = Links::default();
    for (index, link) in links.into_iter().enumerate() {
        let field = format!("links/{index}");
        let id = link.proposal_id;
        let operation = changeset
            .proposals
            .iter()
            .find(|proposal| proposal.id == id)
            .filter(|_| {
                selected.contains(&id)
                    && !edits.contains_key(&id)
                    && !checked.by_proposal.contains_key(&id)
            })
            .map(|proposal| &proposal.operation);
        let record = match operation {
            Some(Operation::CreatePerson { .. }) => parties
                .person(scope, PersonId::from_uuid(link.record_id))
                .await?
                .map(|person| LinkedRecord {
                    party: Party::Person(person.id),
                    local_id: person.local_id(),
                }),
            Some(Operation::CreateInstitution { .. }) => parties
                .institution(scope, InstitutionId::from_uuid(link.record_id))
                .await?
                .map(|institution| LinkedRecord {
                    party: Party::Institution(institution.id),
                    local_id: institution.local_id(),
                }),
            _ => None,
        };
        let Some(record) = record else {
            errors.push(FieldError::new(field, "invalid-link"));
            continue;
        };
        let dependents_selected = with_dependents(&changeset.proposals, &HashSet::from([id]))
            .into_iter()
            .filter(|dependent| proposal_status(results, *dependent) == ProposalStatus::Open)
            .all(|dependent| selected.contains(&dependent));
        if !dependents_selected {
            errors.push(FieldError::new(field, "dependents-not-selected"));
        }
        if let Some(proposed) = operation.and_then(Operation::new_record) {
            checked.by_proposed.insert(proposed.as_uuid(), record.party);
        }
        checked.by_proposal.insert(id, record);
    }
    finish(errors)?;
    Ok(checked)
}

/// The existing records of the organization of the same kind as `proposed` with a name like `name`,
/// at most `MAX_DUPLICATES`, in the order of the store.
pub(super) async fn duplicates(
    scope: OrgScope,
    proposed: Party,
    name: &PartyName,
    parties: &dyn PartyStore,
) -> Result<Vec<PartyRef>, StoreError> {
    let same_kind = |party: Party| {
        matches!(
            (party, proposed),
            (Party::Person(_), Party::Person(_)) | (Party::Institution(_), Party::Institution(_))
        )
    };
    Ok(parties
        .named_like(scope, &normalized_name(name.as_str()))
        .await?
        .into_iter()
        .filter(|candidate| same_kind(candidate.party) && candidate.party != proposed)
        .take(MAX_DUPLICATES)
        .collect())
}
