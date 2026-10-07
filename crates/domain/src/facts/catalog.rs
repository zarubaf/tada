//! The shipped `core` catalog (ADR 0049): versioned seed data in code.
//!
//! The label of a core field is the Fluent message `field-<key>`.
//! The label of a choice is the Fluent message `field-<key>-<choice>`.
//! Both are in `locales/de-CH/web.ftl`.
//! A field keeps its ID and its key forever. Increase `CORE_CATALOG_VERSION` for each change of this catalog.

use uuid::{Uuid, uuid};

use super::field::{
    Description, FieldDefinition, FieldKey, FieldScope, FieldStatus, Label, MessageId, ModuleKey,
};
use super::value::{ChoiceKey, ChoiceValue, Currency, Granularity, Unit, ValueType};
use crate::ids::FieldDefinitionId;

/// The version of the `core` catalog.
pub const CORE_CATALOG_VERSION: u32 = 1;

/// The core fields that code computes with. The `app` crate reads them through typed accessors.
pub const RESERVED_KEYS: &[&str] = &["date_window", "exact_dates"];

const CORE_MODULE: &str = "core";

/// The field definitions of the `core` module.
pub fn core_catalog() -> Vec<FieldDefinition> {
    vec![
        core_field(
            uuid!("01a116d3-b70e-7215-91cb-7bc0ffc656e5"),
            "date_window",
            ValueType::DateWindow { granularity: None },
            "The period in which the event takes place, before the exact dates are known. \
             For example May to June 2030.",
        ),
        core_field(
            uuid!("01a116d3-b70f-7a18-a41f-74bc5baf41e5"),
            "exact_dates",
            ValueType::DateWindow {
                granularity: Some(Granularity::Day),
            },
            "The exact days of the event, from the first day to the last day.",
        ),
        core_field(
            uuid!("01a116d3-b710-7353-bfd5-4a46a388ca9e"),
            "duration_days",
            ValueType::Quantity { unit: unit("day") },
            "The number of days that the event is open to its audience.",
        ),
        core_field(
            uuid!("01a116d3-b711-77fe-946c-bc89e33b277f"),
            "audience",
            choice("audience", &["public", "members", "invited"], false),
            "Who can attend the event: the public, the members of the organization, \
             or invited persons only.",
        ),
        core_field(
            uuid!("01a116d3-b712-72fe-ae1c-b2c87b76635f"),
            "visitor_estimate",
            ValueType::Quantity {
                unit: unit("person_per_day"),
            },
            "The expected number of visitors on one day of the event.",
        ),
        core_field(
            uuid!("01a116d3-b713-770c-ab12-3ae59fe08f31"),
            "entry_fee_policy",
            choice("entry_fee_policy", &["free", "low", "regular"], false),
            "The policy for the entry fee: free entry, a low fee, or a regular fee. \
             The amount is in entry_fee_adult.",
        ),
        core_field(
            uuid!("01a116d3-b714-7562-b671-ea53c715c1d0"),
            "entry_fee_adult",
            ValueType::Money {
                currency: Currency::parse("CHF").expect("CHF is an ISO 4217 code"),
            },
            "The entry fee for one adult for one day.",
        ),
        core_field(
            uuid!("01a116d3-b715-76be-acf7-51880d981c54"),
            "components",
            choice(
                "components",
                &[
                    "airshow",
                    "static_display",
                    "catering",
                    "passenger_flights",
                    "exhibition",
                ],
                true,
            ),
            "The parts of the program of the event. An event can have several parts.",
        ),
        core_field(
            uuid!("01a116d3-b716-7011-896b-0bb0aa2fd1af"),
            "venue",
            ValueType::Text,
            "The place of the event, for example the name of an airfield.",
        ),
    ]
}

fn core_field(id: Uuid, key: &str, value_type: ValueType, description: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldDefinitionId::from_uuid(id),
        key: FieldKey::parse(key).expect("a core key is snake_case"),
        label: builtin_label(&format!("field-{key}")),
        value_type,
        description: Description::parse(description).expect("a core description is valid"),
        module: ModuleKey::parse(CORE_MODULE).expect("the core module key is snake_case"),
        status: FieldStatus::Active,
        scope: FieldScope::Shipped,
    }
}

fn choice(field_key: &str, choice_keys: &[&str], multiple: bool) -> ValueType {
    let values = choice_keys
        .iter()
        .map(|key| ChoiceValue {
            key: ChoiceKey::parse(key).expect("a core choice key is snake_case"),
            label: builtin_label(&format!("field-{field_key}-{key}")),
        })
        .collect();
    ValueType::Choice { values, multiple }
}

fn unit(key: &str) -> Unit {
    Unit::parse(key).expect("a core unit is snake_case")
}

fn builtin_label(id: &str) -> Label {
    Label::Builtin(MessageId::parse(id).expect("a core label is a Fluent message ID"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::ids::is_record_id;

    /// The IDs of the messages in a Fluent file.
    fn fluent_message_ids(path: &str) -> HashSet<String> {
        let file = std::fs::read_to_string(path).expect("the Fluent file is readable");
        file.lines()
            .filter_map(|line| line.split_once(" ="))
            .map(|(id, _)| id.to_owned())
            .filter(|id| MessageId::parse(id).is_some())
            .collect()
    }

    fn labels(field: &FieldDefinition) -> Vec<&Label> {
        let mut labels = vec![&field.label];
        if let ValueType::Choice { values, .. } = &field.value_type {
            labels.extend(values.iter().map(|value| &value.label));
        }
        labels
    }

    #[test]
    fn core_keys_and_ids_are_unique() {
        let catalog = core_catalog();
        let keys: HashSet<_> = catalog.iter().map(|field| field.key.as_str()).collect();
        let ids: HashSet<_> = catalog.iter().map(|field| field.id).collect();
        assert_eq!(keys.len(), catalog.len());
        assert_eq!(ids.len(), catalog.len());
        assert!(catalog.iter().all(|field| is_record_id(field.id.as_uuid())));
    }

    #[test]
    fn the_reserved_keys_are_core_fields() {
        let catalog = core_catalog();
        for reserved in RESERVED_KEYS {
            assert!(
                catalog.iter().any(|field| field.key.as_str() == *reserved),
                "{reserved}"
            );
        }
    }

    #[test]
    fn core_fields_are_shipped_active_and_in_the_core_module() {
        for field in core_catalog() {
            assert_eq!(field.module.as_str(), CORE_MODULE);
            assert_eq!(field.status, FieldStatus::Active);
            assert_eq!(field.scope, FieldScope::Shipped);
        }
    }

    #[test]
    fn each_core_field_and_choice_has_a_fluent_label() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/de-CH/web.ftl");
        let messages = fluent_message_ids(path);
        for field in core_catalog() {
            assert_eq!(
                field.label,
                builtin_label(&format!("field-{}", field.key.as_str()))
            );
            for label in labels(&field) {
                let Label::Builtin(id) = label else {
                    panic!("the core field {} has a text label", field.key.as_str());
                };
                assert!(messages.contains(id.as_str()), "{} is missing", id.as_str());
            }
        }
    }
}
