//! The readiness of the Bot API for `GET /readyz` of the `telegram` role (ADR 0076).

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tada_app::health::{DependencyCheck, DependencyUnavailable};

/// The role is not ready if no Bot API call succeeded for this long.
const FRESHNESS: Duration = Duration::from_secs(5 * 60);

/// The time of the last successful Bot API call of a gateway. Clones share it.
#[derive(Debug, Clone, Default)]
pub struct BotApiCheck {
    last_success: Arc<Mutex<Option<Instant>>>,
}

impl BotApiCheck {
    /// Records a successful Bot API call.
    pub(crate) fn succeeded(&self) {
        *self
            .last_success
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    /// True if a call succeeded in the `FRESHNESS` before `now`.
    fn fresh_at(&self, now: Instant) -> bool {
        self.last_success
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|last| now.saturating_duration_since(last) < FRESHNESS)
    }
}

#[derive(Debug)]
struct Stale;

impl fmt::Display for Stale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no Bot API call succeeded in the last {} seconds",
            FRESHNESS.as_secs()
        )
    }
}

impl std::error::Error for Stale {}

#[async_trait]
impl DependencyCheck for BotApiCheck {
    fn name(&self) -> &'static str {
        "bot-api"
    }

    async fn check(&self) -> Result<(), DependencyUnavailable> {
        if self.fresh_at(Instant::now()) {
            Ok(())
        } else {
            Err(DependencyUnavailable(Box::new(Stale)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_success_counts_for_five_minutes() {
        let check = BotApiCheck::default();
        let start = Instant::now();
        assert!(!check.fresh_at(start), "no call succeeded yet");
        check.succeeded();
        let shared = check.clone();
        assert!(shared.fresh_at(Instant::now()));
        assert!(shared.fresh_at(Instant::now() + FRESHNESS - Duration::from_secs(1)));
        assert!(!shared.fresh_at(Instant::now() + FRESHNESS + Duration::from_secs(1)));
    }
}
