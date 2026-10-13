//! The `Operation` of a proposal as the API shows it.

use serde::Serialize;
use tada_app::domain::parties::Party as DomainParty;
use tada_app::domain::proposals::{
    DraftDocument as DomainDraftDocument, Operation as DomainOperation,
};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::values::{FactState, Label, Value, ValueType, state_parts};

/// The change that a proposal suggests. `kind` names the operation. The list of kinds is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Operation {
    CreateEvent {
        id: Uuid,
        key: String,
        name: String,
        time_zone: String,
    },
    SetFact {
        event_id: Uuid,
        field_id: Uuid,
        state: FactState,
        /// The proposed value. It is absent if the proposed state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// True for an approximate value. It is absent if the proposed state is `unknown`.
        #[serde(skip_serializing_if = "Option::is_none")]
        approximate: Option<bool>,
        /// The fact version that the proposal expects. It is absent if the event had no fact of the field.
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_version: Option<i64>,
    },
    AddFieldDefinition {
        id: Uuid,
        event_id: Uuid,
        key: String,
        label: Label,
        value_type: ValueType,
        description: String,
        module: String,
    },
    AddChoiceValue {
        event_id: Uuid,
        field_id: Uuid,
        key: String,
        label: Label,
    },
    DeprecateField {
        event_id: Uuid,
        field_id: Uuid,
    },
    CreateOpenQuestion {
        id: Uuid,
        event_id: Uuid,
        text: String,
        /// The user ID of the member who owns the question.
        owner: Uuid,
    },
    CreateDocumentDraft {
        event_id: Uuid,
        document: DraftDocument,
        /// The Markdown of the draft.
        markdown: String,
    },
    /// A new person of the organization. It belongs to no event.
    CreatePerson {
        id: Uuid,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phone: Option<String>,
    },
    /// A new institution of the organization. It belongs to no event.
    CreateInstitution {
        id: Uuid,
        name: String,
        /// `authority`, `company`, `club` or `other`. `kind` names the operation, so this field has another name.
        institution_kind: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phone: Option<String>,
    },
    /// A new action of the event. It starts `open`.
    CreateAction {
        id: Uuid,
        event_id: Uuid,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// The user ID of the member who owns the action.
        owner: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workstream: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_date: Option<jiff::civil::Date>,
        #[serde(skip_serializing_if = "Option::is_none")]
        due_date: Option<jiff::civil::Date>,
        /// A milestone of the event, or a new milestone of the same changeset.
        #[serde(skip_serializing_if = "Option::is_none")]
        milestone: Option<Uuid>,
    },
    /// A new commitment of the event. With a condition it starts `conditional`, else `firm`.
    CreateCommitment {
        id: Uuid,
        event_id: Uuid,
        text: String,
        promisor: ProposalPromisor,
        /// The user ID of the member who follows the commitment up.
        owner: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workstream: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        due_date: Option<jiff::civil::Date>,
        #[serde(skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
    },
    ChangeActionStatus {
        event_id: Uuid,
        action_id: Uuid,
        /// `open`, `in-progress`, `blocked`, `done` or `canceled`.
        status: String,
        /// The version of the action that the proposal expects.
        expected_version: i64,
    },
    ChangeActionDue {
        event_id: Uuid,
        action_id: Uuid,
        /// The new due date. It is absent if the proposal clears the due date.
        #[serde(skip_serializing_if = "Option::is_none")]
        due_date: Option<jiff::civil::Date>,
        /// The version of the action that the proposal expects.
        expected_version: i64,
    },
    ChangeCommitmentStatus {
        event_id: Uuid,
        commitment_id: Uuid,
        /// `conditional`, `firm`, `fulfilled`, `broken` or `withdrawn`.
        status: String,
        /// The version of the commitment that the proposal expects.
        expected_version: i64,
    },
    /// A new workstream of the event. It starts `active`.
    CreateWorkstream {
        id: Uuid,
        event_id: Uuid,
        name: String,
        /// The user ID of the member who leads the workstream.
        lead: Uuid,
    },
    /// A new milestone of the event. It starts `open`.
    CreateMilestone {
        id: Uuid,
        event_id: Uuid,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        date: jiff::civil::Date,
        /// The user ID of the member who owns the milestone.
        owner: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workstream: Option<Uuid>,
    },
    ChangeMilestoneStatus {
        event_id: Uuid,
        milestone_id: Uuid,
        /// `open`, `reached` or `canceled`.
        status: String,
        /// The version of the milestone that the proposal expects.
        expected_version: i64,
    },
    ChangeActionMilestone {
        event_id: Uuid,
        action_id: Uuid,
        /// The new milestone. It is absent if the proposal clears the milestone.
        #[serde(skip_serializing_if = "Option::is_none")]
        milestone: Option<Uuid>,
        /// The version of the action that the proposal expects.
        expected_version: i64,
    },
    /// A new decision of the event. It starts `in-force`. Only the event managers review it.
    CreateDecision {
        id: Uuid,
        event_id: Uuid,
        /// The words of the decision. It can have line breaks.
        wording: String,
        decided_on: jiff::civil::Date,
        /// The persons and institutions that decided, in their order.
        decided_by: Vec<ProposalPromisor>,
        /// The user ID of the member who follows the decision up.
        owner: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workstream: Option<Uuid>,
    },
    /// The successor replaces a decision in force. Only the event managers review it.
    SupersedeDecision {
        event_id: Uuid,
        decision_id: Uuid,
        /// A decision of the event in force, or a new decision of the same changeset.
        successor: Uuid,
        /// The version of the decision that the proposal expects.
        expected_version: i64,
    },
    /// A decision in force ends without a successor. Only the event managers review it.
    RevokeDecision {
        event_id: Uuid,
        decision_id: Uuid,
        reason: String,
        /// The version of the decision that the proposal expects.
        expected_version: i64,
    },
    /// A new risk of the event. It starts `open`.
    CreateRisk {
        id: Uuid,
        event_id: Uuid,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// `low`, `medium` or `high`.
        likelihood: String,
        /// `low`, `medium` or `high`.
        impact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mitigation: Option<String>,
        /// The user ID of the member who owns the risk.
        owner: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workstream: Option<Uuid>,
    },
    ChangeRiskStatus {
        event_id: Uuid,
        risk_id: Uuid,
        /// `open`, `mitigated`, `occurred` or `closed`.
        status: String,
        /// The version of the risk that the proposal expects.
        expected_version: i64,
    },
    ChangeRiskRating {
        event_id: Uuid,
        risk_id: Uuid,
        /// `low`, `medium` or `high`.
        likelihood: String,
        /// `low`, `medium` or `high`.
        impact: String,
        /// The version of the risk that the proposal expects.
        expected_version: i64,
    },
}

/// The party that makes a commitment: `{"person": "<uuid>"}` or `{"institution": "<uuid>"}`.
/// It can be a new person or institution of the same changeset.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalPromisor {
    Person(Uuid),
    Institution(Uuid),
}

impl From<DomainParty> for ProposalPromisor {
    fn from(party: DomainParty) -> Self {
        match party {
            DomainParty::Person(id) => Self::Person(id.as_uuid()),
            DomainParty::Institution(id) => Self::Institution(id.as_uuid()),
        }
    }
}

/// The document of a draft.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DraftDocument {
    /// A new document of the event.
    New { id: Uuid, name: String },
    /// An existing document of the event.
    Existing {
        document_id: Uuid,
        /// The record version of the document that the proposal expects.
        expected_version: i64,
    },
}

impl From<&DomainDraftDocument> for DraftDocument {
    fn from(document: &DomainDraftDocument) -> Self {
        match document {
            DomainDraftDocument::New { id, name } => Self::New {
                id: id.as_uuid(),
                name: name.as_str().to_owned(),
            },
            DomainDraftDocument::Existing {
                document_id,
                expected_version,
            } => Self::Existing {
                document_id: document_id.as_uuid(),
                expected_version: expected_version.get(),
            },
        }
    }
}

impl From<&DomainOperation> for Operation {
    fn from(operation: &DomainOperation) -> Self {
        let text_label = |text: &tada_app::domain::facts::ShortText| Label::Text {
            text: text.as_str().to_owned(),
        };
        match operation {
            DomainOperation::CreateEvent {
                id,
                key,
                name,
                time_zone,
            } => Self::CreateEvent {
                id: id.as_uuid(),
                key: key.as_str().to_owned(),
                name: name.as_str().to_owned(),
                time_zone: time_zone.as_str().to_owned(),
            },
            DomainOperation::SetFact {
                event_id,
                field_id,
                state,
                expected_version,
            } => {
                let (state, value, approximate) = state_parts(state);
                Self::SetFact {
                    event_id: event_id.as_uuid(),
                    field_id: field_id.as_uuid(),
                    state,
                    value,
                    approximate,
                    expected_version: expected_version.map(|version| version.get()),
                }
            }
            DomainOperation::AddFieldDefinition {
                id,
                event_id,
                key,
                label,
                value_type,
                description,
                module,
            } => Self::AddFieldDefinition {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                key: key.as_str().to_owned(),
                label: text_label(label),
                value_type: value_type.into(),
                description: description.as_str().to_owned(),
                module: module.as_str().to_owned(),
            },
            DomainOperation::AddChoiceValue {
                event_id,
                field_id,
                key,
                label,
            } => Self::AddChoiceValue {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
                key: key.as_str().to_owned(),
                label: text_label(label),
            },
            DomainOperation::DeprecateField { event_id, field_id } => Self::DeprecateField {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
            },
            DomainOperation::CreateOpenQuestion {
                id,
                event_id,
                text,
                owner,
            } => Self::CreateOpenQuestion {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                text: text.as_str().to_owned(),
                owner: owner.as_uuid(),
            },
            DomainOperation::CreateDocumentDraft {
                event_id,
                document,
                markdown,
            } => Self::CreateDocumentDraft {
                event_id: event_id.as_uuid(),
                document: document.into(),
                markdown: markdown.as_str().to_owned(),
            },
            DomainOperation::CreatePerson {
                id,
                name,
                email,
                phone,
            } => Self::CreatePerson {
                id: id.as_uuid(),
                name: name.as_str().to_owned(),
                email: email.as_ref().map(|email| email.as_str().to_owned()),
                phone: phone.as_ref().map(|phone| phone.as_str().to_owned()),
            },
            DomainOperation::CreateInstitution {
                id,
                name,
                kind,
                email,
                phone,
            } => Self::CreateInstitution {
                id: id.as_uuid(),
                name: name.as_str().to_owned(),
                institution_kind: kind.as_str().to_owned(),
                email: email.as_ref().map(|email| email.as_str().to_owned()),
                phone: phone.as_ref().map(|phone| phone.as_str().to_owned()),
            },
            DomainOperation::CreateAction {
                id,
                event_id,
                title,
                description,
                owner,
                workstream,
                start_date,
                due_date,
                milestone,
            } => Self::CreateAction {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                title: title.as_str().to_owned(),
                description: description.as_ref().map(|text| text.as_str().to_owned()),
                owner: owner.as_uuid(),
                workstream: workstream.map(|id| id.as_uuid()),
                start_date: *start_date,
                due_date: *due_date,
                milestone: milestone.map(|id| id.as_uuid()),
            },
            DomainOperation::CreateCommitment {
                id,
                event_id,
                text,
                promisor,
                owner,
                workstream,
                due_date,
                condition,
            } => Self::CreateCommitment {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                text: text.as_str().to_owned(),
                promisor: (*promisor).into(),
                owner: owner.as_uuid(),
                workstream: workstream.map(|id| id.as_uuid()),
                due_date: *due_date,
                condition: condition.as_ref().map(|text| text.as_str().to_owned()),
            },
            DomainOperation::ChangeActionStatus {
                event_id,
                action_id,
                status,
                expected_version,
            } => Self::ChangeActionStatus {
                event_id: event_id.as_uuid(),
                action_id: action_id.as_uuid(),
                status: status.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
            DomainOperation::ChangeActionDue {
                event_id,
                action_id,
                due_date,
                expected_version,
            } => Self::ChangeActionDue {
                event_id: event_id.as_uuid(),
                action_id: action_id.as_uuid(),
                due_date: *due_date,
                expected_version: expected_version.get(),
            },
            DomainOperation::ChangeActionMilestone {
                event_id,
                action_id,
                milestone,
                expected_version,
            } => Self::ChangeActionMilestone {
                event_id: event_id.as_uuid(),
                action_id: action_id.as_uuid(),
                milestone: milestone.map(|id| id.as_uuid()),
                expected_version: expected_version.get(),
            },
            DomainOperation::ChangeCommitmentStatus {
                event_id,
                commitment_id,
                status,
                expected_version,
            } => Self::ChangeCommitmentStatus {
                event_id: event_id.as_uuid(),
                commitment_id: commitment_id.as_uuid(),
                status: status.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
            DomainOperation::CreateWorkstream {
                id,
                event_id,
                name,
                lead,
            } => Self::CreateWorkstream {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                name: name.as_str().to_owned(),
                lead: lead.as_uuid(),
            },
            DomainOperation::CreateMilestone {
                id,
                event_id,
                title,
                description,
                date,
                owner,
                workstream,
            } => Self::CreateMilestone {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                title: title.as_str().to_owned(),
                description: description.as_ref().map(|text| text.as_str().to_owned()),
                date: *date,
                owner: owner.as_uuid(),
                workstream: workstream.map(|id| id.as_uuid()),
            },
            DomainOperation::ChangeMilestoneStatus {
                event_id,
                milestone_id,
                status,
                expected_version,
            } => Self::ChangeMilestoneStatus {
                event_id: event_id.as_uuid(),
                milestone_id: milestone_id.as_uuid(),
                status: status.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
            DomainOperation::CreateDecision {
                id,
                event_id,
                wording,
                decided_on,
                decided_by,
                owner,
                workstream,
            } => Self::CreateDecision {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                wording: wording.as_str().to_owned(),
                decided_on: *decided_on,
                decided_by: decided_by
                    .as_slice()
                    .iter()
                    .copied()
                    .map(ProposalPromisor::from)
                    .collect(),
                owner: owner.as_uuid(),
                workstream: workstream.map(|id| id.as_uuid()),
            },
            DomainOperation::SupersedeDecision {
                event_id,
                decision_id,
                successor,
                expected_version,
            } => Self::SupersedeDecision {
                event_id: event_id.as_uuid(),
                decision_id: decision_id.as_uuid(),
                successor: successor.as_uuid(),
                expected_version: expected_version.get(),
            },
            DomainOperation::RevokeDecision {
                event_id,
                decision_id,
                reason,
                expected_version,
            } => Self::RevokeDecision {
                event_id: event_id.as_uuid(),
                decision_id: decision_id.as_uuid(),
                reason: reason.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
            DomainOperation::CreateRisk {
                id,
                event_id,
                title,
                description,
                rating,
                mitigation,
                owner,
                workstream,
            } => Self::CreateRisk {
                id: id.as_uuid(),
                event_id: event_id.as_uuid(),
                title: title.as_str().to_owned(),
                description: description.as_ref().map(|text| text.as_str().to_owned()),
                likelihood: rating.likelihood.as_str().to_owned(),
                impact: rating.impact.as_str().to_owned(),
                mitigation: mitigation.as_ref().map(|text| text.as_str().to_owned()),
                owner: owner.as_uuid(),
                workstream: workstream.map(|id| id.as_uuid()),
            },
            DomainOperation::ChangeRiskStatus {
                event_id,
                risk_id,
                status,
                expected_version,
            } => Self::ChangeRiskStatus {
                event_id: event_id.as_uuid(),
                risk_id: risk_id.as_uuid(),
                status: status.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
            DomainOperation::ChangeRiskRating {
                event_id,
                risk_id,
                rating,
                expected_version,
            } => Self::ChangeRiskRating {
                event_id: event_id.as_uuid(),
                risk_id: risk_id.as_uuid(),
                likelihood: rating.likelihood.as_str().to_owned(),
                impact: rating.impact.as_str().to_owned(),
                expected_version: expected_version.get(),
            },
        }
    }
}
