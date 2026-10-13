//! Events (ADR 0049): one occurrence of an event, with its key, name and time zone.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::RecordVersion;
use crate::ids::{EventId, OrganizationId};
use crate::name::{self, NameError};
use crate::work::InvalidTransition;

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
    pub status: EventStatus,
    /// Set exactly when the event is closed. Retention counts its period from this time (ADR 0071).
    pub closed_at: Option<Timestamp>,
    /// Set when retention redacted the event (ADR 0072). Such an event stays closed.
    pub redacted_at: Option<Timestamp>,
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
    /// The rule of `parse` as a regular expression, for the JSON Schema of an input.
    /// Change it together with `parse`.
    pub const PATTERN: &str = "^[A-Z0-9]{2,8}$";

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

/// The name of an event, with the rules of `crate::name`.
#[derive(Clone, PartialEq, Eq)]
pub struct EventName(String);

impl EventName {
    /// Removes the spaces at the ends, then checks the value.
    pub fn parse(value: &str) -> Result<Self, NameError> {
        name::parse(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An event name can contain a name of a person, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for EventName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventName({} characters)", self.0.chars().count())
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

    /// The date of `now` in this time zone: "today" of the event.
    pub fn today(&self, now: Timestamp) -> jiff::civil::Date {
        now.to_zoned(crate::schedule::tz_of(self)).date()
    }
}

/// The status of an event: it takes changes while active and only reads once closed (ADR 0045).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not an event status")]
pub struct UnknownEventStatus;

impl EventStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

impl std::str::FromStr for EventStatus {
    type Err = UnknownEventStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "closed" => Ok(Self::Closed),
            _ => Err(UnknownEventStatus),
        }
    }
}

/// Only an active event closes.
pub fn can_close(status: EventStatus) -> Result<(), InvalidTransition> {
    if status == EventStatus::Active {
        Ok(())
    } else {
        Err(InvalidTransition)
    }
}

/// Only a closed event reopens, and only if no redaction touched it: a redaction removes content for good.
pub fn can_reopen(status: EventStatus, redacted: bool) -> Result<(), InvalidTransition> {
    if status == EventStatus::Closed && !redacted {
        Ok(())
    } else {
        Err(InvalidTransition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_refused_after_redaction() {
        assert_eq!(can_reopen(EventStatus::Closed, false), Ok(()));
        assert_eq!(
            can_reopen(EventStatus::Closed, true),
            Err(InvalidTransition)
        );
        assert_eq!(
            can_reopen(EventStatus::Active, false),
            Err(InvalidTransition)
        );
    }

    #[test]
    fn only_an_active_event_closes() {
        assert_eq!(can_close(EventStatus::Active), Ok(()));
        assert_eq!(can_close(EventStatus::Closed), Err(InvalidTransition));
    }

    #[test]
    fn event_status_round_trips() {
        for status in [EventStatus::Active, EventStatus::Closed] {
            assert_eq!(status.as_str().parse::<EventStatus>(), Ok(status));
        }
    }

    #[test]
    fn debug_of_an_event_name_hides_the_name() {
        let name = EventName::parse("Erika Muster Party").unwrap();
        assert!(!format!("{name:?}").contains("Erika"));
    }

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
    fn an_event_name_has_the_rules_of_a_name() {
        assert_eq!(
            EventName::parse("  Open Day Testwil ").unwrap().as_str(),
            "Open Day Testwil"
        );
        assert_eq!(EventName::parse(" "), Err(NameError::Empty));
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

    #[test]
    fn today_is_the_date_in_the_time_zone_of_the_event() {
        let now: Timestamp = "2030-05-17T22:30:00Z".parse().unwrap();
        let zurich = EventTimeZone::parse("Europe/Zurich").unwrap();
        let new_york = EventTimeZone::parse("America/New_York").unwrap();
        assert_eq!(zurich.today(now), jiff::civil::date(2030, 5, 18));
        assert_eq!(new_york.today(now), jiff::civil::date(2030, 5, 17));
    }
}
