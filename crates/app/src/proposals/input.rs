//! The input of `create_changeset`: the one shape of a new changeset for the API, MCP and Telegram (ADR 0040).
//!
//! The types derive `Deserialize` and `JsonSchema`, so the JSON Schema of an MCP tool comes from them.
//! `TryFrom<OperationInput> for Operation` maps an operation to the domain through its constructors.

use std::borrow::Cow;
use std::fmt;

use jiff::civil;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tada_domain::RecordVersion;
use tada_domain::documents::{DocumentName, DraftMarkdown};
use tada_domain::events::{EventKey, EventName, EventTimeZone};
use tada_domain::facts::{
    ChoiceKey, ChoiceValue, Currency, DateWindow, Decimal, Description, FactState, FactValue,
    FieldKey, Granularity, KeyError, Label, MinorUnits, ModuleKey, Range, ReferenceId,
    ReferenceTarget, ShortText, TextError, Unit, ValueError, ValueType, Valued,
};
use tada_domain::ids::{DocumentId, EventId, FieldDefinitionId, OpenQuestionId, UserId};
use tada_domain::proposals::{DraftDocument, Operation, QuestionText};
use uuid::Uuid;

use crate::events::{key_error_code, name_error_code};
use crate::problem::FieldError;

/// A new changeset: the text of one intake and the proposals that it supports.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewChangeset {
    /// The UUIDv7 of the new changeset. Send a new one, so that a retry with the same id is safe.
    /// Without it, tada makes one, and a retry after a lost response is refused.
    #[serde(default)]
    pub id: Option<Uuid>,
    /// The event of all proposals. A changeset without an event belongs to the organization:
    /// it creates new events, and its other proposals work in these new events.
    #[serde(default)]
    pub event_id: Option<Uuid>,
    /// The member's own words. tada stores them as a source version.
    /// The passages count characters (Unicode scalar values) of this text after normalization: Unicode NFC with `\n` line ends.
    /// At most 100,000 characters after the normalization.
    pub source_text: String,
    pub proposals: Vec<NewProposal>,
}

/// One proposal of a new changeset.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewProposal {
    /// The UUIDv7 of the proposal. Other proposals of the changeset depend on it by this ID.
    pub id: Uuid,
    pub operation: OperationInput,
    /// The proposals of the same changeset that must apply before this one,
    /// for example the new field definition of a fact.
    #[serde(default)]
    pub depends_on: Vec<Uuid>,
    /// The passages of the source text that support the proposal. Each proposal has at least one.
    #[serde(default)]
    pub evidence: Vec<PassageInput>,
    /// A short reason for the proposal.
    pub reason: String,
}

/// A passage of a source version: a range of characters and its exact quote.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PassageInput {
    /// The source version of the passage. Leave it out for the source text of the changeset.
    /// Another source version must be readable in the event of the proposal and have a text,
    /// for example a text file that a member uploaded to the event.
    #[serde(default)]
    pub source_version_id: Option<Uuid>,
    /// The offset of the first character.
    pub start: u32,
    /// The offset after the last character.
    pub end: u32,
    /// The exact text from `start` to `end`.
    pub quote: String,
    /// The page of a PDF, from 1.
    #[serde(default)]
    pub page: Option<u32>,
}

/// The change that a proposal suggests.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OperationInput {
    /// Create an event. Only a changeset without an event can create one.
    CreateEvent {
        /// The UUIDv7 of the new event.
        id: Uuid,
        /// Two to eight capital letters and digits, for example `FLY28`.
        key: String,
        name: String,
        /// An IANA time zone. The default is `Europe/Zurich`.
        #[serde(default)]
        time_zone: Option<String>,
    },
    /// Set the fact of a field in an event.
    SetFact {
        event_id: Uuid,
        /// A field of the event catalog, or a new field of the same changeset.
        field_id: Uuid,
        state: FactStateInput,
        /// The current version of the fact. Leave it out if the event has no fact of the field yet.
        #[serde(default)]
        expected_version: Option<i64>,
    },
    /// Add a field definition to the field catalog of an event.
    AddFieldDefinition {
        /// The UUIDv7 of the new field.
        id: Uuid,
        event_id: Uuid,
        /// A `snake_case` key of 2 to 64 characters.
        key: String,
        /// The German label that people see.
        label: String,
        value_type: ValueTypeInput,
        /// The meaning of the field, in English.
        description: String,
        /// A `snake_case` module key, for example `aviation`.
        module: String,
    },
    /// Add a choice to a choice field of the event.
    AddChoiceValue {
        event_id: Uuid,
        /// A field of the event, or a new field of the same changeset.
        field_id: Uuid,
        /// The `snake_case` key of the choice.
        key: String,
        /// The German label of the choice.
        label: String,
    },
    /// Close a field of the event for new facts.
    DeprecateField { event_id: Uuid, field_id: Uuid },
    /// Create an open question with its owner.
    CreateOpenQuestion {
        /// The UUIDv7 of the new open question.
        id: Uuid,
        event_id: Uuid,
        text: String,
        /// The user ID of the member who owns the question.
        owner: Uuid,
    },
    /// Add a draft version to a document of the event (ADR 0051).
    CreateDocumentDraft {
        event_id: Uuid,
        document: DraftDocumentInput,
        /// CommonMark with tables, UTF-8, one sentence per line. No raw HTML and no images.
        /// A fact is an empty link to an exact fact version: `[](tada:fact/<fact-uuid>?v=<n>)`.
        /// Never write the value of a fact as text.
        /// A source passage is a link with the supporting words: `[words](tada:source/<source-version-uuid>#<start>-<end>)`.
        /// A `tada:` link writes each UUID in lowercase with hyphens. Other links use `https` or `mailto` only.
        /// Each link must cite an accepted fact version, an assumption or an unknown of the event, never an open proposal,
        /// or a source version that the member can see.
        /// A number, date or amount outside a `tada:` link gives a lint warning for the reviewer.
        markdown: String,
    },
}

/// The document of a draft.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftDocumentInput {
    /// A new document of the event.
    New {
        /// The UUIDv7 of the new document.
        id: Uuid,
        /// The name of the document, 1 to 200 characters.
        name: String,
    },
    /// An existing document of the event.
    Existing {
        document_id: Uuid,
        /// The current version of the document.
        expected_version: i64,
    },
}

/// The state of a fact. An unknown fact has no value.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FactStateInput {
    Accepted {
        value: ValueInput,
        /// True for an approximate value, for example "about 20,000".
        #[serde(default)]
        approximate: bool,
    },
    Assumption {
        value: ValueInput,
        #[serde(default)]
        approximate: bool,
    },
    Unknown,
}

/// A fact value. Its type must match the value type of the field.
/// The read tools give values in the same shape (ADR 0040).
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ValueInput {
    Text {
        text: String,
    },
    Boolean {
        value: bool,
    },
    /// A decimal number or range in the unit of the field, as text, for example `"20000"` or `"1.5"`.
    /// A single value has `min` equal to `max`.
    Quantity {
        min: String,
        max: String,
    },
    /// An amount or range in the minor unit of the currency of the field, for example 1500 for CHF 15.00.
    Money {
        min: i64,
        max: i64,
    },
    /// A date as `YYYY-MM-DD`.
    Date {
        date: String,
    },
    /// A range of dates as `YYYY-MM-DD`, with the meaning of its dates.
    DateWindow {
        start: String,
        end: String,
        granularity: GranularityInput,
    },
    /// The keys of the choices.
    Choice {
        keys: Vec<String>,
    },
    Reference {
        target: ReferenceTargetInput,
        id: Uuid,
    },
}

/// The value type of a new field. It never changes.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ValueTypeInput {
    Text,
    Boolean,
    Quantity {
        /// The `snake_case` key of the unit, for example `person_per_day`.
        unit: String,
    },
    Money {
        /// An ISO 4217 code, for example `CHF`.
        currency: String,
    },
    Date,
    DateWindow {
        /// Leave it out to accept each granularity.
        #[serde(default)]
        granularity: Option<GranularityInput>,
    },
    Choice {
        values: Vec<ChoiceInput>,
        /// True if a fact can hold several choices.
        #[serde(default)]
        multiple: bool,
    },
    Reference {
        target: ReferenceTargetInput,
    },
}

/// One choice of a new choice field.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChoiceInput {
    pub key: String,
    /// The German label of the choice.
    pub label: String,
}

#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GranularityInput {
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceTargetInput {
    Document,
    Event,
}

/// The input contains the words of members, so `Debug` shows the name of the type only (ADR 0035).
macro_rules! redacted_debug {
    ($($name:ident),*) => {
        $(impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(..)"))
            }
        })*
    };
}

redacted_debug!(
    NewChangeset,
    NewProposal,
    PassageInput,
    OperationInput,
    DraftDocumentInput,
    FactStateInput,
    ValueInput,
    ValueTypeInput,
    ChoiceInput,
    GranularityInput,
    ReferenceTargetInput
);

/// Collects the invalid fields of one input, with paths relative to it.
#[derive(Default)]
struct Errors(Vec<FieldError>);

impl Errors {
    fn push(&mut self, field: impl Into<Cow<'static, str>>, code: &'static str) {
        self.0.push(FieldError::new(field, code));
    }

    /// The value of `result`, or `None` after it adds the error at `field`.
    fn take<T, E>(
        &mut self,
        field: &'static str,
        result: Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> Option<T> {
        result.map_err(|error| self.push(field, code(error))).ok()
    }

    /// Adds the errors of a nested input under `prefix`.
    fn nest(&mut self, prefix: &str, errors: Vec<FieldError>) {
        for error in errors {
            self.push(format!("{prefix}/{}", error.field), error.code);
        }
    }

    fn finish<T>(self, value: Option<T>) -> Result<T, Vec<FieldError>> {
        match value {
            Some(value) if self.0.is_empty() => Ok(value),
            _ => Err(self.0),
        }
    }
}

pub(crate) fn text_error_code(error: TextError) -> &'static str {
    match error {
        TextError::Empty => "empty",
        TextError::TooLong => "too-long",
        TextError::ControlCharacter => "control-character",
    }
}

fn snake_case_code(error: KeyError) -> &'static str {
    match error {
        KeyError::Length => "length",
        KeyError::Characters => "characters",
    }
}

/// The entry code of a value that breaks a rule of its value type.
pub(crate) fn value_error_code(error: ValueError) -> &'static str {
    match error {
        ValueError::TypeMismatch => "type-mismatch",
        ValueError::NoChoice => "no-choice",
        ValueError::DuplicateChoice => "duplicate-choice",
        ValueError::SeveralChoices => "several-choices",
        ValueError::UnknownChoice => "unknown-choice",
        ValueError::GranularityMismatch => "granularity-mismatch",
        ValueError::ReferenceTargetMismatch => "reference-target-mismatch",
        ValueError::ScaleTooLarge => "scale-too-large",
        ValueError::RangeOrder => "range-order",
        ValueError::DateWindowOrder => "date-window-order",
        ValueError::Currency => "currency",
        ValueError::NoTextForm => "no-text-form",
        ValueError::Key(error) => snake_case_code(error),
        ValueError::Text(error) => text_error_code(error),
    }
}

/// Maps an operation to the domain. The errors name the fields of the operation, for example `state/value/min`.
/// The checks against the field catalog and the other proposals of the changeset come later.
impl TryFrom<OperationInput> for Operation {
    type Error = Vec<FieldError>;

    fn try_from(input: OperationInput) -> Result<Self, Vec<FieldError>> {
        let mut errors = Errors::default();
        let operation = match input {
            OperationInput::CreateEvent {
                id,
                key,
                name,
                time_zone,
            } => {
                let key = errors.take("key", EventKey::parse(&key), key_error_code);
                let name = errors.take("name", EventName::parse(&name), name_error_code);
                let time_zone = match time_zone {
                    None => Some(EventTimeZone::default_zone()),
                    Some(zone) => {
                        errors.take("time_zone", EventTimeZone::parse(&zone), |_| "unknown")
                    }
                };
                (|| {
                    Some(Operation::CreateEvent {
                        id: EventId::from_uuid(id),
                        key: key?,
                        name: name?,
                        time_zone: time_zone?,
                    })
                })()
            }
            OperationInput::SetFact {
                event_id,
                field_id,
                state,
                expected_version,
            } => {
                let state = state_from_input(state)
                    .map_err(|nested| errors.nest("state", nested))
                    .ok();
                let expected_version = match expected_version.map(RecordVersion::new) {
                    None => Some(None),
                    Some(Some(version)) => Some(Some(version)),
                    Some(None) => {
                        errors.push("expected_version", "invalid");
                        None
                    }
                };
                (|| {
                    Some(Operation::SetFact {
                        event_id: EventId::from_uuid(event_id),
                        field_id: FieldDefinitionId::from_uuid(field_id),
                        state: state?,
                        expected_version: expected_version?,
                    })
                })()
            }
            OperationInput::AddFieldDefinition {
                id,
                event_id,
                key,
                label,
                value_type,
                description,
                module,
            } => {
                let key = errors.take("key", FieldKey::parse(&key), snake_case_code);
                let label = errors.take("label", ShortText::parse(&label), value_error_code);
                let value_type = value_type_from_input(value_type)
                    .map_err(|nested| errors.nest("value_type", nested))
                    .ok();
                let description = errors.take(
                    "description",
                    Description::parse(&description),
                    text_error_code,
                );
                let module = errors.take("module", ModuleKey::parse(&module), snake_case_code);
                (|| {
                    Some(Operation::AddFieldDefinition {
                        id: FieldDefinitionId::from_uuid(id),
                        event_id: EventId::from_uuid(event_id),
                        key: key?,
                        label: label?,
                        value_type: value_type?,
                        description: description?,
                        module: module?,
                    })
                })()
            }
            OperationInput::AddChoiceValue {
                event_id,
                field_id,
                key,
                label,
            } => {
                let key = errors.take("key", ChoiceKey::parse(&key), snake_case_code);
                let label = errors.take("label", ShortText::parse(&label), value_error_code);
                (|| {
                    Some(Operation::AddChoiceValue {
                        event_id: EventId::from_uuid(event_id),
                        field_id: FieldDefinitionId::from_uuid(field_id),
                        key: key?,
                        label: label?,
                    })
                })()
            }
            OperationInput::DeprecateField { event_id, field_id } => {
                Some(Operation::DeprecateField {
                    event_id: EventId::from_uuid(event_id),
                    field_id: FieldDefinitionId::from_uuid(field_id),
                })
            }
            OperationInput::CreateOpenQuestion {
                id,
                event_id,
                text,
                owner,
            } => errors
                .take("text", QuestionText::parse(&text), text_error_code)
                .map(|text| Operation::CreateOpenQuestion {
                    id: OpenQuestionId::from_uuid(id),
                    event_id: EventId::from_uuid(event_id),
                    text,
                    owner: UserId::from_uuid(owner),
                }),
            OperationInput::CreateDocumentDraft {
                event_id,
                document,
                markdown,
            } => {
                let document = match document {
                    DraftDocumentInput::New { id, name } => errors
                        .take(
                            "document/new/name",
                            DocumentName::parse(&name),
                            text_error_code,
                        )
                        .map(|name| DraftDocument::New {
                            id: DocumentId::from_uuid(id),
                            name,
                        }),
                    DraftDocumentInput::Existing {
                        document_id,
                        expected_version,
                    } => match RecordVersion::new(expected_version) {
                        Some(expected_version) => Some(DraftDocument::Existing {
                            document_id: DocumentId::from_uuid(document_id),
                            expected_version,
                        }),
                        None => {
                            errors.push("document/existing/expected_version", "invalid");
                            None
                        }
                    },
                };
                let markdown =
                    errors.take("markdown", DraftMarkdown::parse(&markdown), text_error_code);
                (|| {
                    Some(Operation::CreateDocumentDraft {
                        event_id: EventId::from_uuid(event_id),
                        document: document?,
                        markdown: markdown?,
                    })
                })()
            }
        };
        errors.finish(operation)
    }
}

pub(crate) fn state_from_input(
    input: FactStateInput,
) -> Result<FactState<Valued>, Vec<FieldError>> {
    let valued = |value: ValueInput, approximate: bool| {
        let mut errors = Errors::default();
        let value = value_from_input(value)
            .map_err(|nested| errors.nest("value", nested))
            .ok();
        errors.finish(value.map(|value| Valued { value, approximate }))
    };
    match input {
        FactStateInput::Accepted { value, approximate } => {
            valued(value, approximate).map(FactState::Accepted)
        }
        FactStateInput::Assumption { value, approximate } => {
            valued(value, approximate).map(FactState::Assumption)
        }
        FactStateInput::Unknown => Ok(FactState::Unknown),
    }
}

fn value_from_input(input: ValueInput) -> Result<FactValue, Vec<FieldError>> {
    let mut errors = Errors::default();
    let value = match input {
        ValueInput::Text { text } => errors
            .take("text", ShortText::parse(&text), value_error_code)
            .map(FactValue::Text),
        ValueInput::Boolean { value } => Some(FactValue::Boolean(value)),
        ValueInput::Quantity { min, max } => {
            let min = errors.take("min", Decimal::parse(&min), value_error_code);
            let max = errors.take("max", Decimal::parse(&max), value_error_code);
            match (min, max) {
                (Some(min), Some(max)) => errors
                    .take("max", Range::new(min, max), value_error_code)
                    .map(FactValue::Quantity),
                _ => None,
            }
        }
        ValueInput::Money { min, max } => errors
            .take(
                "max",
                Range::new(MinorUnits::new(min), MinorUnits::new(max)),
                value_error_code,
            )
            .map(FactValue::Money),
        ValueInput::Date { date } => errors
            .take("date", date.parse::<civil::Date>(), |_| "date")
            .map(FactValue::Date),
        ValueInput::DateWindow {
            start,
            end,
            granularity,
        } => {
            let start = errors.take("start", start.parse::<civil::Date>(), |_| "date");
            let end = errors.take("end", end.parse::<civil::Date>(), |_| "date");
            match (start, end) {
                (Some(start), Some(end)) => errors
                    .take(
                        "end",
                        DateWindow::new(start, end, granularity.into()),
                        value_error_code,
                    )
                    .map(FactValue::DateWindow),
                _ => None,
            }
        }
        ValueInput::Choice { keys } => {
            let keys: Vec<Option<ChoiceKey>> = keys
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    ChoiceKey::parse(key)
                        .map_err(|error| {
                            errors.push(format!("keys/{index}"), snake_case_code(error))
                        })
                        .ok()
                })
                .collect();
            keys.into_iter()
                .collect::<Option<Vec<_>>>()
                .map(FactValue::Choice)
        }
        ValueInput::Reference { target, id } => Some(FactValue::Reference(match target {
            ReferenceTargetInput::Document => ReferenceId::Document(DocumentId::from_uuid(id)),
            ReferenceTargetInput::Event => ReferenceId::Event(EventId::from_uuid(id)),
        })),
    };
    errors.finish(value)
}

fn value_type_from_input(input: ValueTypeInput) -> Result<ValueType, Vec<FieldError>> {
    let mut errors = Errors::default();
    let value_type = match input {
        ValueTypeInput::Text => Some(ValueType::Text),
        ValueTypeInput::Boolean => Some(ValueType::Boolean),
        ValueTypeInput::Quantity { unit } => errors
            .take("unit", Unit::parse(&unit), snake_case_code)
            .map(|unit| ValueType::Quantity { unit }),
        ValueTypeInput::Money { currency } => errors
            .take("currency", Currency::parse(&currency), value_error_code)
            .map(|currency| ValueType::Money { currency }),
        ValueTypeInput::Date => Some(ValueType::Date),
        ValueTypeInput::DateWindow { granularity } => Some(ValueType::DateWindow {
            granularity: granularity.map(Granularity::from),
        }),
        ValueTypeInput::Choice { values, multiple } => {
            if values.is_empty() {
                errors.push("values", "empty");
            }
            let mut keys: Vec<&str> = Vec::new();
            let values: Vec<Option<ChoiceValue>> = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    if keys.contains(&value.key.as_str()) {
                        errors.push(format!("values/{index}/key"), "duplicate");
                    }
                    keys.push(&value.key);
                    let key = ChoiceKey::parse(&value.key).map_err(|error| {
                        errors.push(format!("values/{index}/key"), snake_case_code(error));
                    });
                    let label = ShortText::parse(&value.label).map_err(|error| {
                        errors.push(format!("values/{index}/label"), value_error_code(error));
                    });
                    Some(ChoiceValue {
                        key: key.ok()?,
                        label: Label::Text(label.ok()?),
                    })
                })
                .collect();
            values
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(|values| ValueType::Choice { values, multiple })
        }
        ValueTypeInput::Reference { target } => Some(ValueType::Reference {
            target: match target {
                ReferenceTargetInput::Document => ReferenceTarget::Document,
                ReferenceTargetInput::Event => ReferenceTarget::Event,
            },
        }),
    };
    errors.finish(value_type)
}

impl From<GranularityInput> for Granularity {
    fn from(input: GranularityInput) -> Self {
        match input {
            GranularityInput::Day => Self::Day,
            GranularityInput::Week => Self::Week,
            GranularityInput::Month => Self::Month,
        }
    }
}

/// The value in the shape of the input, so that a value that a read tool gives can go back into a proposal.
impl From<&FactValue> for ValueInput {
    fn from(value: &FactValue) -> Self {
        match value {
            FactValue::Text(text) => Self::Text {
                text: text.as_str().to_owned(),
            },
            FactValue::Boolean(value) => Self::Boolean { value: *value },
            FactValue::Quantity(range) => Self::Quantity {
                min: decimal_text(range.min()),
                max: decimal_text(range.max()),
            },
            FactValue::Money(range) => Self::Money {
                min: range.min().get(),
                max: range.max().get(),
            },
            FactValue::Date(date) => Self::Date {
                date: date.to_string(),
            },
            FactValue::DateWindow(window) => Self::DateWindow {
                start: window.start().to_string(),
                end: window.end().to_string(),
                granularity: match window.granularity() {
                    Granularity::Day => GranularityInput::Day,
                    Granularity::Week => GranularityInput::Week,
                    Granularity::Month => GranularityInput::Month,
                },
            },
            FactValue::Choice(keys) => Self::Choice {
                keys: keys.iter().map(|key| key.as_str().to_owned()).collect(),
            },
            FactValue::Reference(ReferenceId::Document(id)) => Self::Reference {
                target: ReferenceTargetInput::Document,
                id: id.as_uuid(),
            },
            FactValue::Reference(ReferenceId::Event(id)) => Self::Reference {
                target: ReferenceTargetInput::Event,
                id: id.as_uuid(),
            },
        }
    }
}

/// The text of a decimal that `Decimal::parse` reads: for example `20000`, `1.5` or `-0.25`.
fn decimal_text(decimal: Decimal) -> String {
    let digits = decimal.units().unsigned_abs().to_string();
    let scale = usize::from(decimal.scale());
    let sign = if decimal.units() < 0 { "-" } else { "" };
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let digits = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = digits.split_at(digits.len() - scale);
    format!("{sign}{whole}.{fraction}")
}

/// Parses a decimal number without a float: an optional `-`, digits, and an optional `.` with digits.
#[cfg(test)]
mod tests {
    use super::*;

    fn decimal(units: i64, scale: u8) -> Decimal {
        Decimal::new(units, scale).unwrap()
    }

    #[test]
    fn writes_decimals_that_parse_back() {
        for (units, scale, text) in [
            (20_000, 0, "20000"),
            (15, 1, "1.5"),
            (-25, 2, "-0.25"),
            (7, 6, "0.000007"),
            (-3, 0, "-3"),
        ] {
            assert_eq!(decimal_text(decimal(units, scale)), text);
            assert_eq!(Decimal::parse(text), Ok(decimal(units, scale)));
        }
    }

    #[test]
    fn a_value_goes_back_into_the_same_value() {
        for json in [
            serde_json::json!({"type": "text", "text": "Flugfeld Testwil"}),
            serde_json::json!({"type": "boolean", "value": true}),
            serde_json::json!({"type": "quantity", "min": "15000", "max": "25000.5"}),
            serde_json::json!({"type": "money", "min": 1500, "max": 1500}),
            serde_json::json!({"type": "date", "date": "2030-05-18"}),
            serde_json::json!({"type": "date-window", "start": "2030-05-01", "end": "2030-06-30", "granularity": "month"}),
            serde_json::json!({"type": "choice", "keys": ["airshow", "catering"]}),
            serde_json::json!({"type": "reference", "target": "event", "id": "01a116d3-b70e-7215-91cb-7bc0ffc656e5"}),
        ] {
            let input: ValueInput = serde_json::from_value(json.clone()).unwrap();
            let value = value_from_input(input).unwrap();
            let output = serde_json::to_value(ValueInput::from(&value)).unwrap();
            assert_eq!(output, json);
        }
    }

    #[test]
    fn parses_decimals_without_a_float() {
        assert_eq!(Decimal::parse("20000"), Ok(Decimal::integer(20_000)));
        assert_eq!(Decimal::parse("1.5"), Ok(decimal(15, 1)));
        assert_eq!(Decimal::parse("-0.25"), Ok(decimal(-25, 2)));
        assert_eq!(Decimal::parse("1.0000001"), Err(ValueError::ScaleTooLarge));
        for text in [
            "",
            "-",
            "1.",
            ".5",
            "1e3",
            "1,5",
            "+1",
            "99999999999999999999",
        ] {
            assert_eq!(
                Decimal::parse(text),
                Err(ValueError::TypeMismatch),
                "{text:?}"
            );
        }
    }

    fn json_operation(json: serde_json::Value) -> Result<Operation, Vec<FieldError>> {
        Operation::try_from(serde_json::from_value::<OperationInput>(json).unwrap())
    }

    fn fields(errors: Vec<FieldError>) -> Vec<(String, &'static str)> {
        errors
            .into_iter()
            .map(|error| (error.field.into_owned(), error.code))
            .collect()
    }

    #[test]
    fn maps_a_fact_with_a_date_window() {
        let operation = json_operation(serde_json::json!({
            "kind": "set-fact",
            "event_id": Uuid::from_u128(1),
            "field_id": Uuid::from_u128(2),
            "state": {
                "state": "assumption",
                "value": {"type": "date-window", "start": "2030-05-01", "end": "2030-06-30", "granularity": "month"},
            },
        }))
        .unwrap();
        let window = DateWindow::new(
            civil::date(2030, 5, 1),
            civil::date(2030, 6, 30),
            Granularity::Month,
        )
        .unwrap();
        assert_eq!(
            operation,
            Operation::SetFact {
                event_id: EventId::from_uuid(Uuid::from_u128(1)),
                field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(2)),
                state: FactState::Assumption(Valued {
                    value: FactValue::DateWindow(window),
                    approximate: false,
                }),
                expected_version: None,
            }
        );
    }

    #[test]
    fn names_each_invalid_field_of_an_operation() {
        let errors = json_operation(serde_json::json!({
            "kind": "set-fact",
            "event_id": Uuid::from_u128(1),
            "field_id": Uuid::from_u128(2),
            "state": {"state": "accepted", "value": {"type": "quantity", "min": "30", "max": "2.0000001"}},
            "expected_version": 0,
        }))
        .unwrap_err();
        assert_eq!(
            fields(errors),
            [
                ("state/value/max".to_owned(), "scale-too-large"),
                ("expected_version".to_owned(), "invalid"),
            ]
        );

        let errors = json_operation(serde_json::json!({
            "kind": "add-field-definition",
            "id": Uuid::from_u128(3),
            "event_id": Uuid::from_u128(1),
            "key": "Runway",
            "label": "Piste",
            "value_type": {"type": "choice", "values": [
                {"key": "grass", "label": "Gras"},
                {"key": "grass", "label": ""},
            ]},
            "description": "The surface of the runway.",
            "module": "aviation",
        }))
        .unwrap_err();
        assert_eq!(
            fields(errors),
            [
                ("key".to_owned(), "characters"),
                ("value_type/values/1/key".to_owned(), "duplicate"),
                ("value_type/values/1/label".to_owned(), "empty"),
            ]
        );
    }

    #[test]
    fn a_new_event_gets_the_default_time_zone() {
        let operation = json_operation(serde_json::json!({
            "kind": "create-event",
            "id": Uuid::from_u128(1),
            "key": "FLY30",
            "name": "Fly-in Musterhausen",
        }))
        .unwrap();
        let Operation::CreateEvent { time_zone, .. } = operation else {
            panic!("not a new event");
        };
        assert_eq!(time_zone, EventTimeZone::default_zone());
    }

    #[test]
    fn rejects_unknown_input_fields() {
        let input = serde_json::json!({"kind": "deprecate-field", "event_id": Uuid::from_u128(2), "field_id": Uuid::from_u128(1), "force": true});
        assert!(serde_json::from_value::<OperationInput>(input).is_err());
    }

    #[test]
    fn debug_hides_the_input() {
        let input: PassageInput =
            serde_json::from_value(serde_json::json!({"start": 0, "end": 4, "quote": "Anna"}))
                .unwrap();
        assert_eq!(format!("{input:?}"), "PassageInput(..)");
    }
}
