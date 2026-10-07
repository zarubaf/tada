//! Field definitions (ADR 0049, layer 3): the records of the field catalog.

use super::value::{ShortText, ValueType};
use super::{TextError, checked_text, snake_case_key};
use crate::ids::{EventId, FieldDefinitionId};

/// One field that facts can use. The key and the value type never change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDefinition {
    pub id: FieldDefinitionId,
    pub key: FieldKey,
    pub label: Label,
    pub value_type: ValueType,
    /// The meaning of the field, in English, for people and for AI agents.
    pub description: Description,
    pub module: ModuleKey,
    pub status: FieldStatus,
    pub scope: FieldScope,
}

snake_case_key!(
    /// The stable key of a field, for example `visitor_estimate`: `snake_case`, 2 to 64 characters (ADR 0044).
    FieldKey
);

snake_case_key!(
    /// The key of a module, for example `core` or `aviation`.
    ModuleKey
);

/// The label that people see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// A Fluent message of a shipped field, in `locales/de-CH/web.ftl`.
    Builtin(MessageId),
    /// The German text of a field that the project added.
    Text(ShortText),
}

/// The ID of a Fluent message: an ASCII letter, then ASCII letters, digits, `-` and `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(String);

impl MessageId {
    /// Returns `None` if `id` is not a Fluent message ID.
    pub fn parse(id: &str) -> Option<Self> {
        let mut bytes = id.bytes();
        let first_is_letter = bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic());
        let rest_is_valid =
            bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        (first_is_letter && rest_is_valid).then(|| Self(id.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The description of a field: 1 to 2000 characters, without control characters and without spaces at the ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description(String);

impl Description {
    pub const MAX_CHARS: usize = 2000;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A field is never deleted. A deprecated field is readable, but closed for new facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldStatus {
    Active,
    Deprecated,
}

/// Where a field definition belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldScope {
    /// tada ships the field in a module catalog.
    Shipped,
    /// The field belongs to one event.
    Event(EventId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::KeyError;

    #[test]
    fn rejects_a_field_key_that_is_not_snake_case() {
        assert_eq!(FieldKey::parse("Date-Window"), Err(KeyError::Characters));
        assert_eq!(FieldKey::parse("d"), Err(KeyError::Length));
        assert_eq!(
            FieldKey::parse("date_window").unwrap().as_str(),
            "date_window"
        );
    }

    #[test]
    fn accepts_only_fluent_message_ids() {
        assert_eq!(
            MessageId::parse("field-date_window").unwrap().as_str(),
            "field-date_window"
        );
        for id in ["", "-field", "1field", "field date", "feld-ä"] {
            assert_eq!(MessageId::parse(id), None, "{id:?}");
        }
    }
}
