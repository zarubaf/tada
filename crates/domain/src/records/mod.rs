//! References to existing records: the records that proposals change and that evidence supports.

use crate::ids::{ActionId, CommitmentId, InstitutionId, PersonId};

/// A record that accepted proposals support with evidence (ADR 0068, ADR 0069).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordRef {
    Action(ActionId),
    Commitment(CommitmentId),
    Person(PersonId),
    Institution(InstitutionId),
}
