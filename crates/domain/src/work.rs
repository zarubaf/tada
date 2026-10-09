//! Workstreams, actions and commitments, with their states (Slice 2a).

use crate::facts::{TextError, checked_text};

/// Defines a text newtype of 1 to `MAX_CHARS` characters, without control characters.
/// `Debug` shows the length only (ADR 0035).
macro_rules! text_type {
    ($(#[$doc:meta])* $name:ident, $max:expr) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub const MAX_CHARS: usize = $max;

            /// Removes the spaces at the ends, then checks the text.
            pub fn parse(input: &str) -> Result<Self, TextError> {
                Ok(Self(checked_text(input, Self::MAX_CHARS)?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({} characters)", stringify!($name), self.0.chars().count())
            }
        }
    };
}

text_type!(
    /// The name of a workstream: 1 to 200 characters.
    WorkstreamName,
    200
);
text_type!(
    /// The title of an action: 1 to 200 characters.
    ActionTitle,
    200
);
text_type!(
    /// The text of a commitment: 1 to 500 characters.
    CommitmentText,
    500
);
text_type!(
    /// The condition of a commitment: 1 to 500 characters.
    ConditionText,
    500
);
text_type!(
    /// The reason to make a commitment firm: 1 to 500 characters.
    FirmReason,
    500
);

/// The description of an action: 1 to 4000 characters.
/// Unlike the other texts, it can have line breaks and tabs.
#[derive(Clone, PartialEq, Eq)]
pub struct ActionDescription(String);

impl ActionDescription {
    pub const MAX_CHARS: usize = 4000;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(TextError::Empty);
        }
        if text.chars().count() > Self::MAX_CHARS {
            return Err(TextError::TooLong);
        }
        if text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(TextError::ControlCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ActionDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ActionDescription({} characters)",
            self.0.chars().count()
        )
    }
}

/// Defines a status enum with kebab-case `as_str` and `parse`.
macro_rules! status_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn parse(input: &str) -> Option<Self> {
                match input {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

status_enum!(
    /// The status of a workstream.
    WorkstreamStatus { Active => "active", Closed => "closed" }
);
status_enum!(
    /// The status of an action.
    ActionStatus {
        Open => "open",
        InProgress => "in-progress",
        Blocked => "blocked",
        Done => "done",
        Canceled => "canceled",
    }
);
status_enum!(
    /// The status of a commitment.
    CommitmentStatus {
        Conditional => "conditional",
        Firm => "firm",
        Fulfilled => "fulfilled",
        Broken => "broken",
        Withdrawn => "withdrawn",
    }
);

impl ActionStatus {
    /// Returns true if an action in this status can change to `next`.
    /// A change to the same status is not a transition.
    pub fn can_change_to(self, next: Self) -> bool {
        use ActionStatus::{Blocked, Canceled, Done, InProgress, Open};
        matches!(
            (self, next),
            (Open, InProgress)
                | (InProgress, Open)
                | (Open | InProgress, Blocked)
                | (Blocked, Open | InProgress)
                | (Open | InProgress | Blocked, Done | Canceled)
                | (Done, Open)
        )
    }
}

impl CommitmentStatus {
    /// The status of a new commitment: conditional with a condition, else firm.
    pub fn initial(condition: Option<&ConditionText>) -> Self {
        if condition.is_some() {
            Self::Conditional
        } else {
            Self::Firm
        }
    }

    /// Returns true if a commitment in this status can change to `next`.
    pub fn can_change_to(self, next: Self) -> bool {
        use CommitmentStatus::{Broken, Conditional, Firm, Fulfilled, Withdrawn};
        matches!(
            (self, next),
            (Conditional, Firm) | (Conditional | Firm, Fulfilled | Broken | Withdrawn)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION: [ActionStatus; 5] = [
        ActionStatus::Open,
        ActionStatus::InProgress,
        ActionStatus::Blocked,
        ActionStatus::Done,
        ActionStatus::Canceled,
    ];
    const COMMITMENT: [CommitmentStatus; 5] = [
        CommitmentStatus::Conditional,
        CommitmentStatus::Firm,
        CommitmentStatus::Fulfilled,
        CommitmentStatus::Broken,
        CommitmentStatus::Withdrawn,
    ];

    #[test]
    fn an_action_can_be_reopened_after_done() {
        assert!(ActionStatus::Done.can_change_to(ActionStatus::Open));
        assert!(!ActionStatus::Done.can_change_to(ActionStatus::InProgress));
        assert!(!ActionStatus::Done.can_change_to(ActionStatus::Canceled));
    }

    #[test]
    fn a_canceled_action_is_final() {
        assert!(
            ACTION
                .iter()
                .all(|next| !ActionStatus::Canceled.can_change_to(*next))
        );
    }

    #[test]
    fn a_blocked_action_returns_to_open_or_in_progress() {
        assert!(ActionStatus::Blocked.can_change_to(ActionStatus::Open));
        assert!(ActionStatus::Blocked.can_change_to(ActionStatus::InProgress));
        assert!(ActionStatus::Open.can_change_to(ActionStatus::Blocked));
        assert!(ActionStatus::InProgress.can_change_to(ActionStatus::Blocked));
    }

    #[test]
    fn an_action_never_changes_to_its_own_status() {
        assert!(ACTION.iter().all(|s| !s.can_change_to(*s)));
    }

    #[test]
    fn a_commitment_with_a_condition_starts_conditional() {
        let condition = ConditionText::parse("if the permit arrives").unwrap();
        assert_eq!(
            CommitmentStatus::initial(Some(&condition)),
            CommitmentStatus::Conditional
        );
    }

    #[test]
    fn a_commitment_without_a_condition_starts_firm() {
        assert_eq!(CommitmentStatus::initial(None), CommitmentStatus::Firm);
    }

    #[test]
    fn a_firm_commitment_never_returns_to_conditional() {
        assert!(
            COMMITMENT
                .iter()
                .all(|s| !s.can_change_to(CommitmentStatus::Conditional))
        );
        assert!(CommitmentStatus::Conditional.can_change_to(CommitmentStatus::Firm));
        assert!(!CommitmentStatus::Firm.can_change_to(CommitmentStatus::Firm));
    }

    #[test]
    fn fulfilled_broken_and_withdrawn_are_final() {
        for done in [
            CommitmentStatus::Fulfilled,
            CommitmentStatus::Broken,
            CommitmentStatus::Withdrawn,
        ] {
            assert!(COMMITMENT.iter().all(|next| !done.can_change_to(*next)));
        }
        for open in [CommitmentStatus::Conditional, CommitmentStatus::Firm] {
            assert!(open.can_change_to(CommitmentStatus::Fulfilled));
            assert!(open.can_change_to(CommitmentStatus::Broken));
            assert!(open.can_change_to(CommitmentStatus::Withdrawn));
        }
    }

    #[test]
    fn statuses_round_trip_in_kebab_case() {
        assert_eq!(ActionStatus::InProgress.as_str(), "in-progress");
        for s in ACTION {
            assert_eq!(ActionStatus::parse(s.as_str()), Some(s));
        }
        for s in COMMITMENT {
            assert_eq!(CommitmentStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(ActionStatus::parse("in_progress"), None);
        assert_eq!(
            WorkstreamStatus::parse("closed"),
            Some(WorkstreamStatus::Closed)
        );
    }

    #[test]
    fn text_types_refuse_text_over_the_limit() {
        assert!(WorkstreamName::parse(&"a".repeat(200)).is_ok());
        assert_eq!(
            WorkstreamName::parse(&"a".repeat(201)),
            Err(TextError::TooLong)
        );
        assert_eq!(
            ActionTitle::parse(&"a".repeat(201)),
            Err(TextError::TooLong)
        );
        assert_eq!(
            CommitmentText::parse(&"a".repeat(501)),
            Err(TextError::TooLong)
        );
        assert_eq!(
            ConditionText::parse(&"a".repeat(501)),
            Err(TextError::TooLong)
        );
        assert_eq!(FirmReason::parse(&"a".repeat(501)), Err(TextError::TooLong));
        assert!(ActionDescription::parse(&"a".repeat(4000)).is_ok());
        assert_eq!(
            ActionDescription::parse(&"a".repeat(4001)),
            Err(TextError::TooLong)
        );
    }

    #[test]
    fn a_description_keeps_line_breaks_but_a_title_does_not() {
        assert!(ActionDescription::parse("one\ntwo").is_ok());
        assert_eq!(
            ActionTitle::parse("one\ntwo"),
            Err(TextError::ControlCharacter)
        );
        assert_eq!(
            ActionDescription::parse("a\u{7}"),
            Err(TextError::ControlCharacter)
        );
    }

    #[test]
    fn debug_shows_the_length_only() {
        assert_eq!(
            format!("{:?}", ActionTitle::parse("secret").unwrap()),
            "ActionTitle(6 characters)"
        );
    }
}
