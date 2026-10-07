//! Proposals (ADR 0050): the typed operations of a changeset and the rules of their dependencies.
//!
//! A proposal never changes after its creation.
//! An operation that creates a record carries the UUIDv7 of the new record (ADR 0038).
//! Other proposals of the same changeset refer to the new record by this ID.

use std::collections::{HashMap, VecDeque};

use crate::RecordVersion;
use crate::events::{EventKey, EventName, EventTimeZone};
use crate::facts::{
    ChoiceKey, Description, FactState, FieldKey, ModuleKey, ShortText, TextError, ValueType,
    Valued, checked_text,
};
use crate::ids::{EventId, FieldDefinitionId, OpenQuestionId, ProposalId, UserId};
use crate::sources::Passage;

/// The change that a proposal suggests. Slice 1 has these operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Create an event of layer 1 (ADR 0049).
    CreateEvent {
        id: EventId,
        key: EventKey,
        name: EventName,
        time_zone: EventTimeZone,
    },
    /// Set the fact of a field in an event: a new state with its value.
    /// `expected_version` is the current version of the fact. `None` means that the event has no fact of the field yet.
    SetFact {
        event_id: EventId,
        field_id: FieldDefinitionId,
        state: FactState<Valued>,
        expected_version: Option<RecordVersion>,
    },
    /// Add a field definition to the field catalog of an event.
    AddFieldDefinition {
        id: FieldDefinitionId,
        event_id: EventId,
        key: FieldKey,
        label: ShortText,
        value_type: ValueType,
        description: Description,
        module: ModuleKey,
    },
    /// Add a choice to a choice field of an event.
    AddChoiceValue {
        event_id: EventId,
        field_id: FieldDefinitionId,
        key: ChoiceKey,
        label: ShortText,
    },
    /// Close a field of an event for new facts. The field stays readable.
    DeprecateField {
        event_id: EventId,
        field_id: FieldDefinitionId,
    },
    /// Create an open question with its owner.
    CreateOpenQuestion {
        id: OpenQuestionId,
        event_id: EventId,
        text: QuestionText,
        owner: UserId,
    },
}

/// The ID of the record that an operation creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewRecord {
    Event(EventId),
    Field(FieldDefinitionId),
    OpenQuestion(OpenQuestionId),
}

impl NewRecord {
    pub fn as_uuid(self) -> uuid::Uuid {
        match self {
            Self::Event(id) => id.as_uuid(),
            Self::Field(id) => id.as_uuid(),
            Self::OpenQuestion(id) => id.as_uuid(),
        }
    }
}

impl Operation {
    /// The record that the operation creates, if it creates one.
    pub fn new_record(&self) -> Option<NewRecord> {
        match self {
            Self::CreateEvent { id, .. } => Some(NewRecord::Event(*id)),
            Self::AddFieldDefinition { id, .. } => Some(NewRecord::Field(*id)),
            Self::CreateOpenQuestion { id, .. } => Some(NewRecord::OpenQuestion(*id)),
            Self::SetFact { .. } | Self::AddChoiceValue { .. } | Self::DeprecateField { .. } => {
                None
            }
        }
    }

    /// The event that the operation works in. Each operation names it, so each proposal belongs to one event.
    /// For `CreateEvent`, it is the new event.
    pub fn event_id(&self) -> EventId {
        match self {
            Self::CreateEvent { id, .. } => *id,
            Self::SetFact { event_id, .. }
            | Self::AddFieldDefinition { event_id, .. }
            | Self::AddChoiceValue { event_id, .. }
            | Self::DeprecateField { event_id, .. }
            | Self::CreateOpenQuestion { event_id, .. } => *event_id,
        }
    }

    /// The existing or new field that the operation uses or changes.
    pub fn field_id(&self) -> Option<FieldDefinitionId> {
        match self {
            Self::SetFact { field_id, .. }
            | Self::AddChoiceValue { field_id, .. }
            | Self::DeprecateField { field_id, .. } => Some(*field_id),
            Self::AddFieldDefinition { id, .. } => Some(*id),
            Self::CreateEvent { .. } | Self::CreateOpenQuestion { .. } => None,
        }
    }
}

/// One proposal of a changeset: its operation, the proposals that it depends on, its evidence and its reason.
/// The passages point into the source version of the changeset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: ProposalId,
    pub operation: Operation,
    pub depends_on: Vec<ProposalId>,
    pub evidence: Vec<Passage>,
    pub reason: Reason,
}

/// The short reason of a proposal: 1 to 1000 characters, without control characters and without spaces at the ends.
#[derive(Clone, PartialEq, Eq)]
pub struct Reason(String);

impl Reason {
    pub const MAX_CHARS: usize = 1000;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The reason can quote personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reason({} characters)", self.0.chars().count())
    }
}

/// The text of an open question: 1 to 500 characters, without control characters and without spaces at the ends.
#[derive(Clone, PartialEq, Eq)]
pub struct QuestionText(String);

impl QuestionText {
    pub const MAX_CHARS: usize = 500;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The question can contain personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for QuestionText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuestionText({} characters)", self.0.chars().count())
    }
}

/// The dependencies of a changeset break a rule of ADR 0050.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DependencyError {
    #[error("the proposal {proposal} depends on a proposal outside its changeset")]
    Outside { proposal: ProposalId },
    #[error("the proposal {proposal} is in a dependency cycle or depends on one")]
    Cycle { proposal: ProposalId },
}

/// Checks the dependencies of the proposals of one changeset: each proposal with the proposals that it depends on.
/// The dependencies must stay inside the changeset and form a directed graph without cycles (ADR 0050).
/// The proposal IDs must be unique.
pub fn check_dependencies(graph: &[(ProposalId, Vec<ProposalId>)]) -> Result<(), DependencyError> {
    let index: HashMap<ProposalId, usize> = graph
        .iter()
        .enumerate()
        .map(|(position, (id, _))| (*id, position))
        .collect();
    // For each proposal: the number of its dependencies that are not resolved yet, and its dependents.
    let mut open = vec![0_usize; graph.len()];
    let mut dependents = vec![Vec::new(); graph.len()];
    for (position, (id, depends_on)) in graph.iter().enumerate() {
        for dependency in depends_on {
            let Some(&target) = index.get(dependency) else {
                return Err(DependencyError::Outside { proposal: *id });
            };
            open[position] += 1;
            dependents[target].push(position);
        }
    }
    // Kahn's algorithm: resolve the proposals without open dependencies until none is left.
    let mut ready: VecDeque<usize> = (0..graph.len()).filter(|&p| open[p] == 0).collect();
    while let Some(position) = ready.pop_front() {
        for &dependent in &dependents[position] {
            open[dependent] -= 1;
            if open[dependent] == 0 {
                ready.push_back(dependent);
            }
        }
    }
    match open.iter().position(|&count| count > 0) {
        Some(position) => Err(DependencyError::Cycle {
            proposal: graph[position].0,
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn id(n: u128) -> ProposalId {
        ProposalId::from_uuid(Uuid::from_u128(n))
    }

    #[test]
    fn accepts_dependencies_without_a_cycle() {
        let graph = [
            (id(1), vec![]),
            (id(2), vec![id(1)]),
            (id(3), vec![id(1), id(2)]),
            (id(4), vec![]),
        ];
        assert_eq!(check_dependencies(&graph), Ok(()));
        assert_eq!(check_dependencies(&[]), Ok(()));
    }

    #[test]
    fn rejects_a_cycle() {
        let graph = [(id(1), vec![id(2)]), (id(2), vec![id(1)])];
        assert_eq!(
            check_dependencies(&graph),
            Err(DependencyError::Cycle { proposal: id(1) })
        );
        let longer = [
            (id(1), vec![]),
            (id(2), vec![id(4)]),
            (id(3), vec![id(2)]),
            (id(4), vec![id(3), id(1)]),
        ];
        assert_eq!(
            check_dependencies(&longer),
            Err(DependencyError::Cycle { proposal: id(2) })
        );
    }

    #[test]
    fn rejects_a_proposal_that_depends_on_itself() {
        let graph = [(id(1), vec![id(1)])];
        assert_eq!(
            check_dependencies(&graph),
            Err(DependencyError::Cycle { proposal: id(1) })
        );
    }

    #[test]
    fn rejects_a_dependency_outside_the_changeset() {
        let graph = [(id(1), vec![]), (id(2), vec![id(1), id(9)])];
        assert_eq!(
            check_dependencies(&graph),
            Err(DependencyError::Outside { proposal: id(2) })
        );
    }

    #[test]
    fn checks_reasons_and_question_texts() {
        assert_eq!(
            Reason::parse(" The member names the date. ")
                .unwrap()
                .as_str(),
            "The member names the date."
        );
        assert_eq!(Reason::parse(" "), Err(TextError::Empty));
        assert_eq!(
            Reason::parse(&"a".repeat(Reason::MAX_CHARS + 1)),
            Err(TextError::TooLong)
        );
        assert_eq!(
            QuestionText::parse("Which weekend?").unwrap().as_str(),
            "Which weekend?"
        );
        assert_eq!(
            QuestionText::parse(&"a".repeat(QuestionText::MAX_CHARS + 1)),
            Err(TextError::TooLong)
        );
    }

    #[test]
    fn debug_hides_reasons_and_questions() {
        let reason = Reason::parse("Anna Muster said so").unwrap();
        assert!(!format!("{reason:?}").contains("Anna"));
        let question = QuestionText::parse("Does Anna Muster come?").unwrap();
        assert!(!format!("{question:?}").contains("Anna"));
    }
}
