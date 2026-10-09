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

/// A status change that the transitions of ADR 0068 do not allow.
/// A change to the same status is not a transition either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the status cannot change to this status")]
pub struct InvalidTransition;

impl ActionStatus {
    pub const ALL: [Self; 5] = [
        Self::Open,
        Self::InProgress,
        Self::Blocked,
        Self::Done,
        Self::Canceled,
    ];

    /// The one decision of a status change of an action (ADR 0068): the direct command, the check of a
    /// proposal and the apply all ask it.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use ActionStatus::{Blocked, Canceled, Done, InProgress, Open};
        let allowed = matches!(
            (self, next),
            (Open, InProgress)
                | (InProgress, Open)
                | (Open | InProgress, Blocked)
                | (Blocked, Open | InProgress)
                | (Open | InProgress | Blocked, Done | Canceled)
                | (Done, Open)
        );
        if allowed {
            Ok(next)
        } else {
            Err(InvalidTransition)
        }
    }

    /// The statuses that an action in this status can change to.
    pub fn next_statuses(self) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|next| self.change_to(*next).is_ok())
            .collect()
    }
}

impl CommitmentStatus {
    pub const ALL: [Self; 5] = [
        Self::Conditional,
        Self::Firm,
        Self::Fulfilled,
        Self::Broken,
        Self::Withdrawn,
    ];

    /// The status of a new commitment: conditional with a condition, else firm.
    pub fn initial(condition: Option<&ConditionText>) -> Self {
        if condition.is_some() {
            Self::Conditional
        } else {
            Self::Firm
        }
    }

    /// The one decision of a status change of a commitment (ADR 0068). A change to `firm` needs a reason,
    /// which "make firm" and a proposal give; `change_directly` refuses it.
    pub fn change_to(self, next: Self) -> Result<Self, InvalidTransition> {
        use CommitmentStatus::{Broken, Conditional, Firm, Fulfilled, Withdrawn};
        let allowed = matches!(
            (self, next),
            (Conditional, Firm) | (Conditional | Firm, Fulfilled | Broken | Withdrawn)
        );
        if allowed {
            Ok(next)
        } else {
            Err(InvalidTransition)
        }
    }

    /// A status change of the direct change command: any transition but to `firm`.
    pub fn change_directly(self, next: Self) -> Result<Self, InvalidTransition> {
        if next == Self::Firm {
            return Err(InvalidTransition);
        }
        self.change_to(next)
    }

    /// The statuses that the direct change command can set on a commitment in this status.
    pub fn direct_next_statuses(self) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|next| self.change_directly(*next).is_ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION: [ActionStatus; 5] = ActionStatus::ALL;
    const COMMITMENT: [CommitmentStatus; 5] = CommitmentStatus::ALL;

    #[test]
    fn an_action_can_be_reopened_after_done() {
        assert!(ActionStatus::Done.change_to(ActionStatus::Open).is_ok());
        assert!(
            !ActionStatus::Done
                .change_to(ActionStatus::InProgress)
                .is_ok()
        );
        assert!(!ActionStatus::Done.change_to(ActionStatus::Canceled).is_ok());
    }

    #[test]
    fn a_canceled_action_is_final() {
        assert!(
            ACTION
                .iter()
                .all(|next| !ActionStatus::Canceled.change_to(*next).is_ok())
        );
    }

    #[test]
    fn a_blocked_action_returns_to_open_or_in_progress() {
        assert!(ActionStatus::Blocked.change_to(ActionStatus::Open).is_ok());
        assert!(
            ActionStatus::Blocked
                .change_to(ActionStatus::InProgress)
                .is_ok()
        );
        assert!(ActionStatus::Open.change_to(ActionStatus::Blocked).is_ok());
        assert!(
            ActionStatus::InProgress
                .change_to(ActionStatus::Blocked)
                .is_ok()
        );
    }

    #[test]
    fn an_action_never_changes_to_its_own_status() {
        assert!(ACTION.iter().all(|s| !s.change_to(*s).is_ok()));
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
                .all(|s| !s.change_to(CommitmentStatus::Conditional).is_ok())
        );
        assert!(
            CommitmentStatus::Conditional
                .change_to(CommitmentStatus::Firm)
                .is_ok()
        );
        assert!(
            !CommitmentStatus::Firm
                .change_to(CommitmentStatus::Firm)
                .is_ok()
        );
    }

    #[test]
    fn fulfilled_broken_and_withdrawn_are_final() {
        for done in [
            CommitmentStatus::Fulfilled,
            CommitmentStatus::Broken,
            CommitmentStatus::Withdrawn,
        ] {
            assert!(COMMITMENT.iter().all(|next| !done.change_to(*next).is_ok()));
        }
        for open in [CommitmentStatus::Conditional, CommitmentStatus::Firm] {
            assert!(open.change_to(CommitmentStatus::Fulfilled).is_ok());
            assert!(open.change_to(CommitmentStatus::Broken).is_ok());
            assert!(open.change_to(CommitmentStatus::Withdrawn).is_ok());
        }
    }

    #[test]
    fn a_direct_change_never_makes_a_commitment_firm() {
        assert_eq!(
            CommitmentStatus::Conditional.change_directly(CommitmentStatus::Firm),
            Err(InvalidTransition)
        );
        assert_eq!(
            CommitmentStatus::Conditional.change_directly(CommitmentStatus::Fulfilled),
            Ok(CommitmentStatus::Fulfilled)
        );
        assert_eq!(
            CommitmentStatus::Conditional.direct_next_statuses(),
            [
                CommitmentStatus::Fulfilled,
                CommitmentStatus::Broken,
                CommitmentStatus::Withdrawn
            ]
        );
        assert!(
            CommitmentStatus::Fulfilled
                .direct_next_statuses()
                .is_empty()
        );
    }

    #[test]
    fn next_statuses_follow_the_transitions() {
        assert_eq!(
            ActionStatus::Open.next_statuses(),
            [
                ActionStatus::InProgress,
                ActionStatus::Blocked,
                ActionStatus::Done,
                ActionStatus::Canceled
            ]
        );
        assert_eq!(ActionStatus::Done.next_statuses(), [ActionStatus::Open]);
        assert!(ActionStatus::Canceled.next_statuses().is_empty());
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
