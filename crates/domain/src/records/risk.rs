//! Risks: what can go wrong, rated by likelihood and impact (Slice 2d).

use std::cmp::Reverse;

use crate::work::InvalidTransition;

text_type!(
    /// The title of a risk: 1 to 200 characters.
    RiskTitle,
    200
);

status_enum!(
    /// A level of likelihood or impact.
    Level { Low => "low", Medium => "medium", High => "high" }
);

impl Level {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// The weight in a score: 1, 2 or 3.
    pub const fn weight(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }
}

/// The rating of a risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskRating {
    pub likelihood: Level,
    pub impact: Level,
}

impl RiskRating {
    /// Likelihood weight times impact weight: 1 to 9.
    pub fn score(self) -> u8 {
        self.likelihood.weight() * self.impact.weight()
    }

    /// Sorts the worst risk first: the higher score, then the higher impact.
    pub fn sort_key(self) -> (Reverse<u8>, Reverse<u8>) {
        (Reverse(self.score()), Reverse(self.impact.weight()))
    }
}

status_enum!(
    /// The status of a risk.
    RiskStatus {
        Open => "open",
        Mitigated => "mitigated",
        Occurred => "occurred",
        Closed => "closed",
    }
);

impl RiskStatus {
    pub const ALL: [Self; 4] = [Self::Open, Self::Mitigated, Self::Occurred, Self::Closed];

    /// The one decision of a status change of a risk.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use RiskStatus::{Closed, Mitigated, Occurred, Open};
        let allowed = matches!(
            (self, next),
            (Open, Mitigated)
                | (Mitigated, Open)
                | (Open | Mitigated, Occurred)
                | (Open | Mitigated | Occurred, Closed)
                | (Closed, Open)
        );
        if allowed {
            Ok(next)
        } else {
            Err(InvalidTransition)
        }
    }

    /// The statuses that a record in this status can change to.
    pub fn next_statuses(self) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|next| self.change_to(*next).is_ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::TextError;
    use Level::{High, Low, Medium};

    fn rating(likelihood: Level, impact: Level) -> RiskRating {
        RiskRating { likelihood, impact }
    }

    #[test]
    fn risk_transitions() {
        let allowed = [
            (RiskStatus::Open, RiskStatus::Mitigated),
            (RiskStatus::Mitigated, RiskStatus::Open),
            (RiskStatus::Open, RiskStatus::Occurred),
            (RiskStatus::Mitigated, RiskStatus::Occurred),
            (RiskStatus::Open, RiskStatus::Closed),
            (RiskStatus::Mitigated, RiskStatus::Closed),
            (RiskStatus::Occurred, RiskStatus::Closed),
            (RiskStatus::Closed, RiskStatus::Open),
        ];
        for from in RiskStatus::ALL {
            for to in RiskStatus::ALL {
                assert_eq!(
                    from.change_to(to).is_ok(),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn same_status_is_invalid() {
        for status in RiskStatus::ALL {
            assert_eq!(status.change_to(status), Err(InvalidTransition));
        }
    }

    #[test]
    fn a_closed_risk_can_open_again() {
        assert!(RiskStatus::Closed.change_to(RiskStatus::Open).is_ok());
    }

    #[test]
    fn strings_round_trip() {
        for status in RiskStatus::ALL {
            assert_eq!(RiskStatus::parse(status.as_str()), Some(status));
        }
        for level in Level::ALL {
            assert_eq!(Level::parse(level.as_str()), Some(level));
        }
    }

    #[test]
    fn weights_and_scores() {
        assert_eq!([Low.weight(), Medium.weight(), High.weight()], [1, 2, 3]);
        assert_eq!(rating(Low, Low).score(), 1);
        assert_eq!(rating(High, High).score(), 9);
        assert_eq!(rating(Medium, Medium).score(), 4);
    }

    #[test]
    fn risk_sort_key_orders_by_score_then_impact() {
        assert!(rating(Medium, Medium).sort_key() < rating(High, Low).sort_key());
        assert_eq!(rating(Medium, High).score(), rating(High, Medium).score());
        assert!(rating(Medium, High).sort_key() < rating(High, Medium).sort_key());
    }

    #[test]
    fn title_limit() {
        assert!(RiskTitle::parse(&"a".repeat(200)).is_ok());
        assert_eq!(RiskTitle::parse(&"a".repeat(201)), Err(TextError::TooLong));
    }
}
