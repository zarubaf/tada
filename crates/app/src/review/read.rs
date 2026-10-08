//! The review of one changeset: its proposals with their evidence, status and current values (ADR 0050).

use jiff::Timestamp;
use tada_domain::ids::{ChangesetId, EventId, SourceVersionId};
use tada_domain::proposals::{Operation, Proposal};
use tada_domain::sources::Excerpt;

use super::{ProposalStatus, ReviewQueryError, ReviewStores, proposal_status, reviewable};
use crate::caller::{Actor, MemberCaller};
use crate::clock::Clock;
use crate::facts::FactVersionRef;
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
}

/// Why a proposal conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictReason {
    /// The fact has another version than the proposal expects.
    FactChanged,
    /// Another target record changed, for example a field that is deprecated now.
    TargetChanged,
}

impl ConflictReason {
    /// The API value (ADR 0044).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FactChanged => "fact-changed",
            Self::TargetChanged => "target-changed",
        }
    }
}

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
        let excerpts = proposal
            .evidence
            .iter()
            .map(|passage| passage.excerpt(source.as_str(), EXCERPT_CONTEXT))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| StoreError::Internal(Box::new(PassageOutsideText)))?;
        proposals.push(ProposalReview {
            conflict: (status == ProposalStatus::Conflict)
                .then(|| conflict_reason(&proposal.operation, current.as_ref())),
            stale: status.is_stale(changeset.created_at, now),
            status,
            excerpts,
            current,
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
