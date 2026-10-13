//! Requirements: what the event must meet (Slice 2d).

use crate::work::InvalidTransition;

text_type!(
    /// The text of a requirement: 1 to 500 characters.
    RequirementText,
    500
);

status_enum!(
    /// The status of a requirement.
    RequirementStatus: transitions { Open => "open", Met => "met", Waived => "waived" }
);

impl RequirementStatus {
    /// The one decision of a status change of a requirement. `waived` is final and needs a reason,
    /// which the caller checks.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use RequirementStatus::{Met, Open, Waived};
        let allowed = matches!((self, next), (Open, Met | Waived) | (Met, Open));
        if allowed {
            Ok(next)
        } else {
            Err(InvalidTransition)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::TextError;

    #[test]
    fn requirement_waived_is_final() {
        let allowed = [
            (RequirementStatus::Open, RequirementStatus::Met),
            (RequirementStatus::Met, RequirementStatus::Open),
            (RequirementStatus::Open, RequirementStatus::Waived),
        ];
        for from in RequirementStatus::ALL {
            for to in RequirementStatus::ALL {
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
        for status in RequirementStatus::ALL {
            assert_eq!(status.change_to(status), Err(InvalidTransition));
        }
    }

    #[test]
    fn strings_round_trip() {
        for status in RequirementStatus::ALL {
            assert_eq!(RequirementStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn text_limit() {
        assert!(RequirementText::parse(&"a".repeat(500)).is_ok());
        assert_eq!(
            RequirementText::parse(&"a".repeat(501)),
            Err(TextError::TooLong)
        );
    }
}
