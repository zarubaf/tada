//! The stored form of an operation is the `OperationRecord` of this code module, in the format `OPERATION_VERSION`.
//! A change of the format needs a new version, and this codec must read each older version.

use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use tada_app::domain::RecordVersion;
use tada_app::domain::documents::{DocumentName, DraftMarkdown};
use tada_app::domain::events::{EventKey, EventName, EventTimeZone};
use tada_app::domain::facts::{ChoiceKey, Description, FieldKey, ModuleKey, ShortText};
use tada_app::domain::identity::Email;
use tada_app::domain::ids::{
    ActionId, CommitmentId, DocumentId, EventId, FieldDefinitionId, InstitutionId, MilestoneId,
    OpenQuestionId, PersonId, UserId, WorkstreamId,
};
use tada_app::domain::parties::{InstitutionKind, Party, PartyName, PhoneNumber};
use tada_app::domain::proposals::{DraftDocument, Operation, QuestionText};
use tada_app::domain::records::{MilestoneStatus, MilestoneTitle};
use tada_app::domain::work::{
    ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText, RecordDescription,
    WorkstreamName,
};

use crate::error::InvalidRow;
use crate::values;

/// The format of `proposal.operation`.
pub(super) const OPERATION_VERSION: i32 = 1;

pub(super) const OPERATION: &str = "proposal.operation";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OperationRecord {
    CreateEvent {
        id: Uuid,
        key: String,
        name: String,
        time_zone: String,
    },
    SetFact {
        event_id: Uuid,
        field_id: Uuid,
        state: String,
        value: Option<serde_json::Value>,
        approximate: bool,
        expected_version: Option<i64>,
    },
    AddFieldDefinition {
        id: Uuid,
        event_id: Uuid,
        key: String,
        label: String,
        value_type: serde_json::Value,
        description: String,
        module: String,
    },
    AddChoiceValue {
        event_id: Uuid,
        field_id: Uuid,
        key: String,
        label: String,
    },
    DeprecateField {
        event_id: Uuid,
        field_id: Uuid,
    },
    CreateOpenQuestion {
        id: Uuid,
        event_id: Uuid,
        text: String,
        owner: Uuid,
    },
    CreateDocumentDraft {
        event_id: Uuid,
        document: DocumentRecord,
        markdown: String,
    },
    CreatePerson {
        id: Uuid,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    },
    CreateInstitution {
        id: Uuid,
        name: String,
        institution_kind: String,
        email: Option<String>,
        phone: Option<String>,
    },
    /// `start_date` and `milestone` are absent in the records of older releases; they read as `None`.
    CreateAction {
        id: Uuid,
        event_id: Uuid,
        title: String,
        description: Option<String>,
        owner: Uuid,
        workstream: Option<Uuid>,
        due_date: Option<String>,
        start_date: Option<String>,
        milestone: Option<Uuid>,
    },
    CreateCommitment {
        id: Uuid,
        event_id: Uuid,
        text: String,
        promisor: PartyRecord,
        owner: Uuid,
        workstream: Option<Uuid>,
        due_date: Option<String>,
        condition: Option<String>,
    },
    ChangeActionStatus {
        event_id: Uuid,
        action_id: Uuid,
        status: String,
        expected_version: i64,
    },
    ChangeActionDue {
        event_id: Uuid,
        action_id: Uuid,
        due_date: Option<String>,
        expected_version: i64,
    },
    ChangeActionMilestone {
        event_id: Uuid,
        action_id: Uuid,
        milestone: Option<Uuid>,
        expected_version: i64,
    },
    ChangeCommitmentStatus {
        event_id: Uuid,
        commitment_id: Uuid,
        status: String,
        expected_version: i64,
    },
    CreateWorkstream {
        id: Uuid,
        event_id: Uuid,
        name: String,
        lead: Uuid,
    },
    CreateMilestone {
        id: Uuid,
        event_id: Uuid,
        title: String,
        description: Option<String>,
        date: String,
        owner: Uuid,
        workstream: Option<Uuid>,
    },
    ChangeMilestoneStatus {
        event_id: Uuid,
        milestone_id: Uuid,
        status: String,
        expected_version: i64,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
enum PartyRecord {
    Person(Uuid),
    Institution(Uuid),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DocumentRecord {
    New {
        id: Uuid,
        name: String,
    },
    Existing {
        document_id: Uuid,
        expected_version: i64,
    },
}

pub(crate) fn operation_to_json(operation: &Operation) -> serde_json::Value {
    let record = match operation {
        Operation::CreateEvent {
            id,
            key,
            name,
            time_zone,
        } => OperationRecord::CreateEvent {
            id: id.as_uuid(),
            key: key.as_str().to_owned(),
            name: name.as_str().to_owned(),
            time_zone: time_zone.as_str().to_owned(),
        },
        Operation::SetFact {
            event_id,
            field_id,
            state,
            expected_version,
        } => {
            let (state, value, approximate) = values::fact_state_to_columns(state);
            OperationRecord::SetFact {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
                state: state.to_owned(),
                value,
                approximate,
                expected_version: expected_version.map(RecordVersion::get),
            }
        }
        Operation::AddFieldDefinition {
            id,
            event_id,
            key,
            label,
            value_type,
            description,
            module,
        } => OperationRecord::AddFieldDefinition {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            key: key.as_str().to_owned(),
            label: label.as_str().to_owned(),
            value_type: values::value_type_to_json(value_type),
            description: description.as_str().to_owned(),
            module: module.as_str().to_owned(),
        },
        Operation::AddChoiceValue {
            event_id,
            field_id,
            key,
            label,
        } => OperationRecord::AddChoiceValue {
            event_id: event_id.as_uuid(),
            field_id: field_id.as_uuid(),
            key: key.as_str().to_owned(),
            label: label.as_str().to_owned(),
        },
        Operation::DeprecateField { event_id, field_id } => OperationRecord::DeprecateField {
            event_id: event_id.as_uuid(),
            field_id: field_id.as_uuid(),
        },
        Operation::CreateOpenQuestion {
            id,
            event_id,
            text,
            owner,
        } => OperationRecord::CreateOpenQuestion {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            text: text.as_str().to_owned(),
            owner: owner.as_uuid(),
        },
        Operation::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } => OperationRecord::CreateDocumentDraft {
            event_id: event_id.as_uuid(),
            document: match document {
                DraftDocument::New { id, name } => DocumentRecord::New {
                    id: id.as_uuid(),
                    name: name.as_str().to_owned(),
                },
                DraftDocument::Existing {
                    document_id,
                    expected_version,
                } => DocumentRecord::Existing {
                    document_id: document_id.as_uuid(),
                    expected_version: expected_version.get(),
                },
            },
            markdown: markdown.as_str().to_owned(),
        },
        Operation::CreatePerson {
            id,
            name,
            email,
            phone,
        } => OperationRecord::CreatePerson {
            id: id.as_uuid(),
            name: name.as_str().to_owned(),
            email: email.as_ref().map(|email| email.as_str().to_owned()),
            phone: phone.as_ref().map(|phone| phone.as_str().to_owned()),
        },
        Operation::CreateInstitution {
            id,
            name,
            kind,
            email,
            phone,
        } => OperationRecord::CreateInstitution {
            id: id.as_uuid(),
            name: name.as_str().to_owned(),
            institution_kind: kind.as_str().to_owned(),
            email: email.as_ref().map(|email| email.as_str().to_owned()),
            phone: phone.as_ref().map(|phone| phone.as_str().to_owned()),
        },
        Operation::CreateAction {
            id,
            event_id,
            title,
            description,
            owner,
            workstream,
            start_date,
            due_date,
            milestone,
        } => OperationRecord::CreateAction {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            title: title.as_str().to_owned(),
            description: description.as_ref().map(|text| text.as_str().to_owned()),
            owner: owner.as_uuid(),
            workstream: workstream.map(WorkstreamId::as_uuid),
            due_date: due_date.map(|date| date.to_string()),
            start_date: start_date.map(|date| date.to_string()),
            milestone: milestone.map(MilestoneId::as_uuid),
        },
        Operation::CreateCommitment {
            id,
            event_id,
            text,
            promisor,
            owner,
            workstream,
            due_date,
            condition,
        } => OperationRecord::CreateCommitment {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            text: text.as_str().to_owned(),
            promisor: match promisor {
                Party::Person(id) => PartyRecord::Person(id.as_uuid()),
                Party::Institution(id) => PartyRecord::Institution(id.as_uuid()),
            },
            owner: owner.as_uuid(),
            workstream: workstream.map(WorkstreamId::as_uuid),
            due_date: due_date.map(|date| date.to_string()),
            condition: condition.as_ref().map(|text| text.as_str().to_owned()),
        },
        Operation::ChangeActionStatus {
            event_id,
            action_id,
            status,
            expected_version,
        } => OperationRecord::ChangeActionStatus {
            event_id: event_id.as_uuid(),
            action_id: action_id.as_uuid(),
            status: status.as_str().to_owned(),
            expected_version: expected_version.get(),
        },
        Operation::ChangeActionDue {
            event_id,
            action_id,
            due_date,
            expected_version,
        } => OperationRecord::ChangeActionDue {
            event_id: event_id.as_uuid(),
            action_id: action_id.as_uuid(),
            due_date: due_date.map(|date| date.to_string()),
            expected_version: expected_version.get(),
        },
        Operation::ChangeActionMilestone {
            event_id,
            action_id,
            milestone,
            expected_version,
        } => OperationRecord::ChangeActionMilestone {
            event_id: event_id.as_uuid(),
            action_id: action_id.as_uuid(),
            milestone: milestone.map(MilestoneId::as_uuid),
            expected_version: expected_version.get(),
        },
        Operation::ChangeCommitmentStatus {
            event_id,
            commitment_id,
            status,
            expected_version,
        } => OperationRecord::ChangeCommitmentStatus {
            event_id: event_id.as_uuid(),
            commitment_id: commitment_id.as_uuid(),
            status: status.as_str().to_owned(),
            expected_version: expected_version.get(),
        },
        Operation::CreateWorkstream {
            id,
            event_id,
            name,
            lead,
        } => OperationRecord::CreateWorkstream {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            name: name.as_str().to_owned(),
            lead: lead.as_uuid(),
        },
        Operation::CreateMilestone {
            id,
            event_id,
            title,
            description,
            date,
            owner,
            workstream,
        } => OperationRecord::CreateMilestone {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            title: title.as_str().to_owned(),
            description: description.as_ref().map(|text| text.as_str().to_owned()),
            date: date.to_string(),
            owner: owner.as_uuid(),
            workstream: workstream.map(WorkstreamId::as_uuid),
        },
        Operation::ChangeMilestoneStatus {
            event_id,
            milestone_id,
            status,
            expected_version,
        } => OperationRecord::ChangeMilestoneStatus {
            event_id: event_id.as_uuid(),
            milestone_id: milestone_id.as_uuid(),
            status: status.as_str().to_owned(),
            expected_version: expected_version.get(),
        },
    };
    serde_json::to_value(record).expect("an operation record is valid JSON")
}

pub(crate) fn operation_from_json(
    version: i32,
    json: &serde_json::Value,
) -> Result<Operation, InvalidRow> {
    if version != OPERATION_VERSION {
        return Err(InvalidRow("proposal.operation_version"));
    }
    let invalid = || InvalidRow(OPERATION);
    let record = OperationRecord::deserialize(json).map_err(|_| invalid())?;
    Ok(match record {
        OperationRecord::CreateEvent {
            id,
            key,
            name,
            time_zone,
        } => Operation::CreateEvent {
            id: EventId::from_uuid(id),
            key: EventKey::parse(&key).map_err(|_| invalid())?,
            name: EventName::parse(&name).map_err(|_| invalid())?,
            time_zone: EventTimeZone::parse(&time_zone).map_err(|_| invalid())?,
        },
        OperationRecord::SetFact {
            event_id,
            field_id,
            state,
            value,
            approximate,
            expected_version,
        } => Operation::SetFact {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
            state: values::fact_state_from_columns(&state, value, approximate)
                .map_err(|_| invalid())?,
            expected_version: expected_version
                .map(|number| RecordVersion::new(number).ok_or_else(invalid))
                .transpose()?,
        },
        OperationRecord::AddFieldDefinition {
            id,
            event_id,
            key,
            label,
            value_type,
            description,
            module,
        } => Operation::AddFieldDefinition {
            id: FieldDefinitionId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            key: FieldKey::parse(&key).map_err(|_| invalid())?,
            label: ShortText::parse(&label).map_err(|_| invalid())?,
            value_type: values::value_type_from_json(&value_type).map_err(|_| invalid())?,
            description: Description::parse(&description).map_err(|_| invalid())?,
            module: ModuleKey::parse(&module).map_err(|_| invalid())?,
        },
        OperationRecord::AddChoiceValue {
            event_id,
            field_id,
            key,
            label,
        } => Operation::AddChoiceValue {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
            key: ChoiceKey::parse(&key).map_err(|_| invalid())?,
            label: ShortText::parse(&label).map_err(|_| invalid())?,
        },
        OperationRecord::DeprecateField { event_id, field_id } => Operation::DeprecateField {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
        },
        OperationRecord::CreateOpenQuestion {
            id,
            event_id,
            text,
            owner,
        } => Operation::CreateOpenQuestion {
            id: OpenQuestionId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            text: QuestionText::parse(&text).map_err(|_| invalid())?,
            owner: UserId::from_uuid(owner),
        },
        OperationRecord::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } => Operation::CreateDocumentDraft {
            event_id: EventId::from_uuid(event_id),
            document: match document {
                DocumentRecord::New { id, name } => DraftDocument::New {
                    id: DocumentId::from_uuid(id),
                    name: DocumentName::parse(&name).map_err(|_| invalid())?,
                },
                DocumentRecord::Existing {
                    document_id,
                    expected_version,
                } => DraftDocument::Existing {
                    document_id: DocumentId::from_uuid(document_id),
                    expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
                },
            },
            markdown: DraftMarkdown::parse(&markdown).map_err(|_| invalid())?,
        },
        OperationRecord::CreatePerson {
            id,
            name,
            email,
            phone,
        } => Operation::CreatePerson {
            id: PersonId::from_uuid(id),
            name: PartyName::parse(&name).map_err(|_| invalid())?,
            email: parsed(email, Email::parse)?,
            phone: parsed(phone, PhoneNumber::parse)?,
        },
        OperationRecord::CreateInstitution {
            id,
            name,
            institution_kind,
            email,
            phone,
        } => Operation::CreateInstitution {
            id: InstitutionId::from_uuid(id),
            name: PartyName::parse(&name).map_err(|_| invalid())?,
            kind: InstitutionKind::parse(&institution_kind).ok_or_else(invalid)?,
            email: parsed(email, Email::parse)?,
            phone: parsed(phone, PhoneNumber::parse)?,
        },
        OperationRecord::CreateAction {
            id,
            event_id,
            title,
            description,
            owner,
            workstream,
            due_date,
            start_date,
            milestone,
        } => Operation::CreateAction {
            id: ActionId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            title: ActionTitle::parse(&title).map_err(|_| invalid())?,
            description: parsed(description, RecordDescription::parse)?,
            owner: UserId::from_uuid(owner),
            workstream: workstream.map(WorkstreamId::from_uuid),
            start_date: parsed(start_date, |text| text.parse())?,
            due_date: parsed(due_date, |text| text.parse())?,
            milestone: milestone.map(MilestoneId::from_uuid),
        },
        OperationRecord::CreateCommitment {
            id,
            event_id,
            text,
            promisor,
            owner,
            workstream,
            due_date,
            condition,
        } => Operation::CreateCommitment {
            id: CommitmentId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            text: CommitmentText::parse(&text).map_err(|_| invalid())?,
            promisor: match promisor {
                PartyRecord::Person(id) => Party::Person(PersonId::from_uuid(id)),
                PartyRecord::Institution(id) => Party::Institution(InstitutionId::from_uuid(id)),
            },
            owner: UserId::from_uuid(owner),
            workstream: workstream.map(WorkstreamId::from_uuid),
            due_date: parsed(due_date, |text| text.parse())?,
            condition: parsed(condition, ConditionText::parse)?,
        },
        OperationRecord::ChangeActionStatus {
            event_id,
            action_id,
            status,
            expected_version,
        } => Operation::ChangeActionStatus {
            event_id: EventId::from_uuid(event_id),
            action_id: ActionId::from_uuid(action_id),
            status: ActionStatus::parse(&status).ok_or_else(invalid)?,
            expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
        },
        OperationRecord::ChangeActionDue {
            event_id,
            action_id,
            due_date,
            expected_version,
        } => Operation::ChangeActionDue {
            event_id: EventId::from_uuid(event_id),
            action_id: ActionId::from_uuid(action_id),
            due_date: parsed(due_date, |text| text.parse())?,
            expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
        },
        OperationRecord::ChangeActionMilestone {
            event_id,
            action_id,
            milestone,
            expected_version,
        } => Operation::ChangeActionMilestone {
            event_id: EventId::from_uuid(event_id),
            action_id: ActionId::from_uuid(action_id),
            milestone: milestone.map(MilestoneId::from_uuid),
            expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
        },
        OperationRecord::ChangeCommitmentStatus {
            event_id,
            commitment_id,
            status,
            expected_version,
        } => Operation::ChangeCommitmentStatus {
            event_id: EventId::from_uuid(event_id),
            commitment_id: CommitmentId::from_uuid(commitment_id),
            status: CommitmentStatus::parse(&status).ok_or_else(invalid)?,
            expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
        },
        OperationRecord::CreateWorkstream {
            id,
            event_id,
            name,
            lead,
        } => Operation::CreateWorkstream {
            id: WorkstreamId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            name: WorkstreamName::parse(&name).map_err(|_| invalid())?,
            lead: UserId::from_uuid(lead),
        },
        OperationRecord::CreateMilestone {
            id,
            event_id,
            title,
            description,
            date,
            owner,
            workstream,
        } => Operation::CreateMilestone {
            id: MilestoneId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            title: MilestoneTitle::parse(&title).map_err(|_| invalid())?,
            description: parsed(description, RecordDescription::parse)?,
            date: date.parse().map_err(|_| invalid())?,
            owner: UserId::from_uuid(owner),
            workstream: workstream.map(WorkstreamId::from_uuid),
        },
        OperationRecord::ChangeMilestoneStatus {
            event_id,
            milestone_id,
            status,
            expected_version,
        } => Operation::ChangeMilestoneStatus {
            event_id: EventId::from_uuid(event_id),
            milestone_id: MilestoneId::from_uuid(milestone_id),
            status: MilestoneStatus::parse(&status).ok_or_else(invalid)?,
            expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
        },
    })
}

/// Parses an optional stored text. Any error is an invalid operation record.
fn parsed<T, E>(
    text: Option<String>,
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> Result<Option<T>, InvalidRow> {
    text.map(|text| parse(&text).map_err(|_| InvalidRow(OPERATION)))
        .transpose()
}

/// The columns `target_kind`, `target_id` and `expected_version` of a proposal.
/// A fact has no ID before its first version, so the target of a fact is its field.
pub(super) fn target(operation: &Operation) -> (&'static str, Uuid, Option<i64>) {
    match operation {
        Operation::CreateEvent { id, .. } => ("event", id.as_uuid(), None),
        Operation::SetFact {
            field_id,
            expected_version,
            ..
        } => (
            "fact",
            field_id.as_uuid(),
            expected_version.map(RecordVersion::get),
        ),
        Operation::AddFieldDefinition { id, .. } => ("field_definition", id.as_uuid(), None),
        Operation::AddChoiceValue { field_id, .. } | Operation::DeprecateField { field_id, .. } => {
            ("field_definition", field_id.as_uuid(), None)
        }
        Operation::CreateOpenQuestion { id, .. } => ("open_question", id.as_uuid(), None),
        Operation::CreatePerson { id, .. } => ("person", id.as_uuid(), None),
        Operation::CreateInstitution { id, .. } => ("institution", id.as_uuid(), None),
        Operation::CreateAction { id, .. } => ("action", id.as_uuid(), None),
        Operation::CreateCommitment { id, .. } => ("commitment", id.as_uuid(), None),
        Operation::CreateWorkstream { id, .. } => ("workstream", id.as_uuid(), None),
        Operation::ChangeActionStatus {
            action_id,
            expected_version,
            ..
        }
        | Operation::ChangeActionDue {
            action_id,
            expected_version,
            ..
        }
        | Operation::ChangeActionMilestone {
            action_id,
            expected_version,
            ..
        } => ("action", action_id.as_uuid(), Some(expected_version.get())),
        Operation::ChangeCommitmentStatus {
            commitment_id,
            expected_version,
            ..
        } => (
            "commitment",
            commitment_id.as_uuid(),
            Some(expected_version.get()),
        ),
        Operation::CreateMilestone { id, .. } => ("milestone", id.as_uuid(), None),
        Operation::ChangeMilestoneStatus {
            milestone_id,
            expected_version,
            ..
        } => (
            "milestone",
            milestone_id.as_uuid(),
            Some(expected_version.get()),
        ),
        Operation::CreateDocumentDraft { document, .. } => match document {
            DraftDocument::New { id, .. } => ("document", id.as_uuid(), None),
            DraftDocument::Existing {
                document_id,
                expected_version,
            } => (
                "document",
                document_id.as_uuid(),
                Some(expected_version.get()),
            ),
        },
    }
}
