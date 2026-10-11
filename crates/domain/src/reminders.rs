//! Rules for the reminders of the AI-PM: the policy, which records to remind and the answer buttons.
//! Pure functions; the caller supplies the clock and the stored state.

use jiff::Timestamp;
use jiff::civil::{Date, Time};
use uuid::Uuid;

use crate::RecordVersion;
use crate::events::EventTimeZone;
use crate::ids::DigestItemId;
use crate::schedule::{
    LocalSchedule, QuietHours, Weekdays, add_working_days, tz_of, working_days_since,
};

/// A value of a command that the rules reject. `code` is a field code of `validation-failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldError {
    pub field: &'static str,
    pub code: &'static str,
}

const OUT_OF_RANGE: &str = "out-of-range";

/// The settings of the reminders of one organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmPolicy {
    pub schedule: LocalSchedule,
    pub quiet: QuietHours,
    pub cooldown_hours: u8,
    pub follow_up_working_days: u8,
    pub look_ahead_working_days: u8,
    pub paused: bool,
    pub ai_enabled: bool,
}

impl PmPolicy {
    /// Checks every value and returns all errors at once.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time: Time,
        time_zone: &str,
        weekdays: u8,
        quiet_start: Time,
        quiet_end: Time,
        cooldown_hours: u8,
        follow_up_working_days: u8,
        look_ahead_working_days: u8,
        paused: bool,
        ai_enabled: bool,
    ) -> Result<Self, Vec<FieldError>> {
        let mut errors = Vec::new();
        let zone = EventTimeZone::parse(time_zone);
        if zone.is_err() {
            errors.push(FieldError {
                field: "time_zone",
                code: "invalid-time-zone",
            });
        }
        let weekdays = Weekdays::from_bits(weekdays);
        if weekdays.is_err() {
            errors.push(FieldError {
                field: "weekdays",
                code: OUT_OF_RANGE,
            });
        }
        for (field, value, range) in [
            ("cooldown_hours", cooldown_hours, 1..=168),
            ("follow_up_working_days", follow_up_working_days, 1..=10),
            ("look_ahead_working_days", look_ahead_working_days, 0..=10),
        ] {
            if !range.contains(&value) {
                errors.push(FieldError {
                    field,
                    code: OUT_OF_RANGE,
                });
            }
        }
        match (zone, weekdays) {
            (Ok(zone), Ok(weekdays)) if errors.is_empty() => Ok(Self {
                schedule: LocalSchedule {
                    time,
                    zone,
                    weekdays,
                },
                quiet: QuietHours {
                    start: quiet_start,
                    end: quiet_end,
                },
                cooldown_hours,
                follow_up_working_days,
                look_ahead_working_days,
                paused,
                ai_enabled,
            }),
            _ => Err(errors),
        }
    }
}

/// The defaults of a new organization: 08:30 Europe/Zurich, Monday to Friday, quiet 20:00 to 08:00,
/// cooldown 20 hours, follow-up and look-ahead 2 working days, not paused, AI off.
impl Default for PmPolicy {
    fn default() -> Self {
        Self {
            schedule: LocalSchedule {
                time: Time::constant(8, 30, 0, 0),
                zone: EventTimeZone::default_zone(),
                weekdays: Weekdays::MON_FRI,
            },
            quiet: QuietHours {
                start: Time::constant(20, 0, 0, 0),
                end: Time::constant(8, 0, 0, 0),
            },
            cooldown_hours: 20,
            follow_up_working_days: 2,
            look_ahead_working_days: 2,
            paused: false,
            ai_enabled: false,
        }
    }
}

macro_rules! code_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }

            /// Returns `None` for a code that is not one of the variants.
            pub fn parse(code: &str) -> Option<Self> {
                match code { $($code => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

code_enum!(
    /// The kind of record a reminder is about.
    RecordKind { Action => "action", Commitment => "commitment" }
);
code_enum!(
    /// Why an item is in a digest.
    Reason { Overdue => "overdue", DueSoon => "due-soon", Blocked => "blocked", FollowUp => "follow-up" }
);
code_enum!(
    /// The answer of a member to an item, from a button.
    Answer { OnTrack => "on-track", Blocked => "blocked", Done => "done", Fulfilled => "fulfilled" }
);

/// A record that could be in a digest of its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub kind: RecordKind,
    pub id: Uuid,
    pub version: RecordVersion,
    pub due_date: Option<Date>,
    pub blocked: bool,
}

/// What the last digest to this person said about a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    pub notified_version: RecordVersion,
    pub notified_at: Timestamp,
    pub response: Option<Answer>,
    pub response_version: Option<RecordVersion>,
    pub response_at: Option<Timestamp>,
}

/// The context of a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub today: Date,
    pub now: Timestamp,
    pub zone: EventTimeZone,
    pub days: Weekdays,
    pub look_ahead: u8,
    pub follow_up: u8,
}

/// The reason to put a record in a digest, or `None` to leave it out.
pub fn select(c: &Candidate, m: Option<&Memory>, r: &Rules) -> Option<Reason> {
    let overdue = c.due_date.is_some_and(|due| due < r.today);
    let base = if overdue {
        Reason::Overdue
    } else if c
        .due_date
        .is_some_and(|due| due <= add_working_days(r.today, r.look_ahead, r.days))
    {
        Reason::DueSoon
    } else if c.blocked {
        Reason::Blocked
    } else {
        return None;
    };
    let Some(m) = m else { return Some(base) };
    if c.version != m.notified_version {
        return Some(base);
    }
    if m.response_version == Some(c.version) && m.response.is_some() {
        // Answered. Remind again only once, when the item became overdue after the answer.
        let since = c.due_date.and_then(|due| overdue_since(due, &r.zone))?;
        let answered_before = m.response_at.is_some_and(|at| at < since);
        return (overdue && answered_before && m.notified_at < since).then_some(Reason::Overdue);
    }
    (working_days_since(m.notified_at, r.now, &r.zone, r.days) >= u32::from(r.follow_up))
        .then_some(Reason::FollowUp)
}

/// The start of the day after the due date: the first instant at which the record is overdue.
fn overdue_since(due: Date, zone: &EventTimeZone) -> Option<Timestamp> {
    let start = due.tomorrow().ok()?.to_datetime(Time::midnight());
    tz_of(zone)
        .to_ambiguous_zoned(start)
        .compatible()
        .ok()
        .map(|z| z.timestamp())
}

pub use crate::schedule::in_cooldown;

/// The data of an inline button: the answer and the opaque reference of the digest item.
/// The text is `<answer>:<22 characters>`, at most 64 bytes (Telegram limit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackData {
    pub answer: Answer,
    pub item: DigestItemId,
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const ENCODED_LEN: usize = 22;

impl CallbackData {
    pub fn encode(&self) -> String {
        let mut text = format!("{}:", self.answer.as_str());
        text.push_str(&base64url(self.item.as_uuid().as_bytes()));
        text
    }

    /// Returns `None` for anything that `encode` does not produce.
    pub fn decode(text: &str) -> Option<Self> {
        let (code, rest) = text.split_once(':')?;
        let answer = Answer::parse(code)?;
        let bytes = unbase64url(rest)?;
        Some(Self {
            answer,
            item: DigestItemId::from_uuid(Uuid::from_bytes(bytes)),
        })
    }
}

/// Base64url without padding: 16 bytes make 22 characters.
fn base64url(bytes: &[u8; 16]) -> String {
    let mut out = String::with_capacity(ENCODED_LEN);
    for chunk in bytes.chunks(3) {
        let mut word = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            word |= u32::from(*b) << (16 - 8 * i);
        }
        for i in 0..=chunk.len() {
            out.push(char::from(ALPHABET[((word >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// The inverse of `base64url`. It rejects another length, another alphabet and a tail that holds bits
/// the encoder never sets, so each item has one text.
fn unbase64url(text: &str) -> Option<[u8; 16]> {
    if text.len() != ENCODED_LEN {
        return None;
    }
    let mut out = [0u8; 16];
    let mut written = 0;
    for chunk in text.as_bytes().chunks(4) {
        let mut word = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            let value = ALPHABET.iter().position(|a| a == c)?;
            word |= u32::try_from(value).ok()? << (18 - 6 * i);
        }
        for i in 0..chunk.len() - 1 {
            out[written] = u8::try_from((word >> (16 - 8 * i)) & 0xFF).ok()?;
            written += 1;
        }
    }
    (base64url(&out) == text).then_some(out)
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::civil::{Date, date, time};
    use uuid::Uuid;

    use super::*;
    use crate::RecordVersion;
    use crate::events::EventTimeZone;
    use crate::schedule::Weekdays;

    fn ts(s: &str) -> Timestamp {
        s.parse().expect("test timestamp")
    }

    fn v(n: i64) -> RecordVersion {
        RecordVersion::new(n).expect("test version")
    }

    fn rules(today: Date, now: &str) -> Rules {
        Rules {
            today,
            now: ts(now),
            zone: EventTimeZone::default_zone(),
            days: Weekdays::MON_FRI,
            look_ahead: 2,
            follow_up: 2,
        }
    }

    fn candidate(due: Option<Date>, blocked: bool, version: i64) -> Candidate {
        Candidate {
            kind: RecordKind::Action,
            id: Uuid::nil(),
            version: v(version),
            due_date: due,
            blocked,
        }
    }

    fn memory(version: i64, notified_at: &str) -> Memory {
        Memory {
            notified_version: v(version),
            notified_at: ts(notified_at),
            response: None,
            response_version: None,
            response_at: None,
        }
    }

    #[test]
    fn select_new_item() {
        let r = rules(date(2026, 10, 9), "2026-10-09T06:30:00Z");
        assert_eq!(
            select(&candidate(Some(date(2026, 10, 8)), false, 1), None, &r),
            Some(Reason::Overdue)
        );
        assert_eq!(
            select(&candidate(Some(date(2026, 10, 9)), false, 1), None, &r),
            Some(Reason::DueSoon)
        );
        assert_eq!(
            select(&candidate(None, true, 1), None, &r),
            Some(Reason::Blocked)
        );
        assert_eq!(select(&candidate(None, false, 1), None, &r), None);
    }

    #[test]
    fn select_changed_version() {
        let r = rules(date(2026, 10, 9), "2026-10-09T06:30:00Z");
        let m = memory(1, "2026-10-08T06:30:00Z");
        let c = candidate(Some(date(2026, 10, 9)), false, 2);
        assert_eq!(select(&c, Some(&m), &r), Some(Reason::DueSoon));
    }

    #[test]
    fn select_follow_up_after_two_working_days() {
        let m = memory(1, "2026-10-09T06:30:00Z"); // Friday
        let c = candidate(Some(date(2026, 10, 20)), true, 1);
        // Monday: 1 working day.
        let monday = rules(date(2026, 10, 12), "2026-10-12T06:30:00Z");
        assert_eq!(select(&c, Some(&m), &monday), None);
        // Tuesday: 2 working days.
        let tuesday = rules(date(2026, 10, 13), "2026-10-13T06:30:00Z");
        assert_eq!(select(&c, Some(&m), &tuesday), Some(Reason::FollowUp));
    }

    #[test]
    fn select_skips_answered_unless_overdue_after() {
        let r = rules(date(2026, 10, 13), "2026-10-13T06:30:00Z");
        let mut m = memory(1, "2026-10-08T06:30:00Z");
        m.response = Some(Answer::OnTrack);
        m.response_version = Some(v(1));
        // Answered before the item became overdue (due 2026-10-12; overdue from 10-13 local).
        m.response_at = Some(ts("2026-10-09T08:00:00Z"));
        let overdue = candidate(Some(date(2026, 10, 12)), false, 1);
        assert_eq!(select(&overdue, Some(&m), &r), Some(Reason::Overdue));
        // Answered after it became overdue: skip.
        m.response_at = Some(ts("2026-10-13T08:00:00Z"));
        assert_eq!(select(&overdue, Some(&m), &r), None);
        // Answered, not overdue, follow-up time passed: skip.
        m.response_at = Some(ts("2026-10-08T08:00:00Z"));
        let due_soon = candidate(Some(date(2026, 10, 14)), false, 1);
        assert_eq!(select(&due_soon, Some(&m), &r), None);
        // Already reminded after it became overdue: do not repeat.
        m.response_at = Some(ts("2026-10-09T08:00:00Z"));
        m.notified_at = ts("2026-10-13T06:00:00Z");
        assert_eq!(select(&overdue, Some(&m), &r), None);
    }

    #[test]
    fn select_ignores_far_due() {
        let r = rules(date(2026, 10, 9), "2026-10-09T06:30:00Z");
        assert_eq!(
            select(&candidate(Some(date(2026, 10, 14)), false, 1), None, &r),
            None
        );
    }

    #[test]
    fn callback_round_trip_and_rejects_garbage() {
        let item =
            DigestItemId::from_uuid(Uuid::from_u128(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210));
        for answer in [
            Answer::OnTrack,
            Answer::Blocked,
            Answer::Done,
            Answer::Fulfilled,
        ] {
            let data = CallbackData { answer, item };
            let text = data.encode();
            assert!(text.len() <= 64);
            assert!(text.starts_with(answer.as_str()));
            assert_eq!(CallbackData::decode(&text), Some(data));
        }
        assert_eq!(CallbackData::decode(""), None);
        assert_eq!(CallbackData::decode("done"), None);
        assert_eq!(CallbackData::decode("done:short"), None);
        assert_eq!(CallbackData::decode("other:AAAAAAAAAAAAAAAAAAAAAA"), None);
        assert_eq!(CallbackData::decode("done:AAAAAAAAAAAAAAAAAAAAA+"), None);
        assert_eq!(CallbackData::decode("done:AAAAAAAAAAAAAAAAAAAAAB"), None); // non-canonical tail
        assert_eq!(CallbackData::decode("done:AAAAAAAAAAAAAAAAAAAAAAA"), None);
    }

    fn policy(
        cooldown: u8,
        follow_up: u8,
        look_ahead: u8,
        tz: &str,
        days: u8,
    ) -> Result<PmPolicy, Vec<FieldError>> {
        PmPolicy::new(
            time(8, 30, 0, 0),
            tz,
            days,
            time(20, 0, 0, 0),
            time(8, 0, 0, 0),
            cooldown,
            follow_up,
            look_ahead,
            false,
            false,
        )
    }

    #[test]
    fn policy_rejects_out_of_range() {
        assert_eq!(
            policy(20, 2, 2, "Europe/Zurich", 0b1_1111).unwrap(),
            PmPolicy::default()
        );
        assert!(policy(1, 1, 0, "Europe/Zurich", 0b1_1111).is_ok());
        assert!(policy(168, 10, 10, "Europe/Zurich", 0b1_1111).is_ok());
        let errors = policy(0, 0, 11, "Mars/Base", 0).unwrap_err();
        let codes: Vec<_> = errors.iter().map(|e| (e.field, e.code)).collect();
        assert_eq!(
            codes,
            [
                ("time_zone", "invalid-time-zone"),
                ("weekdays", "out-of-range"),
                ("cooldown_hours", "out-of-range"),
                ("follow_up_working_days", "out-of-range"),
                ("look_ahead_working_days", "out-of-range"),
            ]
        );
        assert_eq!(policy(169, 2, 2, "Europe/Zurich", 1).unwrap_err().len(), 1);
    }
}
