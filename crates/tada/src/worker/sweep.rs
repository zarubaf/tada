//! The schedule of a periodic sweep of the worker.

use jiff::{SignedDuration, Timestamp};
use tada_app::rate_limit::RateWindow;

/// The shortest time between a failed sweep and its next attempt. A sweep reads the whole
/// bucket or table, so a failure that persists must not repeat on each tick of the worker.
const RETRY_INTERVAL: SignedDuration = SignedDuration::from_hours(1);

/// A sweep that runs once in each period. A period starts at a multiple of its length.
/// The worker runs a due sweep at its start, then again in each later period.
#[derive(Debug)]
pub struct Sweep {
    period: SignedDuration,
    /// The start of the period of the last successful sweep.
    swept: Option<Timestamp>,
    /// No attempt before this time, after a failure.
    retry_at: Option<Timestamp>,
}

impl Sweep {
    pub fn every(period: SignedDuration) -> Self {
        Self {
            period,
            swept: None,
            retry_at: None,
        }
    }

    /// A sweep once in 24 hours.
    pub fn daily() -> Self {
        Self::every(SignedDuration::from_hours(24))
    }

    /// The start of the period of `now`, if no sweep in this period succeeded
    /// and the retry interval after the last failure has passed.
    pub fn due(&self, now: Timestamp) -> Option<Timestamp> {
        if self.retry_at.is_some_and(|retry_at| now < retry_at) {
            return None;
        }
        let start = RateWindow::containing(now, self.period).start;
        (self.swept != Some(start)).then_some(start)
    }

    /// Records a successful sweep in the period that starts at `period`.
    pub fn done(&mut self, period: Timestamp) {
        self.swept = Some(period);
        self.retry_at = None;
    }

    /// Records a failed sweep at `now`. The next attempt waits for the retry interval,
    /// or for the period if that is shorter.
    pub fn failed(&mut self, now: Timestamp) {
        let wait = self.period.min(RETRY_INTERVAL);
        self.retry_at = Some(now.saturating_add(wait).unwrap_or(Timestamp::MAX));
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
        // A sweep that nobody reports as failed stays due.
        assert_eq!(sweep.due(at("2030-05-18T08:30:02Z")), Some(eight));
        sweep.done(eight);
        assert_eq!(sweep.due(at("2030-05-18T08:59:59Z")), None);
        assert_eq!(
            sweep.due(at("2030-05-18T09:00:01Z")),
            Some(at("2030-05-18T09:00:00Z"))
        );
    }

    #[test]
    fn a_daily_sweep_runs_again_on_the_next_day() {
        let mut sweep = Sweep::daily();
        let day = at("2030-05-18T00:00:00Z");
        assert_eq!(sweep.due(at("2030-05-18T23:00:00Z")), Some(day));
        sweep.done(day);
        assert_eq!(sweep.due(at("2030-05-18T23:59:59Z")), None);
        assert_eq!(
            sweep.due(at("2030-05-19T00:00:00Z")),
            Some(at("2030-05-19T00:00:00Z"))
        );
    }

    #[test]
    fn a_failure_is_not_retried_before_the_retry_interval() {
        let mut sweep = Sweep::daily();
        let day = at("2030-05-18T00:00:00Z");
        sweep.failed(at("2030-05-18T08:00:00Z"));
        assert_eq!(sweep.due(at("2030-05-18T08:00:02Z")), None);
        assert_eq!(sweep.due(at("2030-05-18T08:59:59Z")), None);
        assert_eq!(sweep.due(at("2030-05-18T09:00:00Z")), Some(day));
        // A second failure waits again.
        sweep.failed(at("2030-05-18T09:00:01Z"));
        assert_eq!(sweep.due(at("2030-05-18T09:30:00Z")), None);
    }

    #[test]
    fn a_success_waits_for_the_full_period() {
        let mut sweep = Sweep::daily();
        let day = at("2030-05-18T00:00:00Z");
        sweep.failed(at("2030-05-18T08:00:00Z"));
        sweep.done(day);
        assert_eq!(sweep.due(at("2030-05-18T09:00:00Z")), None);
        assert_eq!(sweep.due(at("2030-05-18T23:59:59Z")), None);
        assert!(sweep.due(at("2030-05-19T00:00:00Z")).is_some());
    }

    #[test]
    fn a_short_period_retries_after_the_period() {
        let mut sweep = Sweep::every(SignedDuration::from_mins(10));
        sweep.failed(at("2030-05-18T08:00:00Z"));
        assert_eq!(sweep.due(at("2030-05-18T08:09:59Z")), None);
        assert!(sweep.due(at("2030-05-18T08:10:00Z")).is_some());
    }
}
