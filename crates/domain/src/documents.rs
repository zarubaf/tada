//! Documents (ADR 0051): the name of a document and the Markdown text of a draft.

use crate::facts::{TextError, checked_text};

/// The name of a document: 1 to 200 characters, without control characters and without spaces at the ends.
#[derive(Clone, PartialEq, Eq)]
pub struct DocumentName(String);

impl DocumentName {
    pub const MAX_CHARS: usize = 200;

    /// Removes the spaces at the ends, then checks the text.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        Ok(Self(checked_text(input, Self::MAX_CHARS)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name can contain personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for DocumentName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DocumentName({} characters)", self.0.chars().count())
    }
}

/// The Markdown text of a draft (ADR 0051, ADR 0058), with LF line ends.
/// It has at least one character that is not a space, and at most `MAX_CHARS` characters.
/// It contains no control characters other than line ends and tabs.
#[derive(Clone, PartialEq, Eq)]
pub struct DraftMarkdown(String);

impl DraftMarkdown {
    /// A concept of an event has some ten thousand characters. The limit leaves room and bounds the size of a proposal.
    pub const MAX_CHARS: usize = 200_000;

    /// Changes CRLF and CR line ends to LF, then checks the text. It keeps all other characters as they are.
    pub fn parse(input: &str) -> Result<Self, TextError> {
        let text = input.replace("\r\n", "\n").replace('\r', "\n");
        if text.trim().is_empty() {
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
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A draft can contain personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for DraftMarkdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DraftMarkdown({} characters)", self.0.chars().count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_document_names() {
        assert_eq!(
            DocumentName::parse(" Konzept Open Day ").unwrap().as_str(),
            "Konzept Open Day"
        );
        assert_eq!(DocumentName::parse(" "), Err(TextError::Empty));
        assert_eq!(
            DocumentName::parse(&"a".repeat(DocumentName::MAX_CHARS + 1)),
            Err(TextError::TooLong)
        );
    }

    #[test]
    fn a_draft_has_lf_line_ends_and_keeps_tabs() {
        let draft = DraftMarkdown::parse("# Konzept\r\n\r\n- Punkt\r\tEinzug\n").unwrap();
        assert_eq!(draft.as_str(), "# Konzept\n\n- Punkt\n\tEinzug\n");
    }

    #[test]
    fn rejects_an_empty_draft_a_long_draft_and_control_characters() {
        assert_eq!(DraftMarkdown::parse(" \n\t"), Err(TextError::Empty));
        assert_eq!(
            DraftMarkdown::parse(&"a".repeat(DraftMarkdown::MAX_CHARS + 1)),
            Err(TextError::TooLong)
        );
        assert_eq!(
            DraftMarkdown::parse("Text\u{0}"),
            Err(TextError::ControlCharacter)
        );
    }

    #[test]
    fn debug_hides_names_and_drafts() {
        let name = DocumentName::parse("Anna Muster").unwrap();
        assert!(!format!("{name:?}").contains("Anna"));
        let draft = DraftMarkdown::parse("Anna Muster kommt.").unwrap();
        assert!(!format!("{draft:?}").contains("Anna"));
    }
}
