//! Source texts and passages (ADR 0050): the exact locations that evidence points to.

use unicode_normalization::UnicodeNormalization;

/// The normalized text of a source version: Unicode NFC with `\n` line ends.
/// The character offsets of a passage count in this text.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceText(String);

impl SourceText {
    /// Converts `\r\n` and `\r` to `\n`, then applies Unicode NFC.
    pub fn normalize(input: &str) -> Self {
        let lines = input.replace("\r\n", "\n").replace('\r', "\n");
        Self(lines.nfc().collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The text can contain personal data, so `Debug` shows its length only (ADR 0035).
impl std::fmt::Debug for SourceText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SourceText({} characters)", self.0.chars().count())
    }
}

/// A passage of a source version: a range of characters, its exact quote, and the page of a PDF.
#[derive(Clone, PartialEq, Eq)]
pub struct Passage {
    /// The offset of the first character, in characters of the normalized text.
    pub start: u32,
    /// The offset after the last character.
    pub end: u32,
    pub quote: String,
    /// The page of a PDF, from 1.
    pub page: Option<u32>,
}

/// The quote can contain personal data, so `Debug` shows the range only (ADR 0035).
impl std::fmt::Debug for Passage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Passage")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("page", &self.page)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PassageError {
    #[error("a passage has at least one character")]
    Empty,
    #[error("the passage ends after the end of the text")]
    OutOfRange,
    #[error("the quote is not the text of the range")]
    QuoteMismatch,
    #[error("the page of a passage starts at 1")]
    Page,
}

impl Passage {
    /// Returns `Ok` if `quote` is the text from `start` to `end` of `text`, the normalized text of the source version.
    pub fn check(&self, text: &str) -> Result<(), PassageError> {
        if self.page == Some(0) {
            return Err(PassageError::Page);
        }
        if self.start >= self.end {
            return Err(PassageError::Empty);
        }
        let start = byte_offset(text, self.start).ok_or(PassageError::OutOfRange)?;
        let end = byte_offset(text, self.end).ok_or(PassageError::OutOfRange)?;
        if text[start..end] == self.quote {
            Ok(())
        } else {
            Err(PassageError::QuoteMismatch)
        }
    }
}

/// The byte offset of the character offset `chars` in `text`, or `None` after the end of the text.
fn byte_offset(text: &str, chars: u32) -> Option<usize> {
    let chars = usize::try_from(chars).ok()?;
    text.char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .nth(chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(start: u32, end: u32, quote: &str) -> Passage {
        Passage {
            start,
            end,
            quote: quote.to_owned(),
            page: None,
        }
    }

    const TEXT: &str = "Das Flugfeld öffnet im Mai.\nDer Eintritt ist gratis.";

    #[test]
    fn accepts_a_quote_that_matches_its_range() {
        assert_eq!(passage(4, 12, "Flugfeld").check(TEXT), Ok(()));
        // "ö" is one character, so the offsets count characters, not bytes.
        assert_eq!(passage(13, 19, "öffnet").check(TEXT), Ok(()));
        assert_eq!(
            passage(28, 52, "Der Eintritt ist gratis.").check(TEXT),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_quote_that_does_not_match_its_range() {
        assert_eq!(
            passage(4, 12, "Flugplatz").check(TEXT),
            Err(PassageError::QuoteMismatch)
        );
        assert_eq!(
            passage(5, 13, "Flugfeld").check(TEXT),
            Err(PassageError::QuoteMismatch)
        );
    }

    #[test]
    fn rejects_an_empty_range_a_range_after_the_text_and_page_zero() {
        assert_eq!(passage(4, 4, "").check(TEXT), Err(PassageError::Empty));
        assert_eq!(passage(5, 4, "").check(TEXT), Err(PassageError::Empty));
        assert_eq!(
            passage(50, 53, "is.").check(TEXT),
            Err(PassageError::OutOfRange)
        );
        let mut on_page_zero = passage(4, 12, "Flugfeld");
        on_page_zero.page = Some(0);
        assert_eq!(on_page_zero.check(TEXT), Err(PassageError::Page));
    }

    #[test]
    fn normalizes_line_ends_and_unicode() {
        // "o" with a combining diaeresis becomes the one character "ö".
        let text = SourceText::normalize("Flugfeld o\u{308}ffnet\r\nim Mai\rund Juni\n");
        assert_eq!(text.as_str(), "Flugfeld öffnet\nim Mai\nund Juni\n");
        assert_eq!(SourceText::normalize(text.as_str()), text);
    }

    #[test]
    fn debug_hides_the_text_and_the_quote() {
        let text = SourceText::normalize("Anna Muster");
        assert!(!format!("{text:?}").contains("Anna"));
        assert!(!format!("{:?}", passage(0, 4, "Anna")).contains("Anna"));
    }
}
