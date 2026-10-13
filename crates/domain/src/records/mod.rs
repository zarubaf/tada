//! The records of an event and references to them: the records that proposals change and that evidence supports.

pub mod decision;
pub mod milestone;
pub mod requirement;
pub mod risk;

use crate::ids::{ActionId, CommitmentId, InstitutionId, MilestoneId, PersonId};

pub use decision::{DecisionParties, DecisionStatus, DecisionWording, EmptyParties};
pub use milestone::{MilestoneStatus, MilestoneTitle};
pub use requirement::{RequirementStatus, RequirementText};
pub use risk::{Level, RiskRating, RiskStatus, RiskTitle};

/// The reason to revoke a decision or to waive a requirement: 1 to 500 characters.
/// It is the type of the reason to make a commitment firm.
pub use crate::work::FirmReason as Reason;

/// A record that accepted proposals support with evidence (ADR 0068, ADR 0069).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordRef {
    Action(ActionId),
    Commitment(CommitmentId),
    Person(PersonId),
    Institution(InstitutionId),
    Milestone(MilestoneId),
}
