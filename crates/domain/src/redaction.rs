//! Legal redaction (ADR 0045): what a redaction can target, why it runs and how a tombstone reads.

use std::str::FromStr;

use uuid::Uuid;

/// The kind of record that a redaction targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RedactionTargetKind {
    SourceItem,
    FactVersion,
    DocumentVersion,
    Proposal,
    Person,
    Institution,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a kind of redaction target")]
pub struct UnknownRedactionTargetKind;

impl RedactionTargetKind {
    pub const ALL: [Self; 7] = [
        Self::SourceItem,
        Self::FactVersion,
        Self::DocumentVersion,
        Self::Proposal,
        Self::Person,
        Self::Institution,
        Self::Event,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceItem => "source-item",
            Self::FactVersion => "fact-version",
            Self::DocumentVersion => "document-version",
            Self::Proposal => "proposal",
            Self::Person => "person",
            Self::Institution => "institution",
            Self::Event => "event",
        }
    }
}

impl FromStr for RedactionTargetKind {
    type Err = UnknownRedactionTargetKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or(UnknownRedactionTargetKind)
    }
}

/// One record that a redaction targets. The order (kind, then ID) is the lock order of a redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RedactionTarget {
    pub kind: RedactionTargetKind,
    pub id: Uuid,
}

/// Why a redaction runs: a request of a person, or the end of a retention period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionReason {
    Request,
    Retention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a reason of redaction")]
pub struct UnknownRedactionReason;

impl RedactionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Retention => "retention",
        }
    }
}

impl FromStr for RedactionReason {
    type Err = UnknownRedactionReason;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "request" => Ok(Self::Request),
            "retention" => Ok(Self::Retention),
            _ => Err(UnknownRedactionReason),
        }
    }
}

/// Content that a redaction can remove. `Redacted` is the tombstone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redactable<T> {
    Present(T),
    Redacted,
}

impl<T> Redactable<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Redactable<U> {
        match self {
            Self::Present(value) => Redactable::Present(f(value)),
            Self::Redacted => Redactable::Redacted,
        }
    }

    pub fn as_present(&self) -> Option<&T> {
        match self {
            Self::Present(value) => Some(value),
            Self::Redacted => None,
        }
    }
}

/// The text of a dispute note: one to 1000 characters after the spaces at the ends are removed.
/// The text can name a person, so `Debug` shows its length only (ADR 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct DisputeNoteText(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DisputeNoteError {
    #[error("a dispute note has text")]
    Empty,
    #[error("a dispute note has at most 1000 characters")]
    TooLong,
}

impl DisputeNoteText {
    pub const MAX_CHARACTERS: usize = 1000;

    pub fn parse(value: &str) -> Result<Self, DisputeNoteError> {
        let text = value.trim();
        if text.is_empty() {
            return Err(DisputeNoteError::Empty);
        }
        if text.chars().count() > Self::MAX_CHARACTERS {
            return Err(DisputeNoteError::TooLong);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for DisputeNoteText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DisputeNoteText({} characters)", self.0.chars().count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispute_note_text_rejects_empty_and_1001() {
        assert_eq!(DisputeNoteText::parse(" \n"), Err(DisputeNoteError::Empty));
        assert_eq!(
            DisputeNoteText::parse(&"ä".repeat(1001)),
            Err(DisputeNoteError::TooLong)
        );
        assert_eq!(
            DisputeNoteText::parse(&"ä".repeat(1000))
                .unwrap()
                .as_str()
                .chars()
                .count(),
            1000
        );
        assert_eq!(
            DisputeNoteText::parse("  Falsch  ").unwrap().as_str(),
            "Falsch"
        );
    }

    #[test]
    fn redaction_target_kind_round_trips() {
        for kind in RedactionTargetKind::ALL {
            assert_eq!(kind.as_str().parse::<RedactionTargetKind>(), Ok(kind));
        }
        assert_eq!(RedactionTargetKind::SourceItem.as_str(), "source-item");
        assert_eq!(
            "other".parse::<RedactionTargetKind>(),
            Err(UnknownRedactionTargetKind)
        );
    }

    #[test]
    fn redaction_reason_round_trips() {
        for reason in [RedactionReason::Request, RedactionReason::Retention] {
            assert_eq!(reason.as_str().parse::<RedactionReason>(), Ok(reason));
        }
    }

    #[test]
    fn redactable_maps_and_exposes_present_content() {
        let present = Redactable::Present(2).map(|n| n * 2);
        assert_eq!(present.as_present(), Some(&4));
        assert_eq!(
            Redactable::<i32>::Redacted.map(|n| n * 2).as_present(),
            None
        );
    }
}
