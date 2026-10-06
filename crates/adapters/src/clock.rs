//! The clock of the operating system (ADR 0038).

use jiff::Timestamp;
use tada_app::clock::Clock;

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let microseconds = Timestamp::now().as_microsecond();
        // A value from `as_microsecond` is always in range.
        Timestamp::from_microsecond(microseconds).unwrap_or(Timestamp::UNIX_EPOCH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_microsecond_precision() {
        assert_eq!(SystemClock.now().subsec_nanosecond() % 1000, 0);
    }
}
