//! Milestones: a dated point of an event (Slice 2d).

use crate::work::InvalidTransition;

text_type!(
    /// The title of a milestone: 1 to 200 characters.
    MilestoneTitle,
    200
);

status_enum!(
    /// The status of a milestone.
    MilestoneStatus { Open => "open", Reached => "reached", Canceled => "canceled" }
);

impl MilestoneStatus {
    pub const ALL: [Self; 3] = [Self::Open, Self::Reached, Self::Canceled];

    /// The one decision of a status change of a milestone. `canceled` is final.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use MilestoneStatus::{Canceled, Open, Reached};
        let allowed = matches!((self, next), (Open, Reached | Canceled) | (Reached, Open));
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

    #[test]
    fn milestone_transitions() {
        let allowed = [
            (MilestoneStatus::Open, MilestoneStatus::Reached),
            (MilestoneStatus::Open, MilestoneStatus::Canceled),
            (MilestoneStatus::Reached, MilestoneStatus::Open),
        ];
        for from in MilestoneStatus::ALL {
            for to in MilestoneStatus::ALL {
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
        for status in MilestoneStatus::ALL {
            assert_eq!(status.change_to(status), Err(InvalidTransition));
        }
    }

    #[test]
    fn next_statuses_follow_the_transitions() {
        assert_eq!(
            MilestoneStatus::Open.next_statuses(),
            vec![MilestoneStatus::Reached, MilestoneStatus::Canceled]
        );
        assert!(MilestoneStatus::Canceled.next_statuses().is_empty());
    }

    #[test]
    fn statuses_round_trip() {
        for status in MilestoneStatus::ALL {
            assert_eq!(MilestoneStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn title_limit() {
        assert!(MilestoneTitle::parse(&"a".repeat(200)).is_ok());
        assert_eq!(
            MilestoneTitle::parse(&"a".repeat(201)),
            Err(TextError::TooLong)
        );
    }
}
