//! Milestones: a dated point of an event (Slice 2d).

use jiff::civil::Date;

use crate::work::InvalidTransition;

text_type!(
    /// The title of a milestone: 1 to 200 characters.
    MilestoneTitle,
    200
);

status_enum!(
    /// The status of a milestone.
    MilestoneStatus: transitions { Open => "open", Reached => "reached", Canceled => "canceled" }
);

impl MilestoneStatus {
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

    /// True if a milestone with this status and `date` is overdue on `today` (ADR 0080):
    /// it is still open, and its date is before today in the time zone of the event.
    pub fn is_overdue(self, date: Date, today: Date) -> bool {
        self == Self::Open && date < today
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
    fn only_an_open_milestone_before_today_is_overdue() {
        let today = jiff::civil::date(2030, 5, 18);
        let yesterday = jiff::civil::date(2030, 5, 17);
        assert!(MilestoneStatus::Open.is_overdue(yesterday, today));
        assert!(!MilestoneStatus::Open.is_overdue(today, today));
        assert!(!MilestoneStatus::Reached.is_overdue(yesterday, today));
        assert!(!MilestoneStatus::Canceled.is_overdue(yesterday, today));
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
