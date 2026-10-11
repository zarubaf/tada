//! Local-time schedules, working days and quiet hours (ADR 0038). Pure functions of instants and
//! civil times; the caller supplies the clock.

use jiff::civil::{Date, Time, Weekday};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::events::EventTimeZone;

/// A value outside its allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the value is outside its allowed range")]
pub struct RangeError;

/// The days of the week on which something runs: a non-empty set. Bit 0 is Monday, bit 6 is Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekdays(u8);

impl Weekdays {
    pub const MON_FRI: Self = Self(0b001_1111);

    /// Accepts bits 0 to 6 with at least one bit set.
    pub fn from_bits(bits: u8) -> Result<Self, RangeError> {
        if bits == 0 || bits > 0b111_1111 {
            return Err(RangeError);
        }
        Ok(Self(bits))
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & (1 << day.to_monday_zero_offset()) != 0
    }
}

/// The time zone data of a validated zone name. The binary bundles the data, so a name that
/// `EventTimeZone` accepted is always found.
pub(crate) fn tz_of(zone: &EventTimeZone) -> TimeZone {
    TimeZone::get(zone.as_str()).expect("EventTimeZone holds a name of the bundled database")
}

/// The instant of a civil time on a date. A gap moves the time forward and a fold takes the first
/// occurrence (ADR 0038).
pub(crate) fn instant(date: Date, time: Time, tz: &TimeZone) -> Option<Timestamp> {
    tz.to_ambiguous_zoned(date.to_datetime(time))
        .compatible()
        .ok()
        .map(|zoned| zoned.timestamp())
}

/// A time of day on some weekdays in a time zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSchedule {
    pub time: Time,
    pub zone: EventTimeZone,
    pub weekdays: Weekdays,
}

/// The result of `LocalSchedule::due`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    /// The runs to do now.
    pub runs: u16,
    /// The first slot after `now`.
    pub next_run_at: Timestamp,
}

impl LocalSchedule {
    /// The first slot strictly after `after`.
    pub fn next_after(&self, after: Timestamp) -> Timestamp {
        let tz = tz_of(&self.zone);
        let first = after.to_zoned(tz.clone()).date();
        // A week holds a slot of every allowed day; one more day covers a gap that moves a slot.
        let mut date = first;
        for _ in 0..9 {
            if self.weekdays.contains(date.weekday())
                && let Some(slot) = instant(date, self.time, &tz)
                && slot > after
            {
                return slot;
            }
            match date.tomorrow() {
                Ok(next) => date = next,
                Err(_) => break,
            }
        }
        after
    }

    /// Counts the slots from `next_run_at` up to `now`. `runs` is at most `missed_limit`: a long
    /// outage does not cause a burst.
    pub fn due(&self, next_run_at: Timestamp, now: Timestamp, missed_limit: u16) -> Due {
        let mut slot = next_run_at;
        let mut missed: u32 = 0;
        while slot <= now {
            missed += 1;
            let next = self.next_after(slot);
            if next <= slot {
                break;
            }
            slot = next;
        }
        Due {
            runs: u16::try_from(missed).unwrap_or(u16::MAX).min(missed_limit),
            next_run_at: slot,
        }
    }
}

/// The date `n` working days after `date`.
pub fn add_working_days(date: Date, n: u8, days: Weekdays) -> Date {
    let mut current = date;
    let mut left = n;
    while left > 0 {
        match current.tomorrow() {
            Ok(next) => current = next,
            Err(_) => break,
        }
        if days.contains(current.weekday()) {
            left -= 1;
        }
    }
    current
}

/// The working days after the date of `from`, up to and including the date of `to`, in `zone`.
pub fn working_days_since(
    from: Timestamp,
    to: Timestamp,
    zone: &EventTimeZone,
    days: Weekdays,
) -> u32 {
    let tz = tz_of(zone);
    let end = to.to_zoned(tz.clone()).date();
    let mut current = from.to_zoned(tz).date();
    let mut count = 0;
    while current < end {
        match current.tomorrow() {
            Ok(next) => current = next,
            Err(_) => break,
        }
        if days.contains(current.weekday()) {
            count += 1;
        }
    }
    count
}

/// The hours in which nothing is sent, in the local time of the recipient. `start == end` is an empty
/// window; `start > end` runs over midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    pub start: Time,
    pub end: Time,
}

impl QuietHours {
    /// `now` outside the quiet hours, or else the next `end` in `zone`.
    pub fn send_at(&self, now: Timestamp, zone: &EventTimeZone) -> Timestamp {
        let tz = tz_of(zone);
        let local = now.to_zoned(tz.clone());
        let time = local.time();
        let date = local.date();
        let end_date = if self.start < self.end {
            (self.start <= time && time < self.end).then_some(date)
        } else if self.start > self.end {
            if time < self.end {
                Some(date)
            } else if time >= self.start {
                date.tomorrow().ok()
            } else {
                None
            }
        } else {
            None
        };
        end_date
            .and_then(|d| instant(d, self.end, &tz))
            .filter(|slot| *slot > now)
            .unwrap_or(now)
    }
}

/// Whether a send at `last` is still inside the cooldown at `now`.
pub fn in_cooldown(last: Option<Timestamp>, now: Timestamp, hours: u8) -> bool {
    last.and_then(|at| {
        at.checked_add(SignedDuration::from_hours(i64::from(hours)))
            .ok()
    })
    .is_some_and(|until| now < until)
}

#[cfg(test)]
mod tests {
    use jiff::civil::time;

    use super::*;

    fn ts(s: &str) -> Timestamp {
        s.parse().expect("test timestamp")
    }

    fn zone(name: &str) -> EventTimeZone {
        EventTimeZone::parse(name).expect("test zone")
    }

    fn schedule(at: Time, name: &str) -> LocalSchedule {
        LocalSchedule {
            time: at,
            zone: zone(name),
            weekdays: Weekdays::MON_FRI,
        }
    }

    #[test]
    fn next_after_skips_weekend() {
        let s = schedule(time(8, 30, 0, 0), "Europe/Zurich");
        // Friday 2026-10-09 09:00 local (07:00Z) -> Monday 2026-10-12 08:30 local (06:30Z)
        assert_eq!(
            s.next_after(ts("2026-10-09T07:00:00Z")),
            ts("2026-10-12T06:30:00Z")
        );
        // Before the slot on the same day.
        assert_eq!(
            s.next_after(ts("2026-10-09T05:00:00Z")),
            ts("2026-10-09T06:30:00Z")
        );
        // Exactly at the slot: strictly after.
        assert_eq!(
            s.next_after(ts("2026-10-09T06:30:00Z")),
            ts("2026-10-12T06:30:00Z")
        );
    }

    #[test]
    fn next_after_dst_gap_moves_forward() {
        // 2027-03-28 02:30 does not exist in Zurich; it runs at 03:30 (01:30Z). Use all days.
        let s = LocalSchedule {
            time: time(2, 30, 0, 0),
            zone: zone("Europe/Zurich"),
            weekdays: Weekdays::from_bits(0x7F).unwrap(),
        };
        assert_eq!(
            s.next_after(ts("2027-03-27T23:00:00Z")),
            ts("2027-03-28T01:30:00Z")
        );
    }

    #[test]
    fn next_after_dst_fold_runs_once() {
        // 2026-10-25 02:30 happens twice in Zurich; the first one (00:30Z) counts.
        let s = LocalSchedule {
            time: time(2, 30, 0, 0),
            zone: zone("Europe/Zurich"),
            weekdays: Weekdays::from_bits(0x7F).unwrap(),
        };
        let first = s.next_after(ts("2026-10-24T23:00:00Z"));
        assert_eq!(first, ts("2026-10-25T00:30:00Z"));
        assert_eq!(s.next_after(first), ts("2026-10-26T01:30:00Z"));
    }

    #[test]
    fn due_caps_missed_runs_at_limit() {
        let s = schedule(time(8, 30, 0, 0), "Europe/Zurich");
        // Slots Mon, Tue, Wed are missed; now is Wednesday 12:00 local.
        let due = s.due(ts("2026-10-12T06:30:00Z"), ts("2026-10-14T10:00:00Z"), 1);
        assert_eq!(due.runs, 1);
        assert_eq!(due.next_run_at, ts("2026-10-15T06:30:00Z"));
        let none = s.due(ts("2026-10-15T06:30:00Z"), ts("2026-10-14T10:00:00Z"), 1);
        assert_eq!(none.runs, 0);
        assert_eq!(none.next_run_at, ts("2026-10-15T06:30:00Z"));
    }

    #[test]
    fn send_at_respects_recipient_zone_and_dst() {
        let quiet = QuietHours {
            start: time(20, 0, 0, 0),
            end: time(8, 0, 0, 0),
        };
        let ny = zone("America/New_York");
        // DST starts 2027-03-14 02:00 in New York. 08:00 on that day is EDT: 12:00Z.
        let end = ts("2027-03-14T12:00:00Z");
        // Saturday 22:00 EST (03:00Z), before midnight.
        assert_eq!(quiet.send_at(ts("2027-03-14T03:00:00Z"), &ny), end);
        // 01:00 EST (06:00Z), after midnight.
        assert_eq!(quiet.send_at(ts("2027-03-14T06:00:00Z"), &ny), end);
        // Outside: 10:00 EDT (14:00Z).
        let day = ts("2027-03-14T14:00:00Z");
        assert_eq!(quiet.send_at(day, &ny), day);
        // The same instant is 13:00 in Zurich, outside the window there.
        assert_eq!(
            quiet.send_at(ts("2027-03-14T12:00:00Z"), &zone("Europe/Zurich")),
            ts("2027-03-14T12:00:00Z")
        );
    }

    #[test]
    fn working_days_count_without_weekend() {
        let z = zone("Europe/Zurich");
        // Friday -> Tuesday is 2 working days.
        let n = working_days_since(
            ts("2026-10-09T10:00:00Z"),
            ts("2026-10-13T05:00:00Z"),
            &z,
            Weekdays::MON_FRI,
        );
        assert_eq!(n, 2);
        let date = jiff::civil::date(2026, 10, 9);
        assert_eq!(
            add_working_days(date, 2, Weekdays::MON_FRI),
            jiff::civil::date(2026, 10, 13)
        );
        assert_eq!(add_working_days(date, 0, Weekdays::MON_FRI), date);
    }

    #[test]
    fn weekdays_reject_empty_and_high_bits() {
        assert!(Weekdays::from_bits(0).is_err());
        assert!(Weekdays::from_bits(0x80).is_err());
        assert!(Weekdays::MON_FRI.contains(Weekday::Friday));
        assert!(!Weekdays::MON_FRI.contains(Weekday::Saturday));
    }

    #[test]
    fn cooldown_20_hours() {
        let last = ts("2026-10-09T06:30:00Z");
        assert!(in_cooldown(Some(last), ts("2026-10-10T02:29:00Z"), 20));
        assert!(!in_cooldown(Some(last), ts("2026-10-10T02:30:00Z"), 20));
        assert!(!in_cooldown(None, last, 20));
    }
}
