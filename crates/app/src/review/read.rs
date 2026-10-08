//! The review of one changeset: its proposals with their evidence, status and current values (ADR 0050).

use std::collections::HashMap;

use jiff::Timestamp;
use tada_domain::ids::{ChangesetId, EventId, SourceVersionId};
use tada_domain::proposals::{Operation, Proposal};
use tada_domain::sources::{Excerpt, SourceText};

use super::{ProposalStatus, ReviewQueryError, ReviewStores, proposal_status, reviewable};
use crate::access;
use crate::caller::{Actor, MemberCaller};
use crate::clock::Clock;
use crate::documents::{DraftRendering, resolve_links};
use crate::facts::FactVersionRef;
use crate::proposals::Changeset;
use crate::store::StoreError;

/// Each excerpt of the evidence shows at most this number of characters before and after its passage.
pub const EXCERPT_CONTEXT: u32 = 100;

/// A changeset as a reviewer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesetReview {
    pub id: ChangesetId,
    /// `None` for a changeset of the organization, for example one that creates an event.
    pub event_id: Option<EventId>,
    pub author: Actor,
    pub source_version_id: SourceVersionId,
    pub created_at: Timestamp,
    /// The proposals in the order of the changeset.
    pub proposals: Vec<ProposalReview>,
}

/// One proposal as a reviewer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalReview {
    pub proposal: Proposal,
    /// One excerpt of the source text for each passage of the evidence of the proposal, in the same order.
    pub excerpts: Vec<Excerpt>,
    pub status: ProposalStatus,
    /// True if the proposal is open and older than `STALE_AFTER`.
    pub stale: bool,
    /// Why the proposal conflicts. `None` unless the status is `Conflict`.
    pub conflict: Option<ConflictReason>,
    /// The current version of the fact that a `SetFact` proposal sets, or `None`.
    pub current: Option<FactVersionRef>,
    /// A draft proposal as the reviewer sees it: its Markdown, its lint warnings and the target of each link.
    /// `None` for each other operation.
    pub draft: Option<DraftRendering>,
}

/// Why a proposal conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictReason {
    /// The fact has another version than the proposal expects.
    FactChanged,
    /// Another target record changed, for example a field that is deprecated now.
    TargetChanged,
}

/// A draft proposal without its stored provenance.
#[derive(Debug, thiserror::Error)]
#[error("a draft proposal has no stored provenance")]
struct MissingProvenance;

/// A stored passage that does not match the text of its source version.
#[derive(Debug, thiserror::Error)]
#[error("a stored passage is outside the text of its source version")]
struct PassageOutsideText;

/// The changeset with each proposal, its evidence, its status and the current value of its target,
/// for a caller who can review the changeset.
pub async fn get_changeset(
    caller: &MemberCaller,
    id: ChangesetId,
    stores: ReviewStores<'_>,
    clock: &dyn Clock,
) -> Result<ChangesetReview, ReviewQueryError> {
    let scope = caller.scope();
    let (changeset, source) = reviewable(caller, id, stores).await?;
    let texts = evidence_texts(caller, &changeset, source, stores).await?;
    let results = stores.review.results(scope, id).await?;
    let now = clock.now();
    let mut proposals = Vec::new();
    for proposal in changeset.proposals {
        let status = proposal_status(&results, proposal.id);
        let current = match proposal.operation {
            Operation::SetFact {
                event_id, field_id, ..
            } => {
                stores
                    .facts
                    .current_version(scope, event_id, field_id)
                    .await?
            }
            _ => None,
        };
        let draft = match &proposal.operation {
            Operation::CreateDocumentDraft {
                event_id, markdown, ..
            } => {
                let provenance = changeset
                    .drafts
                    .iter()
                    .find(|draft| draft.proposal_id == proposal.id)
                    .ok_or_else(|| StoreError::Internal(Box::new(MissingProvenance)))?;
                let links = resolve_links(
                    caller,
                    *event_id,
                    &provenance.manifest,
                    stores.identity,
                    stores.facts,
                    stores.sources,
                )
                .await?;
                Some(DraftRendering {
                    markdown: markdown.clone(),
                    lint_warnings: provenance.lint_warnings.clone(),
                    links,
                })
            }
            _ => None,
        };
        let excerpts = proposal
            .evidence
            .iter()
            .map(|evidence| {
                let text = texts.get(&evidence.source_version_id)?;
                evidence.passage.excerpt(text.as_str(), EXCERPT_CONTEXT)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| StoreError::Internal(Box::new(PassageOutsideText)))?;
        proposals.push(ProposalReview {
            conflict: (status == ProposalStatus::Conflict)
                .then(|| conflict_reason(&proposal.operation, current.as_ref())),
            stale: status.is_stale(changeset.created_at, now),
            status,
            excerpts,
            current,
            draft,
            proposal,
        });
    }
    Ok(ChangesetReview {
        id: changeset.id,
        event_id: changeset.event_id,
        author: changeset.author,
        source_version_id: changeset.source_version_id,
        created_at: changeset.created_at,
        proposals,
    })
}

/// The text of each source version that the evidence of the changeset cites: the source text of the changeset,
/// and each other source version in the reach of the caller (`access::source_reach`).
/// A reviewer of the changeset reads the evidence of its event, so a missing text is an inconsistent store.
async fn evidence_texts(
    caller: &MemberCaller,
    changeset: &Changeset,
    source: SourceText,
    stores: ReviewStores<'_>,
) -> Result<HashMap<SourceVersionId, SourceText>, StoreError> {
    let mut texts = HashMap::from([(changeset.source_version_id, source)]);
    let mut cited: Vec<SourceVersionId> = changeset
        .proposals
        .iter()
        .flat_map(|proposal| &proposal.evidence)
        .map(|evidence| evidence.source_version_id)
        .filter(|id| !texts.contains_key(id))
        .collect();
    cited.sort();
    cited.dedup();
    if cited.is_empty() {
        return Ok(texts);
    }
    let reach = access::source_reach(caller, stores.identity).await?;
    for version in stores.sources.texts(caller.scope(), &reach, &cited).await? {
        if let Some(text) = version.text {
            texts.insert(version.id, text);
        }
    }
    Ok(texts)
}

/// The reason of a conflict: a fact with another version, or another change of the target.
fn conflict_reason(operation: &Operation, current: Option<&FactVersionRef>) -> ConflictReason {
    match operation {
        Operation::SetFact {
            expected_version, ..
        } if current.map(|current| current.number) != *expected_version => {
            ConflictReason::FactChanged
        }
        _ => ConflictReason::TargetChanged,
    }
}

#[cfg(test)]
mod tests {
    use tada_domain::RecordVersion;
    use tada_domain::facts::FactState;
    use tada_domain::ids::{FactId, FactVersionId, FieldDefinitionId};
    use uuid::Uuid;

    use super::*;

    fn set_fact(expected: Option<i64>) -> Operation {
        Operation::SetFact {
            event_id: EventId::from_uuid(Uuid::from_u128(1)),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(2)),
            state: FactState::Unknown,
            expected_version: expected.map(|n| RecordVersion::new(n).unwrap()),
        }
    }

    fn version(number: i64) -> FactVersionRef {
        FactVersionRef {
            id: FactVersionId::from_uuid(Uuid::from_u128(3)),
            fact_id: FactId::from_uuid(Uuid::from_u128(4)),
            number: RecordVersion::new(number).unwrap(),
            state: FactState::Unknown,
        }
    }

    #[test]
    fn a_fact_with_another_version_is_the_reason_of_a_conflict() {
        assert_eq!(
            conflict_reason(&set_fact(None), Some(&version(1))),
            ConflictReason::FactChanged
        );
        assert_eq!(
            conflict_reason(&set_fact(Some(1)), Some(&version(2))),
            ConflictReason::FactChanged
        );
        assert_eq!(
            conflict_reason(&set_fact(Some(1)), Some(&version(1))),
            ConflictReason::TargetChanged
        );
        let deprecation = Operation::DeprecateField {
            event_id: EventId::from_uuid(Uuid::from_u128(1)),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(2)),
        };
        assert_eq!(
            conflict_reason(&deprecation, None),
            ConflictReason::TargetChanged
        );
    }
}
