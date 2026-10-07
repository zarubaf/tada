//! The stored form of value types and fact values (ADR 0049) in `jsonb` columns.
//!
//! The domain crate has no `serde`, so this code module maps the domain types to records and back.
//! The mapping back uses the domain constructors, so a stored value that breaks a rule is an `InvalidRow`.

use jiff::civil;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use tada_app::domain::facts::{
    ChoiceKey, ChoiceValue, Currency, DateWindow, Decimal, FactState, FactValue, Granularity,
    Label, MessageId, MinorUnits, Range, ReferenceId, ReferenceTarget, ShortText, Unit, ValueType,
    Valued,
};
use tada_app::domain::ids::{DocumentId, EventId};

use crate::error::InvalidRow;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ValueTypeRecord {
    Text,
    Boolean,
    Quantity {
        unit: String,
    },
    Money {
        currency: String,
    },
    Date,
    DateWindow {
        granularity: Option<GranularityRecord>,
    },
    Choice {
        values: Vec<ChoiceRecord>,
        multiple: bool,
    },
    Reference {
        target: TargetRecord,
    },
}

#[derive(Serialize, Deserialize)]
struct ChoiceRecord {
    key: String,
    label: LabelRecord,
}

/// A label: a Fluent message of a shipped field, or the German text of a field of an event.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LabelRecord {
    Message(String),
    Text(String),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GranularityRecord {
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TargetRecord {
    Document,
    Event,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ValueRecord {
    Text {
        text: String,
    },
    Boolean {
        value: bool,
    },
    Quantity {
        min: DecimalRecord,
        max: DecimalRecord,
    },
    Money {
        min: i64,
        max: i64,
    },
    Date {
        date: civil::Date,
    },
    DateWindow {
        start: civil::Date,
        end: civil::Date,
        granularity: GranularityRecord,
    },
    Choice {
        keys: Vec<String>,
    },
    Reference {
        target: TargetRecord,
        id: Uuid,
    },
}

/// A decimal as `units` × 10^-`scale`, without a float.
#[derive(Clone, Copy, Serialize, Deserialize)]
struct DecimalRecord {
    units: i64,
    scale: u8,
}

const VALUE_TYPE: &str = "field_definition.value_type";
const VALUE: &str = "fact_version.value";

pub(crate) fn value_type_to_json(value_type: &ValueType) -> serde_json::Value {
    let record = match value_type {
        ValueType::Text => ValueTypeRecord::Text,
        ValueType::Boolean => ValueTypeRecord::Boolean,
        ValueType::Quantity { unit } => ValueTypeRecord::Quantity {
            unit: unit.as_str().to_owned(),
        },
        ValueType::Money { currency } => ValueTypeRecord::Money {
            currency: currency.as_str().to_owned(),
        },
        ValueType::Date => ValueTypeRecord::Date,
        ValueType::DateWindow { granularity } => ValueTypeRecord::DateWindow {
            granularity: granularity.map(granularity_record),
        },
        ValueType::Choice { values, multiple } => ValueTypeRecord::Choice {
            values: values
                .iter()
                .map(|value| ChoiceRecord {
                    key: value.key.as_str().to_owned(),
                    label: label_record(&value.label),
                })
                .collect(),
            multiple: *multiple,
        },
        ValueType::Reference { target } => ValueTypeRecord::Reference {
            target: target_record(*target),
        },
    };
    serde_json::to_value(record).expect("a value type record is valid JSON")
}

pub(crate) fn value_type_from_json(json: &serde_json::Value) -> Result<ValueType, InvalidRow> {
    let invalid = |_| InvalidRow(VALUE_TYPE);
    let record = ValueTypeRecord::deserialize(json).map_err(|_| InvalidRow(VALUE_TYPE))?;
    Ok(match record {
        ValueTypeRecord::Text => ValueType::Text,
        ValueTypeRecord::Boolean => ValueType::Boolean,
        ValueTypeRecord::Quantity { unit } => ValueType::Quantity {
            unit: Unit::parse(&unit).map_err(invalid)?,
        },
        ValueTypeRecord::Money { currency } => ValueType::Money {
            currency: Currency::parse(&currency).map_err(|_| InvalidRow(VALUE_TYPE))?,
        },
        ValueTypeRecord::Date => ValueType::Date,
        ValueTypeRecord::DateWindow { granularity } => ValueType::DateWindow {
            granularity: granularity.map(granularity_from_record),
        },
        ValueTypeRecord::Choice { values, multiple } => ValueType::Choice {
            values: values
                .into_iter()
                .map(|value| {
                    Ok(ChoiceValue {
                        key: ChoiceKey::parse(&value.key).map_err(invalid)?,
                        label: label_from_record(value.label, VALUE_TYPE)?,
                    })
                })
                .collect::<Result<_, InvalidRow>>()?,
            multiple,
        },
        ValueTypeRecord::Reference { target } => ValueType::Reference {
            target: target_from_record(target),
        },
    })
}

/// The columns `state`, `value` and `approximate` of a fact version.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the apply of proposals writes fact versions")
)]
pub(crate) fn fact_state_to_columns(
    state: &FactState<Valued>,
) -> (&'static str, Option<serde_json::Value>, bool) {
    match state {
        FactState::Accepted(valued) => (
            "accepted",
            Some(value_to_json(&valued.value)),
            valued.approximate,
        ),
        FactState::Assumption(valued) => (
            "assumption",
            Some(value_to_json(&valued.value)),
            valued.approximate,
        ),
        FactState::Unknown => ("unknown", None, false),
    }
}

pub(crate) fn fact_state_from_columns(
    state: &str,
    value: Option<serde_json::Value>,
    approximate: bool,
) -> Result<FactState<Valued>, InvalidRow> {
    let valued = |value: Option<serde_json::Value>| -> Result<Valued, InvalidRow> {
        let value = value.ok_or(InvalidRow(VALUE))?;
        Ok(Valued {
            value: value_from_json(&value)?,
            approximate,
        })
    };
    match (state, &value) {
        ("accepted", _) => Ok(FactState::Accepted(valued(value)?)),
        ("assumption", _) => Ok(FactState::Assumption(valued(value)?)),
        ("unknown", None) => Ok(FactState::Unknown),
        _ => Err(InvalidRow("fact_version.state")),
    }
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the apply of proposals writes fact versions")
)]
fn value_to_json(value: &FactValue) -> serde_json::Value {
    let decimal = |value: Decimal| DecimalRecord {
        units: value.units(),
        scale: value.scale(),
    };
    let record = match value {
        FactValue::Text(text) => ValueRecord::Text {
            text: text.as_str().to_owned(),
        },
        FactValue::Boolean(value) => ValueRecord::Boolean { value: *value },
        FactValue::Quantity(range) => ValueRecord::Quantity {
            min: decimal(range.min()),
            max: decimal(range.max()),
        },
        FactValue::Money(range) => ValueRecord::Money {
            min: range.min().get(),
            max: range.max().get(),
        },
        FactValue::Date(date) => ValueRecord::Date { date: *date },
        FactValue::DateWindow(window) => ValueRecord::DateWindow {
            start: window.start(),
            end: window.end(),
            granularity: granularity_record(window.granularity()),
        },
        FactValue::Choice(keys) => ValueRecord::Choice {
            keys: keys.iter().map(|key| key.as_str().to_owned()).collect(),
        },
        FactValue::Reference(reference) => {
            let (target, id) = match reference {
                ReferenceId::Document(id) => (TargetRecord::Document, id.as_uuid()),
                ReferenceId::Event(id) => (TargetRecord::Event, id.as_uuid()),
            };
            ValueRecord::Reference { target, id }
        }
    };
    serde_json::to_value(record).expect("a value record is valid JSON")
}

fn value_from_json(json: &serde_json::Value) -> Result<FactValue, InvalidRow> {
    let invalid = |_| InvalidRow(VALUE);
    let decimal = |record: DecimalRecord| Decimal::new(record.units, record.scale).map_err(invalid);
    let record = ValueRecord::deserialize(json).map_err(|_| InvalidRow(VALUE))?;
    Ok(match record {
        ValueRecord::Text { text } => FactValue::Text(ShortText::parse(&text).map_err(invalid)?),
        ValueRecord::Boolean { value } => FactValue::Boolean(value),
        ValueRecord::Quantity { min, max } => {
            FactValue::Quantity(Range::new(decimal(min)?, decimal(max)?).map_err(invalid)?)
        }
        ValueRecord::Money { min, max } => FactValue::Money(
            Range::new(MinorUnits::new(min), MinorUnits::new(max)).map_err(invalid)?,
        ),
        ValueRecord::Date { date } => FactValue::Date(date),
        ValueRecord::DateWindow {
            start,
            end,
            granularity,
        } => FactValue::DateWindow(
            DateWindow::new(start, end, granularity_from_record(granularity)).map_err(invalid)?,
        ),
        ValueRecord::Choice { keys } => FactValue::Choice(
            keys.iter()
                .map(|key| ChoiceKey::parse(key).map_err(|_| InvalidRow(VALUE)))
                .collect::<Result<_, _>>()?,
        ),
        ValueRecord::Reference { target, id } => FactValue::Reference(match target {
            TargetRecord::Document => ReferenceId::Document(DocumentId::from_uuid(id)),
            TargetRecord::Event => ReferenceId::Event(EventId::from_uuid(id)),
        }),
    })
}

/// The columns `label_message` and `label_text` of a field definition.
pub(crate) fn label_to_columns(label: &Label) -> (Option<&str>, Option<&str>) {
    match label {
        Label::Builtin(message) => (Some(message.as_str()), None),
        Label::Text(text) => (None, Some(text.as_str())),
    }
}

pub(crate) fn label_from_columns(
    message: Option<String>,
    text: Option<String>,
) -> Result<Label, InvalidRow> {
    let record = match (message, text) {
        (Some(message), None) => LabelRecord::Message(message),
        (None, Some(text)) => LabelRecord::Text(text),
        _ => return Err(InvalidRow("field_definition.label")),
    };
    label_from_record(record, "field_definition.label")
}

fn label_record(label: &Label) -> LabelRecord {
    match label {
        Label::Builtin(message) => LabelRecord::Message(message.as_str().to_owned()),
        Label::Text(text) => LabelRecord::Text(text.as_str().to_owned()),
    }
}

fn label_from_record(record: LabelRecord, column: &'static str) -> Result<Label, InvalidRow> {
    match record {
        LabelRecord::Message(message) => MessageId::parse(&message)
            .map(Label::Builtin)
            .ok_or(InvalidRow(column)),
        LabelRecord::Text(text) => ShortText::parse(&text)
            .map(Label::Text)
            .map_err(|_| InvalidRow(column)),
    }
}

fn granularity_record(granularity: Granularity) -> GranularityRecord {
    match granularity {
        Granularity::Day => GranularityRecord::Day,
        Granularity::Week => GranularityRecord::Week,
        Granularity::Month => GranularityRecord::Month,
    }
}

fn granularity_from_record(record: GranularityRecord) -> Granularity {
    match record {
        GranularityRecord::Day => Granularity::Day,
        GranularityRecord::Week => Granularity::Week,
        GranularityRecord::Month => Granularity::Month,
    }
}

fn target_record(target: ReferenceTarget) -> TargetRecord {
    match target {
        ReferenceTarget::Document => TargetRecord::Document,
        ReferenceTarget::Event => TargetRecord::Event,
    }
}

fn target_from_record(record: TargetRecord) -> ReferenceTarget {
    match record {
        TargetRecord::Document => ReferenceTarget::Document,
        TargetRecord::Event => ReferenceTarget::Event,
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use tada_app::domain::facts::core_catalog;

    use super::*;

    fn round_trip(state: FactState<Valued>) {
        let (name, value, approximate) = fact_state_to_columns(&state);
        assert_eq!(
            fact_state_from_columns(name, value, approximate).unwrap(),
            state
        );
    }

    fn valued(value: FactValue) -> Valued {
        Valued {
            value,
            approximate: true,
        }
    }

    #[test]
    fn restores_each_value_type_of_the_core_catalog_and_a_reference() {
        let mut value_types: Vec<_> = core_catalog()
            .into_iter()
            .map(|field| field.value_type)
            .collect();
        value_types.extend([
            ValueType::Boolean,
            ValueType::Date,
            ValueType::Reference {
                target: ReferenceTarget::Document,
            },
        ]);
        for value_type in value_types {
            let json = value_type_to_json(&value_type);
            assert_eq!(value_type_from_json(&json).unwrap(), value_type, "{json}");
        }
    }

    #[test]
    fn restores_each_kind_of_fact_value() {
        let values = [
            FactValue::Text(ShortText::parse("Flugfeld Testwil").unwrap()),
            FactValue::Boolean(true),
            FactValue::Quantity(
                Range::new(Decimal::integer(15_000), Decimal::new(250_005, 1).unwrap()).unwrap(),
            ),
            FactValue::Money(Range::exact(MinorUnits::new(1500))),
            FactValue::Date(date(2030, 5, 18)),
            FactValue::DateWindow(
                DateWindow::new(date(2030, 5, 1), date(2030, 6, 30), Granularity::Month).unwrap(),
            ),
            FactValue::Choice(vec![
                ChoiceKey::parse("airshow").unwrap(),
                ChoiceKey::parse("catering").unwrap(),
            ]),
            FactValue::Reference(ReferenceId::Event(EventId::from_uuid(Uuid::now_v7()))),
        ];
        for value in values {
            round_trip(FactState::Accepted(valued(value.clone())));
            round_trip(FactState::Assumption(valued(value)));
        }
        round_trip(FactState::Unknown);
    }

    #[test]
    fn rejects_a_stored_value_that_breaks_a_rule() {
        let window = serde_json::json!({
            "type": "date_window", "start": "2030-06-30", "end": "2030-05-01", "granularity": "month",
        });
        assert!(fact_state_from_columns("accepted", Some(window), false).is_err());
        assert!(fact_state_from_columns("accepted", None, false).is_err());
        let unknown_value = Some(serde_json::json!({"type": "boolean", "value": true}));
        assert!(fact_state_from_columns("unknown", unknown_value, false).is_err());
    }
}
