//! The rules of a record name, for example the name of an event:
//! 1 to 200 characters, without control characters and without spaces at the ends.

/// The maximum number of characters of a name.
pub const NAME_MAX_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("a name is not empty")]
    Empty,
    #[error("a name has at most {NAME_MAX_CHARS} characters")]
    TooLong,
    #[error("a name has no control characters")]
    ControlCharacter,
}

/// Removes the spaces at the ends, then checks the value.
pub(crate) fn parse(value: &str) -> Result<String, NameError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(NameError::Empty);
    }
    if value.chars().count() > NAME_MAX_CHARS {
        return Err(NameError::TooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(NameError::ControlCharacter);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_checks_names() {
        assert_eq!(parse("  Open Day Testwil ").unwrap(), "Open Day Testwil");
        assert_eq!(parse(" \t"), Err(NameError::Empty));
        assert_eq!(parse(&"ä".repeat(201)), Err(NameError::TooLong));
        assert!(parse(&"ä".repeat(200)).is_ok());
        assert_eq!(parse("Open\u{0}Day"), Err(NameError::ControlCharacter));
    }
}
