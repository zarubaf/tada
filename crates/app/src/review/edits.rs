//! Reviewer edits of the proposals that create work records and parties (ADR 0068, ADR 0069).
//!
//! An edit replaces fields of the proposed record before the acceptance. Each new value passes the checks of the
//! direct command; `check_edits` checks an edited owner and an edited workstream against the event.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use tada_domain::identity::Email;
use tada_domain::ids::{UserId, WorkstreamId};
use tada_domain::parties::{PartyName, PhoneNumber};
use tada_domain::proposals::Operation;
use tada_domain::work::{ActionDescription, ActionTitle, CommitmentText, ConditionText};
use uuid::Uuid;

use crate::parties::email_code;
use crate::problem::FieldError;
use crate::proposals::{parse_date, text_error_code};

/// The fields of a proposed record that the reviewer replaces. An absent field keeps the proposed value;
/// `null` clears an optional field.
///
/// - `create-action`: `title`, `description`, `due_date`, `owner`, `workstream`.
/// - `create-commitment`: `text`, `condition`, `due_date`, `owner`, `workstream`.
///   A condition makes the commitment start `conditional`. An edit cannot remove the condition of a proposed commitment.
/// - `create-person` and `create-institution`: `name`, `email`, `phone`.
#[derive(Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordEditInput {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub description: Option<Option<String>>,
    /// A date as `YYYY-MM-DD`.
    #[serde(default, deserialize_with = "present")]
    pub due_date: Option<Option<String>>,
    /// The user ID of a contributor or a manager of the event.
    #[serde(default)]
    pub owner: Option<Uuid>,
    /// An active workstream of the event.
    #[serde(default, deserialize_with = "present")]
    pub workstream: Option<Option<Uuid>>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub condition: Option<Option<String>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub phone: Option<Option<String>>,
}

/// The edit contains the words of members, so `Debug` shows the name of the type only (ADR 0035).
impl fmt::Debug for RecordEditInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecordEditInput(..)")
    }
}

/// Tells an absent field from a field that is `null`: the first keeps the value, the second clears it.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

/// The fields of the operation that a reviewer can edit. Empty for an operation without a record edit.
pub(super) fn editable_fields(operation: &Operation) -> &'static [&'static str] {
    match operation {
        Operation::CreateAction { .. } => {
            &["title", "description", "due_date", "owner", "workstream"]
        }
        Operation::CreateCommitment { .. } => {
            &["text", "condition", "due_date", "owner", "workstream"]
        }
        Operation::CreatePerson { .. } | Operation::CreateInstitution { .. } => {
            &["name", "email", "phone"]
        }
        _ => &[],
    }
}

/// Collects the errors of one edit, with paths relative to its `fields`.
#[derive(Default)]
struct Errors(Vec<FieldError>);

impl Errors {
    fn push(&mut self, field: &'static str, code: &'static str) {
        self.0
            .push(FieldError::new(format!("fields/{field}"), code));
    }

    /// The new value, the current value if the edit leaves the field out, or the current value after an error.
    fn replace<T, E>(
        &mut self,
        field: &'static str,
        current: T,
        input: Option<String>,
        parse: impl FnOnce(&str) -> Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> T {
        match input {
            None => current,
            Some(text) => parse(&text).unwrap_or_else(|error| {
                self.push(field, code(error));
                current
            }),
        }
    }

    /// Like `replace`, for an optional field: `Some(None)` clears it.
    fn replace_optional<T, E>(
        &mut self,
        field: &'static str,
        current: Option<T>,
        input: Option<Option<String>>,
        parse: impl FnOnce(&str) -> Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> Option<T> {
        match input {
            None => current,
            Some(None) => None,
            Some(Some(text)) => match parse(&text) {
                Ok(value) => Some(value),
                Err(error) => {
                    self.push(field, code(error));
                    current
                }
            },
        }
    }
}

/// The operation with the edited fields. The errors name the fields under `fields/`.
/// A field that the operation does not have gives `not-editable`.
pub(super) fn edit_record(
    operation: &Operation,
    edit: RecordEditInput,
) -> Result<Operation, Vec<FieldError>> {
    let mut errors = Errors::default();
    let given = [
        ("title", edit.title.is_some()),
        ("description", edit.description.is_some()),
        ("due_date", edit.due_date.is_some()),
        ("owner", edit.owner.is_some()),
        ("workstream", edit.workstream.is_some()),
        ("text", edit.text.is_some()),
        ("condition", edit.condition.is_some()),
        ("name", edit.name.is_some()),
        ("email", edit.email.is_some()),
        ("phone", edit.phone.is_some()),
    ];
    let editable = editable_fields(operation);
    for (field, present) in given {
        if present && !editable.contains(&field) {
            errors.push(field, "not-editable");
        }
    }
    let RecordEditInput {
        title,
        description,
        due_date,
        owner,
        workstream,
        text,
        condition,
        name,
        email,
        phone,
    } = edit;
    let edited = match operation.clone() {
        Operation::CreateAction {
            id,
            event_id,
            title: current_title,
            description: current_description,
            owner: current_owner,
            workstream: current_workstream,
            due,
        } => Operation::CreateAction {
            id,
            event_id,
            title: errors.replace(
                "title",
                current_title,
                title,
                ActionTitle::parse,
                text_error_code,
            ),
            description: errors.replace_optional(
                "description",
                current_description,
                description,
                ActionDescription::parse,
                text_error_code,
            ),
            owner: owner.map_or(current_owner, UserId::from_uuid),
            workstream: workstream.map_or(current_workstream, |id| id.map(WorkstreamId::from_uuid)),
            due: errors.replace_optional("due_date", due, due_date, parse_date, |_| "date"),
        },
        Operation::CreateCommitment {
            id,
            event_id,
            text: current_text,
            promisor,
            owner: current_owner,
            workstream: current_workstream,
            due,
            condition: current_condition,
        } => {
            // The condition of a proposed commitment stays: only "make firm" ends it (ADR 0068).
            if current_condition.is_some() && matches!(condition, Some(None)) {
                errors.push("condition", "condition-fixed");
            }
            let condition = match condition {
                Some(None) => current_condition,
                other => errors.replace_optional(
                    "condition",
                    current_condition,
                    other,
                    ConditionText::parse,
                    text_error_code,
                ),
            };
            Operation::CreateCommitment {
                id,
                event_id,
                text: errors.replace(
                    "text",
                    current_text,
                    text,
                    CommitmentText::parse,
                    text_error_code,
                ),
                promisor,
                owner: owner.map_or(current_owner, UserId::from_uuid),
                workstream: workstream
                    .map_or(current_workstream, |id| id.map(WorkstreamId::from_uuid)),
                due: errors.replace_optional("due_date", due, due_date, parse_date, |_| "date"),
                condition,
            }
        }
        Operation::CreatePerson {
            id,
            name: current_name,
            email: current_email,
            phone: current_phone,
        } => Operation::CreatePerson {
            id,
            name: errors.replace(
                "name",
                current_name,
                name,
                PartyName::parse,
                text_error_code,
            ),
            email: errors.replace_optional("email", current_email, email, Email::parse, email_code),
            phone: errors.replace_optional(
                "phone",
                current_phone,
                phone,
                PhoneNumber::parse,
                text_error_code,
            ),
        },
        Operation::CreateInstitution {
            id,
            name: current_name,
            kind,
            email: current_email,
            phone: current_phone,
        } => Operation::CreateInstitution {
            id,
            name: errors.replace(
                "name",
                current_name,
                name,
                PartyName::parse,
                text_error_code,
            ),
            kind,
            email: errors.replace_optional("email", current_email, email, Email::parse, email_code),
            phone: errors.replace_optional(
                "phone",
                current_phone,
                phone,
                PhoneNumber::parse,
                text_error_code,
            ),
        },
        other => other,
    };
    if errors.0.is_empty() {
        Ok(edited)
    } else {
        Err(errors.0)
    }
}
