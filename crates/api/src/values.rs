//! The DTOs of fact values, fact states, value types and passages (ADRs 0049 and 0050) that more than one resource uses.
//!
//! The values and states have the shape of the `ValueInput` and `FactStateInput` of a proposal,
//! so a client can send a value back as it reads it. The tag values are kebab-case (ADR 0044).

use jiff::civil;
use serde::Serialize;
use tada_app::domain::facts::{
    self as domain, FactValue, Label as DomainLabel, ReferenceId, ValueType as DomainValueType,
    Valued,
};
use tada_app::domain::sources::Passage as DomainPassage;
use utoipa::ToSchema;
use uuid::Uuid;

/// The status of a fact: `unknown` has no value (ADR 0049).
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FactState {
    Accepted,
    Assumption,
    Unknown,
}

/// A fact value. `type` names the value type. The list of types is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Value {
    /// A short text.
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
    Date {
        date: civil::Date,
    },
    /// A range of dates with the meaning of its dates.
    DateWindow {
        start: civil::Date,
        end: civil::Date,
        granularity: Granularity,
    },
    /// The keys of the choices.
    Choice {
        keys: Vec<String>,
    },
    Reference {
        target: ReferenceTarget,
        id: Uuid,
    },
}

/// The meaning of the dates of a date window.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Granularity {
    Day,
    Week,
    Month,
}

/// The kind of record that a reference points to. The list of kinds is open.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceTarget {
    Document,
    Event,
}

/// The state of a fact with its value and its mark "approximate", in the shape of the `FactStateInput` of a proposal,
/// so a client can send a state back as it reads it. An unknown fact has neither, never `null` (ADR 0044).
pub(crate) fn state_parts(
    state: &domain::FactState<Valued>,
) -> (FactState, Option<Value>, Option<bool>) {
    match state {
        domain::FactState::Accepted(valued) => (
            FactState::Accepted,
            Some(value(valued)),
            Some(valued.approximate),
        ),
        domain::FactState::Assumption(valued) => (
            FactState::Assumption,
            Some(value(valued)),
            Some(valued.approximate),
        ),
        domain::FactState::Unknown => (FactState::Unknown, None, None),
    }
}

fn value(valued: &Valued) -> Value {
    match &valued.value {
        FactValue::Text(text) => Value::Text {
            text: text.as_str().to_owned(),
        },
        FactValue::Boolean(value) => Value::Boolean { value: *value },
        FactValue::Quantity(range) => Value::Quantity {
            min: range.min().to_string(),
            max: range.max().to_string(),
        },
        FactValue::Money(range) => Value::Money {
            min: range.min().get(),
            max: range.max().get(),
        },
        FactValue::Date(date) => Value::Date { date: *date },
        FactValue::DateWindow(window) => Value::DateWindow {
            start: window.start(),
            end: window.end(),
            granularity: window.granularity().into(),
        },
        FactValue::Choice(keys) => Value::Choice {
            keys: keys.iter().map(|key| key.as_str().to_owned()).collect(),
        },
        FactValue::Reference(reference) => {
            let (target, id) = match reference {
                ReferenceId::Document(id) => (ReferenceTarget::Document, id.as_uuid()),
                ReferenceId::Event(id) => (ReferenceTarget::Event, id.as_uuid()),
            };
            Value::Reference { target, id }
        }
    }
}

impl From<domain::Granularity> for Granularity {
    fn from(granularity: domain::Granularity) -> Self {
        match granularity {
            domain::Granularity::Day => Self::Day,
            domain::Granularity::Week => Self::Week,
            domain::Granularity::Month => Self::Month,
        }
    }
}

impl From<domain::ReferenceTarget> for ReferenceTarget {
    fn from(target: domain::ReferenceTarget) -> Self {
        match target {
            domain::ReferenceTarget::Document => Self::Document,
            domain::ReferenceTarget::Event => Self::Event,
        }
    }
}

/// The value type of a field, with its unit, currency or choices. The list of types is open.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ValueType {
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
        /// Absent if the field accepts each granularity.
        #[serde(skip_serializing_if = "Option::is_none")]
        granularity: Option<Granularity>,
    },
    Choice {
        values: Vec<Choice>,
        /// True if a fact can hold several choices.
        multiple: bool,
    },
    Reference {
        target: ReferenceTarget,
    },
}

/// One choice of a choice field.
#[derive(Debug, Serialize, ToSchema)]
pub struct Choice {
    pub key: String,
    pub label: Label,
}

/// The label that people see: a Fluent message of a shipped field, or the German text of a field of an event.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Label {
    /// The ID of a Fluent message of the web client, for example `field-date_window`.
    Message {
        id: String,
    },
    Text {
        text: String,
    },
}

impl From<&DomainLabel> for Label {
    fn from(label: &DomainLabel) -> Self {
        match label {
            DomainLabel::Builtin(id) => Self::Message {
                id: id.as_str().to_owned(),
            },
            DomainLabel::Text(text) => Self::Text {
                text: text.as_str().to_owned(),
            },
        }
    }
}

impl From<&DomainValueType> for ValueType {
    fn from(value_type: &DomainValueType) -> Self {
        match value_type {
            DomainValueType::Text => Self::Text,
            DomainValueType::Boolean => Self::Boolean,
            DomainValueType::Quantity { unit } => Self::Quantity {
                unit: unit.as_str().to_owned(),
            },
            DomainValueType::Money { currency } => Self::Money {
                currency: currency.as_str().to_owned(),
            },
            DomainValueType::Date => Self::Date,
            DomainValueType::DateWindow { granularity } => Self::DateWindow {
                granularity: granularity.map(Granularity::from),
            },
            DomainValueType::Choice { values, multiple } => Self::Choice {
                values: values
                    .iter()
                    .map(|value| Choice {
                        key: value.key.as_str().to_owned(),
                        label: (&value.label).into(),
                    })
                    .collect(),
                multiple: *multiple,
            },
            DomainValueType::Reference { target } => Self::Reference {
                target: (*target).into(),
            },
        }
    }
}

/// A passage of a source version: a range of characters in its normalized text and the exact quote.
#[derive(Debug, Serialize, ToSchema)]
pub struct Passage {
    /// The offset of the first character, in Unicode scalar values.
    pub start: u32,
    /// The offset after the last character.
    pub end: u32,
    pub quote: String,
    /// The page of a PDF, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

impl From<&DomainPassage> for Passage {
    fn from(passage: &DomainPassage) -> Self {
        Self {
            start: passage.start,
            end: passage.end,
            quote: passage.quote.clone(),
            page: passage.page,
        }
    }
}
