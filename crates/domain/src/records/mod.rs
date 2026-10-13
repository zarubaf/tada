//! The records of an event and references to them: the records that proposals change and that evidence supports.

pub mod decision;
pub mod milestone;
pub mod requirement;
pub mod risk;

use uuid::Uuid;

use crate::ids::{
    ActionId, CommitmentId, DecisionId, InstitutionId, MilestoneId, PersonId, RiskId,
};

pub use decision::{
    DecisionChange, DecisionParties, DecisionStatus, DecisionWording, PartiesError,
};
pub use milestone::{MilestoneStatus, MilestoneTitle};
pub use requirement::{RequirementStatus, RequirementText};
pub use risk::{Level, RiskRating, RiskStatus, RiskTitle};

pub use crate::work::RecordReason;

/// A record that accepted proposals support with evidence (ADR 0068, ADR 0069).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordRef {
    Action(ActionId),
    Commitment(CommitmentId),
    Person(PersonId),
    Institution(InstitutionId),
    Milestone(MilestoneId),
    Decision(DecisionId),
    Risk(RiskId),
}

/// The kind of a record that a `RecordRef` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordKind {
    Action,
    Commitment,
    Person,
    Institution,
    Milestone,
    Decision,
    Risk,
}

impl RecordKind {
    pub const ALL: [Self; 7] = [
        Self::Action,
        Self::Commitment,
        Self::Person,
        Self::Institution,
        Self::Milestone,
        Self::Decision,
        Self::Risk,
    ];

    /// The name of the table of the kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Commitment => "commitment",
            Self::Person => "person",
            Self::Institution => "institution",
            Self::Milestone => "milestone",
            Self::Decision => "decision",
            Self::Risk => "risk",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == input)
    }
}

impl RecordRef {
    /// The reference to the record of `kind` with the ID `id`.
    pub const fn new(kind: RecordKind, id: Uuid) -> Self {
        match kind {
            RecordKind::Action => Self::Action(ActionId::from_uuid(id)),
            RecordKind::Commitment => Self::Commitment(CommitmentId::from_uuid(id)),
            RecordKind::Person => Self::Person(PersonId::from_uuid(id)),
            RecordKind::Institution => Self::Institution(InstitutionId::from_uuid(id)),
            RecordKind::Milestone => Self::Milestone(MilestoneId::from_uuid(id)),
            RecordKind::Decision => Self::Decision(DecisionId::from_uuid(id)),
            RecordKind::Risk => Self::Risk(RiskId::from_uuid(id)),
        }
    }

    pub const fn kind(self) -> RecordKind {
        match self {
            Self::Action(_) => RecordKind::Action,
            Self::Commitment(_) => RecordKind::Commitment,
            Self::Person(_) => RecordKind::Person,
            Self::Institution(_) => RecordKind::Institution,
            Self::Milestone(_) => RecordKind::Milestone,
            Self::Decision(_) => RecordKind::Decision,
            Self::Risk(_) => RecordKind::Risk,
        }
    }

    pub const fn as_uuid(self) -> Uuid {
        match self {
            Self::Action(id) => id.as_uuid(),
            Self::Commitment(id) => id.as_uuid(),
            Self::Person(id) => id.as_uuid(),
            Self::Institution(id) => id.as_uuid(),
            Self::Milestone(id) => id.as_uuid(),
            Self::Decision(id) => id.as_uuid(),
            Self::Risk(id) => id.as_uuid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_kind_reads_back_its_name() {
        for kind in RecordKind::ALL {
            assert_eq!(RecordKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(RecordKind::parse("event"), None);
    }

    #[test]
    fn a_reference_keeps_its_kind_and_id() {
        let id = Uuid::from_u128(7);
        for kind in RecordKind::ALL {
            let record = RecordRef::new(kind, id);
            assert_eq!((record.kind(), record.as_uuid()), (kind, id));
        }
    }
}
