//! Proposals (ADR 0050): the typed operations of a changeset and the rules of their dependencies.
//!
//! A proposal never changes after its creation.
//! An operation that creates a record carries the UUIDv7 of the new record (ADR 0038).
//! Other proposals of the same changeset refer to the new record by this ID.

use std::collections::{HashMap, VecDeque};

use jiff::civil::Date;

use crate::RecordVersion;
use crate::documents::{DocumentName, DraftMarkdown};
use crate::events::{EventKey, EventName, EventTimeZone};
use crate::facts::{
    ChoiceKey, Description, FactState, FieldKey, ModuleKey, ShortText, TextError, ValueType,
    Valued, checked_text,
};
use crate::identity::Email;
use crate::ids::{
    ActionId, CommitmentId, DecisionId, DocumentId, EventId, FieldDefinitionId, InstitutionId,
    MilestoneId, OpenQuestionId, PersonId, ProposalId, RiskId, UserId, WorkstreamId,
};
use crate::parties::{InstitutionKind, Party, PartyName, PhoneNumber};
use crate::records::{
    DecisionParties, DecisionWording, MilestoneStatus, MilestoneTitle, RecordReason, RecordRef,
    RiskRating, RiskStatus, RiskTitle,
};
use crate::sources::Evidence;
use crate::work::{
    ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText, RecordDescription,
    WorkstreamName,
};

/// The change that a proposal suggests.
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
    /// Add a draft version to a document of an event (ADR 0051).
    /// The links of the Markdown cite fact versions and source passages; their provenance manifest is fixed when tada stores the proposal.
    CreateDocumentDraft {
        event_id: EventId,
        document: DraftDocument,
        markdown: DraftMarkdown,
    },
    /// Create a person of the organization (ADR 0069). A person belongs to no event.
    CreatePerson {
        id: PersonId,
        name: PartyName,
        email: Option<Email>,
        phone: Option<PhoneNumber>,
    },
    /// Create an institution of the organization (ADR 0069). An institution belongs to no event.
    CreateInstitution {
        id: InstitutionId,
        name: PartyName,
        kind: InstitutionKind,
        email: Option<Email>,
        phone: Option<PhoneNumber>,
    },
    /// Create an action of an event with its owner (ADR 0068). It starts `open`.
    /// The start date is not after the due date, and the milestone is one of the same event (ADR 0080).
    CreateAction {
        id: ActionId,
        event_id: EventId,
        title: ActionTitle,
        description: Option<RecordDescription>,
        owner: UserId,
        workstream: Option<WorkstreamId>,
        start_date: Option<Date>,
        due_date: Option<Date>,
        milestone: Option<MilestoneId>,
    },
    /// Create a commitment of an event (ADR 0068). It starts `conditional` with a condition, else `firm`.
    CreateCommitment {
        id: CommitmentId,
        event_id: EventId,
        text: CommitmentText,
        promisor: Party,
        owner: UserId,
        workstream: Option<WorkstreamId>,
        due_date: Option<Date>,
        condition: Option<ConditionText>,
    },
    /// Change the status of an action. `expected_version` is the current version of the action.
    ChangeActionStatus {
        event_id: EventId,
        action_id: ActionId,
        status: ActionStatus,
        expected_version: RecordVersion,
    },
    /// Set or clear the due date of an action.
    ChangeActionDue {
        event_id: EventId,
        action_id: ActionId,
        due_date: Option<Date>,
        expected_version: RecordVersion,
    },
    /// Set or clear the milestone of an action (ADR 0080).
    ChangeActionMilestone {
        event_id: EventId,
        action_id: ActionId,
        milestone: Option<MilestoneId>,
        expected_version: RecordVersion,
    },
    /// Change the status of a commitment. A change to `firm` is the AI path for "condition met" (ADR 0068).
    ChangeCommitmentStatus {
        event_id: EventId,
        commitment_id: CommitmentId,
        status: CommitmentStatus,
        expected_version: RecordVersion,
    },
    /// Create a workstream of an event with its lead (ADR 0080). It starts `active`.
    /// Only the event managers review it.
    CreateWorkstream {
        id: WorkstreamId,
        event_id: EventId,
        name: WorkstreamName,
        lead: UserId,
    },
    /// Create a milestone of an event with its owner (ADR 0080). It starts `open`.
    CreateMilestone {
        id: MilestoneId,
        event_id: EventId,
        title: MilestoneTitle,
        description: Option<RecordDescription>,
        date: Date,
        owner: UserId,
        workstream: Option<WorkstreamId>,
    },
    /// Change the status of a milestone. `expected_version` is the current version of the milestone.
    ChangeMilestoneStatus {
        event_id: EventId,
        milestone_id: MilestoneId,
        status: MilestoneStatus,
        expected_version: RecordVersion,
    },
    /// Record a decision of an event with its owner (ADR 0080). It starts `in-force`.
    /// Its wording, its date and its parties never change. Only the event managers review it.
    CreateDecision {
        id: DecisionId,
        event_id: EventId,
        wording: DecisionWording,
        decided_on: Date,
        decided_by: DecisionParties,
        owner: UserId,
        workstream: Option<WorkstreamId>,
    },
    /// End a decision in force: the successor replaces it. The successor is another decision of the same event in
    /// force, or a new decision of the same changeset. Only the event managers review it.
    SupersedeDecision {
        event_id: EventId,
        decision_id: DecisionId,
        successor: DecisionId,
        expected_version: RecordVersion,
    },
    /// End a decision in force without a successor, for the reason. Only the event managers review it.
    RevokeDecision {
        event_id: EventId,
        decision_id: DecisionId,
        reason: RecordReason,
        expected_version: RecordVersion,
    },
    /// Record a risk of an event with its owner (ADR 0080). It starts `open`.
    /// The mitigation is optional.
    CreateRisk {
        id: RiskId,
        event_id: EventId,
        title: RiskTitle,
        description: Option<RecordDescription>,
        rating: RiskRating,
        mitigation: Option<RecordDescription>,
        owner: UserId,
        workstream: Option<WorkstreamId>,
    },
    /// Change the status of a risk. `expected_version` is the current version of the risk.
    ChangeRiskStatus {
        event_id: EventId,
        risk_id: RiskId,
        status: RiskStatus,
        expected_version: RecordVersion,
    },
    /// Change the likelihood and the impact of a risk. `expected_version` is the current version of the risk.
    ChangeRiskRating {
        event_id: EventId,
        risk_id: RiskId,
        rating: RiskRating,
        expected_version: RecordVersion,
    },
}

/// The document of a draft: a new document, or an existing document of the event at its current version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftDocument {
    New {
        id: DocumentId,
        name: DocumentName,
    },
    /// `expected_version` is the current record version of the document (ADR 0050).
    Existing {
        document_id: DocumentId,
        expected_version: RecordVersion,
    },
}

impl DraftDocument {
    pub fn document_id(&self) -> DocumentId {
        match self {
            Self::New { id, .. } => *id,
            Self::Existing { document_id, .. } => *document_id,
        }
    }
}

/// The ID of the record that an operation creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewRecord {
    Event(EventId),
    Field(FieldDefinitionId),
    OpenQuestion(OpenQuestionId),
    Document(DocumentId),
    Person(PersonId),
    Institution(InstitutionId),
    Action(ActionId),
    Commitment(CommitmentId),
    Workstream(WorkstreamId),
    Milestone(MilestoneId),
    Decision(DecisionId),
    Risk(RiskId),
}

impl NewRecord {
    pub fn as_uuid(self) -> uuid::Uuid {
        match self {
            Self::Event(id) => id.as_uuid(),
            Self::Field(id) => id.as_uuid(),
            Self::OpenQuestion(id) => id.as_uuid(),
            Self::Document(id) => id.as_uuid(),
            Self::Person(id) => id.as_uuid(),
            Self::Institution(id) => id.as_uuid(),
            Self::Action(id) => id.as_uuid(),
            Self::Commitment(id) => id.as_uuid(),
            Self::Workstream(id) => id.as_uuid(),
            Self::Milestone(id) => id.as_uuid(),
            Self::Decision(id) => id.as_uuid(),
            Self::Risk(id) => id.as_uuid(),
        }
    }
}

/// The event, the owner and the workstream of a new work record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewWorkRecord {
    pub event_id: EventId,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
}

impl Operation {
    /// The record that the operation creates, if it creates one.
    pub fn new_record(&self) -> Option<NewRecord> {
        match self {
            Self::CreateEvent { id, .. } => Some(NewRecord::Event(*id)),
            Self::AddFieldDefinition { id, .. } => Some(NewRecord::Field(*id)),
            Self::CreateOpenQuestion { id, .. } => Some(NewRecord::OpenQuestion(*id)),
            Self::CreateDocumentDraft {
                document: DraftDocument::New { id, .. },
                ..
            } => Some(NewRecord::Document(*id)),
            Self::CreatePerson { id, .. } => Some(NewRecord::Person(*id)),
            Self::CreateInstitution { id, .. } => Some(NewRecord::Institution(*id)),
            Self::CreateAction { id, .. } => Some(NewRecord::Action(*id)),
            Self::CreateCommitment { id, .. } => Some(NewRecord::Commitment(*id)),
            Self::CreateWorkstream { id, .. } => Some(NewRecord::Workstream(*id)),
            Self::CreateMilestone { id, .. } => Some(NewRecord::Milestone(*id)),
            Self::CreateDecision { id, .. } => Some(NewRecord::Decision(*id)),
            Self::CreateRisk { id, .. } => Some(NewRecord::Risk(*id)),
            Self::SetFact { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => None,
        }
    }

    /// The event that the operation works in. For `CreateEvent`, it is the new event.
    /// A person or an institution belongs to the organization, so its operations have no event (ADR 0069).
    pub fn event_id(&self) -> Option<EventId> {
        match self {
            Self::CreateEvent { id, .. } => Some(*id),
            Self::SetFact { event_id, .. }
            | Self::AddFieldDefinition { event_id, .. }
            | Self::AddChoiceValue { event_id, .. }
            | Self::DeprecateField { event_id, .. }
            | Self::CreateOpenQuestion { event_id, .. }
            | Self::CreateDocumentDraft { event_id, .. }
            | Self::CreateAction { event_id, .. }
            | Self::CreateCommitment { event_id, .. }
            | Self::CreateWorkstream { event_id, .. }
            | Self::ChangeActionStatus { event_id, .. }
            | Self::ChangeActionDue { event_id, .. }
            | Self::ChangeActionMilestone { event_id, .. }
            | Self::ChangeCommitmentStatus { event_id, .. }
            | Self::CreateMilestone { event_id, .. }
            | Self::ChangeMilestoneStatus { event_id, .. }
            | Self::CreateDecision { event_id, .. }
            | Self::SupersedeDecision { event_id, .. }
            | Self::RevokeDecision { event_id, .. }
            | Self::CreateRisk { event_id, .. }
            | Self::ChangeRiskStatus { event_id, .. }
            | Self::ChangeRiskRating { event_id, .. } => Some(*event_id),
            Self::CreatePerson { .. } | Self::CreateInstitution { .. } => None,
        }
    }

    /// The promisor of a new commitment, which can be a new record of the same changeset.
    pub fn promisor(&self) -> Option<Party> {
        match self {
            Self::CreateCommitment { promisor, .. } => Some(*promisor),
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateAction { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::CreateWorkstream { .. }
            | Self::CreateMilestone { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::CreateDecision { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. }
            | Self::CreateRisk { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => None,
        }
    }

    /// The existing record that a change operation changes. Its owner reviews the proposal (ADR 0067, rule 1).
    pub fn changed_record(&self) -> Option<RecordRef> {
        match self {
            Self::ChangeActionStatus { action_id, .. }
            | Self::ChangeActionDue { action_id, .. }
            | Self::ChangeActionMilestone { action_id, .. } => Some(RecordRef::Action(*action_id)),
            Self::ChangeCommitmentStatus { commitment_id, .. } => {
                Some(RecordRef::Commitment(*commitment_id))
            }
            Self::ChangeMilestoneStatus { milestone_id, .. } => {
                Some(RecordRef::Milestone(*milestone_id))
            }
            Self::SupersedeDecision { decision_id, .. }
            | Self::RevokeDecision { decision_id, .. } => Some(RecordRef::Decision(*decision_id)),
            Self::ChangeRiskStatus { risk_id, .. } | Self::ChangeRiskRating { risk_id, .. } => {
                Some(RecordRef::Risk(*risk_id))
            }
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateAction { .. }
            | Self::CreateCommitment { .. }
            | Self::CreateWorkstream { .. }
            | Self::CreateMilestone { .. }
            | Self::CreateDecision { .. }
            | Self::CreateRisk { .. } => None,
        }
    }

    /// The version of its target that the operation expects: the record that it changes, the fact that it sets
    /// (`None` if the event has no fact of the field yet) or the existing document of a draft.
    pub fn expected_version(&self) -> Option<RecordVersion> {
        match self {
            Self::SetFact {
                expected_version, ..
            } => *expected_version,
            Self::CreateDocumentDraft {
                document:
                    DraftDocument::Existing {
                        expected_version, ..
                    },
                ..
            }
            | Self::ChangeActionStatus {
                expected_version, ..
            }
            | Self::ChangeActionDue {
                expected_version, ..
            }
            | Self::ChangeActionMilestone {
                expected_version, ..
            }
            | Self::ChangeCommitmentStatus {
                expected_version, ..
            }
            | Self::ChangeMilestoneStatus {
                expected_version, ..
            }
            | Self::SupersedeDecision {
                expected_version, ..
            }
            | Self::RevokeDecision {
                expected_version, ..
            }
            | Self::ChangeRiskStatus {
                expected_version, ..
            }
            | Self::ChangeRiskRating {
                expected_version, ..
            } => Some(*expected_version),
            Self::CreateDocumentDraft {
                document: DraftDocument::New { .. },
                ..
            }
            | Self::CreateEvent { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateAction { .. }
            | Self::CreateCommitment { .. }
            | Self::CreateWorkstream { .. }
            | Self::CreateMilestone { .. }
            | Self::CreateDecision { .. }
            | Self::CreateRisk { .. } => None,
        }
    }

    /// The event, the owner and the workstream of the work record that the operation creates (ADR 0067, ADR 0068).
    /// The owner of a new workstream is its lead, and a workstream has no workstream.
    pub fn new_work_record(&self) -> Option<NewWorkRecord> {
        match self {
            Self::CreateAction {
                event_id,
                owner,
                workstream,
                ..
            }
            | Self::CreateCommitment {
                event_id,
                owner,
                workstream,
                ..
            }
            | Self::CreateMilestone {
                event_id,
                owner,
                workstream,
                ..
            }
            | Self::CreateDecision {
                event_id,
                owner,
                workstream,
                ..
            }
            | Self::CreateRisk {
                event_id,
                owner,
                workstream,
                ..
            } => Some(NewWorkRecord {
                event_id: *event_id,
                owner: *owner,
                workstream: *workstream,
            }),
            Self::CreateWorkstream { event_id, lead, .. } => Some(NewWorkRecord {
                event_id: *event_id,
                owner: *lead,
                workstream: None,
            }),
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => None,
        }
    }

    /// The workstream of the record that a create operation creates. Its lead reviews the proposal (ADR 0067, rule 2).
    pub fn workstream(&self) -> Option<WorkstreamId> {
        match self {
            Self::CreateAction { workstream, .. }
            | Self::CreateCommitment { workstream, .. }
            | Self::CreateMilestone { workstream, .. }
            | Self::CreateDecision { workstream, .. }
            | Self::CreateRisk { workstream, .. } => *workstream,
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateWorkstream { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => None,
        }
    }

    /// True if only the event managers review the proposal, whatever record or workstream it names:
    /// a new workstream and each decision operation (ADR 0080).
    pub fn managers_only(&self) -> bool {
        match self {
            Self::CreateWorkstream { .. }
            | Self::CreateDecision { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. } => true,
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateAction { .. }
            | Self::CreateCommitment { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::CreateMilestone { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::CreateRisk { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => false,
        }
    }

    /// The records that the operation names and that a proposal of the same changeset can create
    /// (ADR 0067, rule 5): the promisor of a new commitment, the workstream of a new record, the milestone of an
    /// action, the parties of a new decision and the successor of a superseded decision (ADR 0080).
    pub fn references(&self) -> Vec<NewRecord> {
        let promisor = self.promisor().map(party_record);
        let workstream = self.workstream().map(NewRecord::Workstream);
        let named: Vec<NewRecord> = match self {
            Self::CreateAction { milestone, .. }
            | Self::ChangeActionMilestone { milestone, .. } => {
                milestone.map(NewRecord::Milestone).into_iter().collect()
            }
            Self::CreateDecision { decided_by, .. } => decided_by
                .as_slice()
                .iter()
                .map(|party| party_record(*party))
                .collect(),
            Self::SupersedeDecision { successor, .. } => vec![NewRecord::Decision(*successor)],
            Self::CreateEvent { .. }
            | Self::SetFact { .. }
            | Self::AddFieldDefinition { .. }
            | Self::AddChoiceValue { .. }
            | Self::DeprecateField { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateCommitment { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::CreateWorkstream { .. }
            | Self::CreateMilestone { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::RevokeDecision { .. }
            | Self::CreateRisk { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => Vec::new(),
        };
        promisor
            .into_iter()
            .chain(workstream)
            .chain(named)
            .collect()
    }

    /// The existing or new field that the operation uses or changes.
    pub fn field_id(&self) -> Option<FieldDefinitionId> {
        match self {
            Self::SetFact { field_id, .. }
            | Self::AddChoiceValue { field_id, .. }
            | Self::DeprecateField { field_id, .. } => Some(*field_id),
            Self::AddFieldDefinition { id, .. } => Some(*id),
            Self::CreateEvent { .. }
            | Self::CreateOpenQuestion { .. }
            | Self::CreateDocumentDraft { .. }
            | Self::CreatePerson { .. }
            | Self::CreateInstitution { .. }
            | Self::CreateAction { .. }
            | Self::CreateCommitment { .. }
            | Self::CreateWorkstream { .. }
            | Self::ChangeActionStatus { .. }
            | Self::ChangeActionDue { .. }
            | Self::ChangeActionMilestone { .. }
            | Self::ChangeCommitmentStatus { .. }
            | Self::CreateMilestone { .. }
            | Self::ChangeMilestoneStatus { .. }
            | Self::CreateDecision { .. }
            | Self::SupersedeDecision { .. }
            | Self::RevokeDecision { .. }
            | Self::CreateRisk { .. }
            | Self::ChangeRiskStatus { .. }
            | Self::ChangeRiskRating { .. } => None,
        }
    }
}

/// The new record that a party is if a proposal of the same changeset creates it.
fn party_record(party: Party) -> NewRecord {
    match party {
        Party::Person(id) => NewRecord::Person(id),
        Party::Institution(id) => NewRecord::Institution(id),
    }
}

/// One proposal of a changeset: its operation, the proposals that it depends on, its evidence and its reason.
/// Each passage of the evidence names its source version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: ProposalId,
    pub operation: Operation,
    pub depends_on: Vec<ProposalId>,
    pub evidence: Vec<Evidence>,
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

    fn event() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    fn version() -> RecordVersion {
        RecordVersion::new(1).unwrap()
    }

    fn new_commitment(promisor: Party, workstream: Option<WorkstreamId>) -> Operation {
        Operation::CreateCommitment {
            id: CommitmentId::from_uuid(Uuid::from_u128(32)),
            event_id: event(),
            text: CommitmentText::parse("Liefert den Generator").unwrap(),
            promisor,
            owner: UserId::from_uuid(Uuid::from_u128(1)),
            workstream,
            due_date: None,
            condition: None,
        }
    }

    #[test]
    fn changed_record_of_each_change() {
        let action = ActionId::from_uuid(Uuid::from_u128(30));
        let commitment = CommitmentId::from_uuid(Uuid::from_u128(31));
        let status = Operation::ChangeActionStatus {
            event_id: event(),
            action_id: action,
            status: ActionStatus::Done,
            expected_version: version(),
        };
        let due = Operation::ChangeActionDue {
            event_id: event(),
            action_id: action,
            due_date: None,
            expected_version: version(),
        };
        let promise = Operation::ChangeCommitmentStatus {
            event_id: event(),
            commitment_id: commitment,
            status: CommitmentStatus::Firm,
            expected_version: version(),
        };
        assert_eq!(status.changed_record(), Some(RecordRef::Action(action)));
        assert_eq!(due.changed_record(), Some(RecordRef::Action(action)));
        assert_eq!(
            promise.changed_record(),
            Some(RecordRef::Commitment(commitment))
        );
        for change in [&status, &due, &promise] {
            assert_eq!(change.workstream(), None);
            assert!(!change.managers_only());
        }

        // A create changes no record. It names its workstream instead.
        let workstream = WorkstreamId::from_uuid(Uuid::from_u128(40));
        let person = Party::Person(PersonId::from_uuid(Uuid::from_u128(60)));
        let create = new_commitment(person, Some(workstream));
        assert_eq!(create.changed_record(), None);
        assert_eq!(create.workstream(), Some(workstream));
        assert_eq!(new_commitment(person, None).workstream(), None);
        assert!(!create.managers_only());
    }

    #[test]
    fn a_new_workstream_changes_no_record_and_names_no_workstream() {
        let workstream = WorkstreamId::from_uuid(Uuid::from_u128(40));
        let create = Operation::CreateWorkstream {
            id: workstream,
            event_id: event(),
            name: WorkstreamName::parse("Programm").unwrap(),
            lead: UserId::from_uuid(Uuid::from_u128(1)),
        };
        assert_eq!(create.changed_record(), None);
        assert_eq!(create.workstream(), None);
        assert_eq!(create.new_record(), Some(NewRecord::Workstream(workstream)));
        assert_eq!(create.event_id(), Some(event()));
    }

    #[test]
    fn references_of_a_commitment_name_its_promisor() {
        let uuid = Uuid::from_u128(60);
        let person = new_commitment(Party::Person(PersonId::from_uuid(uuid)), None);
        assert_eq!(
            person.references(),
            [NewRecord::Person(PersonId::from_uuid(uuid))]
        );
        let institution = new_commitment(Party::Institution(InstitutionId::from_uuid(uuid)), None);
        assert_eq!(
            institution.references(),
            [NewRecord::Institution(InstitutionId::from_uuid(uuid))]
        );
        let deprecation = Operation::DeprecateField {
            event_id: event(),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(50)),
        };
        assert!(deprecation.references().is_empty());
    }

    #[test]
    fn an_action_names_its_milestone_and_a_milestone_change_changes_the_action() {
        let milestone = MilestoneId::from_uuid(Uuid::from_u128(70));
        let action = ActionId::from_uuid(Uuid::from_u128(30));
        let create = Operation::CreateAction {
            id: action,
            event_id: event(),
            title: ActionTitle::parse("Zelt bestellen").unwrap(),
            description: None,
            owner: UserId::from_uuid(Uuid::from_u128(1)),
            workstream: None,
            start_date: None,
            due_date: None,
            milestone: Some(milestone),
        };
        assert_eq!(create.references(), [NewRecord::Milestone(milestone)]);
        let change = Operation::ChangeActionMilestone {
            event_id: event(),
            action_id: action,
            milestone: Some(milestone),
            expected_version: version(),
        };
        assert_eq!(change.references(), [NewRecord::Milestone(milestone)]);
        assert_eq!(change.changed_record(), Some(RecordRef::Action(action)));
        let cleared = Operation::ChangeActionMilestone {
            event_id: event(),
            action_id: action,
            milestone: None,
            expected_version: version(),
        };
        assert!(cleared.references().is_empty());
    }

    #[test]
    fn each_risk_operation_is_a_normal_work_record_operation() {
        let risk = RiskId::from_uuid(Uuid::from_u128(90));
        let workstream = WorkstreamId::from_uuid(Uuid::from_u128(40));
        let rating = RiskRating {
            likelihood: crate::records::Level::Medium,
            impact: crate::records::Level::High,
        };
        let create = Operation::CreateRisk {
            id: risk,
            event_id: event(),
            title: RiskTitle::parse("Starkregen am Festtag").unwrap(),
            description: None,
            rating,
            mitigation: None,
            owner: UserId::from_uuid(Uuid::from_u128(1)),
            workstream: Some(workstream),
        };
        assert_eq!(create.new_record(), Some(NewRecord::Risk(risk)));
        assert_eq!(create.references(), [NewRecord::Workstream(workstream)]);
        assert_eq!(create.workstream(), Some(workstream));
        assert_eq!(create.changed_record(), None);
        assert!(create.new_work_record().is_some());
        let status = Operation::ChangeRiskStatus {
            event_id: event(),
            risk_id: risk,
            status: RiskStatus::Mitigated,
            expected_version: version(),
        };
        let rating_change = Operation::ChangeRiskRating {
            event_id: event(),
            risk_id: risk,
            rating,
            expected_version: version(),
        };
        for change in [&status, &rating_change] {
            assert_eq!(change.changed_record(), Some(RecordRef::Risk(risk)));
            assert_eq!(change.expected_version(), Some(version()));
            assert_eq!(change.new_record(), None);
            assert!(change.references().is_empty());
            assert!(change.new_work_record().is_none());
        }
        // Rules 1 to 3 route a risk; no operation of a risk goes to the managers only.
        for operation in [&create, &status, &rating_change] {
            assert!(!operation.managers_only());
            assert_eq!(operation.event_id(), Some(event()));
        }
    }

    #[test]
    fn each_decision_operation_goes_to_the_managers_and_names_its_new_records() {
        let (old, new) = (
            DecisionId::from_uuid(Uuid::from_u128(80)),
            DecisionId::from_uuid(Uuid::from_u128(81)),
        );
        let (person, institution) = (
            PersonId::from_uuid(Uuid::from_u128(60)),
            InstitutionId::from_uuid(Uuid::from_u128(61)),
        );
        let workstream = WorkstreamId::from_uuid(Uuid::from_u128(40));
        let create = Operation::CreateDecision {
            id: new,
            event_id: event(),
            wording: DecisionWording::parse("Das Fest findet bei jedem Wetter statt.").unwrap(),
            decided_on: jiff::civil::date(2030, 3, 1),
            decided_by: DecisionParties::new(vec![
                Party::Person(person),
                Party::Institution(institution),
            ])
            .unwrap(),
            owner: UserId::from_uuid(Uuid::from_u128(1)),
            workstream: Some(workstream),
        };
        assert_eq!(
            create.references(),
            [
                NewRecord::Workstream(workstream),
                NewRecord::Person(person),
                NewRecord::Institution(institution),
            ]
        );
        assert_eq!(create.new_record(), Some(NewRecord::Decision(new)));
        assert_eq!(create.workstream(), Some(workstream));
        assert_eq!(create.changed_record(), None);
        let supersede = Operation::SupersedeDecision {
            event_id: event(),
            decision_id: old,
            successor: new,
            expected_version: version(),
        };
        assert_eq!(supersede.references(), [NewRecord::Decision(new)]);
        assert_eq!(supersede.changed_record(), Some(RecordRef::Decision(old)));
        assert_eq!(supersede.expected_version(), Some(version()));
        let revoke = Operation::RevokeDecision {
            event_id: event(),
            decision_id: old,
            reason: RecordReason::parse("Der Vorstand hebt ihn auf.").unwrap(),
            expected_version: version(),
        };
        assert!(revoke.references().is_empty());
        assert_eq!(revoke.changed_record(), Some(RecordRef::Decision(old)));
        for operation in [&create, &supersede, &revoke] {
            assert!(operation.managers_only());
            assert_eq!(operation.event_id(), Some(event()));
        }
    }

    #[test]
    fn debug_hides_reasons_and_questions() {
        let reason = Reason::parse("Anna Muster said so").unwrap();
        assert!(!format!("{reason:?}").contains("Anna"));
        let question = QuestionText::parse("Does Anna Muster come?").unwrap();
        assert!(!format!("{question:?}").contains("Anna"));
    }
}
