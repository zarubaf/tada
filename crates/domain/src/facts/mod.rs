//! Facts (ADR 0049): value types, fact states, field definitions and the shipped `core` catalog.
//!
//! This code module holds layers 2 and 3 of ADR 0049.
//! A constructor checks each part of a value, and `FactValue::check` compares a value with its value type.
//! There are no floats, so a value cannot lose precision.
//!
//! This crate has no `serde`.
//! The JSON parse of values lives in `app`, in the input type of a proposal.
//! That input type maps to the constructors of this code module.

mod catalog;
mod field;
mod state;
mod value;

pub use catalog::{CORE_CATALOG_VERSION, RESERVED_KEYS, core_catalog};
pub use field::{
    Description, FieldDefinition, FieldKey, FieldScope, FieldStatus, Label, MessageId, ModuleKey,
};
pub use state::FactState;
pub use value::{
    ChoiceKey, ChoiceValue, Currency, DateWindow, Decimal, FactValue, Granularity, MinorUnits,
    Range, ReferenceId, ReferenceTarget, ShortText, Unit, ValueError, ValueType, Valued,
};

/// The error of a `snake_case` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("a key has 2 to 64 characters")]
    Length,
    #[error(
        "a key is snake_case: a lowercase letter, then lowercase letters, digits and single underscores"
    )]
    Characters,
}

/// The error of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("the text is empty")]
    Empty,
    #[error("the text is too long")]
    TooLong,
    #[error("the text contains a control character")]
    ControlCharacter,
}

/// Checks a `snake_case` key of 2 to 64 characters (ADR 0044).
fn check_snake_case(value: &str) -> Result<(), KeyError> {
    if !(2..=64).contains(&value.len()) {
        return Err(KeyError::Length);
    }
    let bytes = value.as_bytes();
    let well_formed = bytes[0].is_ascii_lowercase()
        && bytes[bytes.len() - 1] != b'_'
        && !value.contains("__")
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_');
    if well_formed {
        Ok(())
    } else {
        Err(KeyError::Characters)
    }
}

/// Removes the spaces at the ends, then checks for 1 to `max_chars` characters without control characters.
pub(crate) fn checked_text(input: &str, max_chars: usize) -> Result<String, TextError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(TextError::Empty);
    }
    if text.chars().count() > max_chars {
        return Err(TextError::TooLong);
    }
    if text.chars().any(char::is_control) {
        return Err(TextError::ControlCharacter);
    }
    Ok(text.to_owned())
}

/// Defines a newtype for a `snake_case` key.
macro_rules! snake_case_key {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, $crate::facts::KeyError> {
                $crate::facts::check_snake_case(value)?;
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
use snake_case_key;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_snake_case_keys() {
        for key in [
            "ab",
            "date_window",
            "entry_fee_adult",
            "a1_b2",
            &"a".repeat(64),
        ] {
            assert_eq!(check_snake_case(key), Ok(()), "{key:?}");
        }
    }

    #[test]
    fn rejects_keys_that_are_not_snake_case() {
        assert_eq!(check_snake_case("a"), Err(KeyError::Length));
        assert_eq!(check_snake_case(&"a".repeat(65)), Err(KeyError::Length));
        for key in [
            "Date-Window",
            "date-window",
            "dateWindow",
            "_date",
            "date_",
            "date__window",
            "1date",
            "dätum",
        ] {
            assert_eq!(check_snake_case(key), Err(KeyError::Characters), "{key:?}");
        }
    }

    #[test]
    fn trims_and_checks_texts() {
        assert_eq!(checked_text("  Testwil ", 10).unwrap(), "Testwil");
        assert_eq!(checked_text(" ", 10), Err(TextError::Empty));
        assert_eq!(checked_text(&"ä".repeat(11), 10), Err(TextError::TooLong));
        assert_eq!(
            checked_text("a\u{7}b", 10),
            Err(TextError::ControlCharacter)
        );
    }
}
