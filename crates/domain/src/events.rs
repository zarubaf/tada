//! Events (ADR 0049): one occurrence of an event, with its key, name and time zone.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::RecordVersion;
use crate::ids::{EventId, OrganizationId};

/// An event. All fields hold checked values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: EventId,
    pub organization_id: OrganizationId,
    pub key: EventKey,
    pub name: EventName,
    pub time_zone: EventTimeZone,
    pub version: RecordVersion,
    pub created_at: Timestamp,
}

/// The short key of an event, unique in its organization, for example `FLY28` (ADR 0038):
/// two to eight capital letters and digits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EventKeyError {
    #[error("an event key has two to eight characters")]
    Length,
    #[error("an event key has only capital letters A to Z and digits")]
    Characters,
}

impl EventKey {
    pub fn parse(value: &str) -> Result<Self, EventKeyError> {
        if !(2..=8).contains(&value.len()) {
            return Err(EventKeyError::Length);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Err(EventKeyError::Characters);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of an event: 1 to 200 characters, without control characters and without spaces at the ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EventNameError {
    #[error("an event name is not empty")]
    Empty,
    #[error("an event name has at most 200 characters")]
    TooLong,
    #[error("an event name has no control characters")]
    ControlCharacter,
}

impl EventName {
    pub const MAX_CHARS: usize = 200;

    /// Removes the spaces at the ends, then checks the value.
    pub fn parse(value: &str) -> Result<Self, EventNameError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(EventNameError::Empty);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(EventNameError::TooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(EventNameError::ControlCharacter);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The IANA time zone of an event, for example `Europe/Zurich` (ADR 0038).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTimeZone(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the time zone is not a known IANA time zone")]
pub struct UnknownTimeZone;

impl EventTimeZone {
    /// The time zone of an event that names none.
    pub fn default_zone() -> Self {
        Self("Europe/Zurich".to_owned())
    }

    /// Accepts the IANA names of the time zone database in the binary. A fixed offset is not an IANA name.
    pub fn parse(name: &str) -> Result<Self, UnknownTimeZone> {
        let zone = TimeZone::get(name).map_err(|_| UnknownTimeZone)?;
        let canonical = zone.iana_name().ok_or(UnknownTimeZone)?;
        Ok(Self(canonical.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_keys_of_two_to_eight_capital_letters_and_digits() {
        for key in ["FL", "FLY28", "ABCDEFG8"] {
            assert_eq!(EventKey::parse(key).unwrap().as_str(), key);
        }
    }

    #[test]
    fn rejects_invalid_keys() {
        assert_eq!(EventKey::parse("F"), Err(EventKeyError::Length));
        assert_eq!(EventKey::parse("ABCDEFGHI"), Err(EventKeyError::Length));
        assert_eq!(EventKey::parse("fly28"), Err(EventKeyError::Characters));
        assert_eq!(EventKey::parse("FLY-28"), Err(EventKeyError::Characters));
        assert_eq!(EventKey::parse("ÄBC"), Err(EventKeyError::Characters));
    }

    #[test]
    fn trims_and_checks_names() {
        assert_eq!(
            EventName::parse("  Open Day Testwil ").unwrap().as_str(),
            "Open Day Testwil"
        );
        assert_eq!(EventName::parse(" \t"), Err(EventNameError::Empty));
        assert_eq!(
            EventName::parse(&"ä".repeat(201)),
            Err(EventNameError::TooLong)
        );
        assert!(EventName::parse(&"ä".repeat(200)).is_ok());
        assert_eq!(
            EventName::parse("Open\u{0}Day"),
            Err(EventNameError::ControlCharacter)
        );
    }

    #[test]
    fn accepts_only_iana_time_zones() {
        assert_eq!(
            EventTimeZone::parse("Europe/Zurich").unwrap().as_str(),
            "Europe/Zurich"
        );
        assert_eq!(
            EventTimeZone::parse("Europe/Atlantis"),
            Err(UnknownTimeZone)
        );
        assert_eq!(EventTimeZone::parse("+01:00"), Err(UnknownTimeZone));
    }
}
