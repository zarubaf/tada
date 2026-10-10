//! The schedule of a periodic sweep of the worker.

use jiff::{SignedDuration, Timestamp};
use tada_app::rate_limit::RateWindow;

/// A sweep that runs once in each period. A period starts at a multiple of its length.
/// The worker runs a due sweep at its start, then again in each later period.
#[derive(Debug)]
pub struct Sweep {
    period: SignedDuration,
    /// The start of the period of the last successful sweep.
    swept: Option<Timestamp>,
}

impl Sweep {
    pub fn every(period: SignedDuration) -> Self {
        Self {
            period,
            swept: None,
        }
    }

    /// The start of the period of `now`, if no sweep in this period succeeded.
    pub fn due(&self, now: Timestamp) -> Option<Timestamp> {
        let start = RateWindow::containing(now, self.period).start;
        (self.swept != Some(start)).then_some(start)
    }

    /// Records a successful sweep in the period that starts at `period`.
    pub fn done(&mut self, period: Timestamp) {
        self.swept = Some(period);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(time: &str) -> Timestamp {
        time.parse().unwrap()
    }

    #[test]
    fn the_sweep_is_due_once_per_period_and_again_after_a_failure() {
        let mut sweep = Sweep::every(SignedDuration::from_hours(1));
        let eight = at("2030-05-18T08:00:00Z");
        assert_eq!(sweep.due(at("2030-05-18T08:30:00Z")), Some(eight));
        // A failed sweep records nothing, so the next loop tries again.
        assert_eq!(sweep.due(at("2030-05-18T08:30:02Z")), Some(eight));
        sweep.done(eight);
        assert_eq!(sweep.due(at("2030-05-18T08:59:59Z")), None);
        assert_eq!(
            sweep.due(at("2030-05-18T09:00:01Z")),
            Some(at("2030-05-18T09:00:00Z"))
        );
    }
}
