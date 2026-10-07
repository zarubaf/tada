//! Direct identifiers of a person (ADR 0035): the email address and the display name.
//!
//! `Debug` never shows the value, so a log line cannot leak it.

use std::fmt;

/// The longest email address that SMTP allows.
const EMAIL_MAX_CHARS: usize = 254;
const DISPLAY_NAME_MAX_CHARS: usize = 100;

/// The organization role of a member (glossary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizationRole {
    Owner,
    Admin,
    Member,
}

impl OrganizationRole {
    /// The kebab-case name that the database and the API use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "owner" => Some(Self::Owner),
            "admin" => Some(Self::Admin),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

/// The role of a member in one event (glossary, ADR 0052).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRole {
    EventManager,
    EventContributor,
    EventViewer,
}

impl EventRole {
    /// The kebab-case name that the database and the API use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EventManager => "event-manager",
            Self::EventContributor => "event-contributor",
            Self::EventViewer => "event-viewer",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "event-manager" => Some(Self::EventManager),
            "event-contributor" => Some(Self::EventContributor),
            "event-viewer" => Some(Self::EventViewer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EmailError {
    #[error("the email address has no single @ with text on both sides")]
    Shape,
    #[error("the email address is longer than {EMAIL_MAX_CHARS} characters")]
    TooLong,
    #[error("the email address contains a control character")]
    ControlCharacter,
}

/// A normalized email address. This is the only place that defines the normalization:
/// it trims the address and lowercases all of it.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    pub fn parse(input: &str) -> Result<Self, EmailError> {
        let address = input.trim().to_lowercase();
        if address.chars().any(char::is_control) {
            return Err(EmailError::ControlCharacter);
        }
        if address.chars().count() > EMAIL_MAX_CHARS {
            return Err(EmailError::TooLong);
        }
        match address.split_once('@') {
            Some((local, domain))
                if !local.is_empty() && !domain.is_empty() && !domain.contains('@') =>
            {
                Ok(Self(address))
            }
            _ => Err(EmailError::Shape),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Email(redacted)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DisplayNameError {
    #[error("the display name is empty")]
    Empty,
    #[error("the display name is longer than {DISPLAY_NAME_MAX_CHARS} characters")]
    TooLong,
    #[error("the display name contains a control character")]
    ControlCharacter,
}

/// The name that other members see: 1 to 100 characters, trimmed.
#[derive(Clone, PartialEq, Eq)]
pub struct DisplayName(String);

impl DisplayName {
    pub fn parse(input: &str) -> Result<Self, DisplayNameError> {
        let name = input.trim();
        if name.chars().any(char::is_control) {
            return Err(DisplayNameError::ControlCharacter);
        }
        if name.is_empty() {
            return Err(DisplayNameError::Empty);
        }
        if name.chars().count() > DISPLAY_NAME_MAX_CHARS {
            return Err(DisplayNameError::TooLong);
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DisplayName(redacted)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_have_stable_kebab_case_names() {
        for role in [
            OrganizationRole::Owner,
            OrganizationRole::Admin,
            OrganizationRole::Member,
        ] {
            assert_eq!(OrganizationRole::parse(role.as_str()), Some(role));
        }
        for role in [
            EventRole::EventManager,
            EventRole::EventContributor,
            EventRole::EventViewer,
        ] {
            assert_eq!(EventRole::parse(role.as_str()), Some(role));
        }
        assert_eq!(EventRole::EventViewer.as_str(), "event-viewer");
        assert_eq!(OrganizationRole::parse("Owner"), None);
        assert_eq!(EventRole::parse("owner"), None);
    }

    #[test]
    fn normalizes_an_email_address() {
        let email = Email::parse(" Anna@Example.ORG ").unwrap();
        assert_eq!(email.as_str(), "anna@example.org");
    }

    #[test]
    fn rejects_malformed_email_addresses() {
        for input in ["", "anna", "@example.org", "anna@", "a@b@example.org"] {
            assert_eq!(Email::parse(input), Err(EmailError::Shape), "{input:?}");
        }
        assert_eq!(
            Email::parse("an\nna@example.org"),
            Err(EmailError::ControlCharacter)
        );
        let long = format!("{}@example.org", "a".repeat(243));
        assert_eq!(long.chars().count(), 255);
        assert_eq!(Email::parse(&long), Err(EmailError::TooLong));
        let longest = format!("{}@example.org", "a".repeat(242));
        assert!(Email::parse(&longest).is_ok());
    }

    #[test]
    fn debug_of_an_email_hides_the_address() {
        let email = Email::parse("anna@example.org").unwrap();
        assert!(!format!("{email:?}").contains("anna"));
    }

    #[test]
    fn trims_a_display_name() {
        assert_eq!(
            DisplayName::parse("  Anna Muster ").unwrap().as_str(),
            "Anna Muster"
        );
    }

    #[test]
    fn rejects_invalid_display_names() {
        assert_eq!(DisplayName::parse("  "), Err(DisplayNameError::Empty));
        assert_eq!(
            DisplayName::parse("Anna\tMuster"),
            Err(DisplayNameError::ControlCharacter)
        );
        assert_eq!(
            DisplayName::parse(&"a".repeat(101)),
            Err(DisplayNameError::TooLong)
        );
        assert!(DisplayName::parse(&"a".repeat(100)).is_ok());
    }

    #[test]
    fn debug_of_a_display_name_hides_the_name() {
        let name = DisplayName::parse("Anna Muster").unwrap();
        assert!(!format!("{name:?}").contains("Anna"));
    }
}
