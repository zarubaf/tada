//! Document drafts (ADR 0051, ADR 0058): the provenance manifest and the draft lint.
//!
//! A draft is Markdown: CommonMark with tables. These functions only parse it.
//! They never make HTML, so raw HTML in a draft stays text.

use std::borrow::Cow;

use pulldown_cmark::{Options, Parser};

mod lint;
mod manifest;
mod provenance;

pub use lint::{LintKind, LintWarning, lint};
pub use manifest::{DraftError, FactLink, Manifest, SourceLink, extract};
pub use provenance::{CitedFact, DraftProvenance, ProvenanceManifest, source_uri};

/// Changes CRLF and CR line endings to LF, so that all functions count the same lines.
fn normalize_line_endings(markdown: &str) -> Cow<'_, str> {
    if markdown.contains('\r') {
        Cow::Owned(markdown.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(markdown)
    }
}

/// The parser of ADR 0058: tables on, all other extensions off.
fn parser(markdown: &str) -> Parser<'_> {
    Parser::new_ext(markdown, Options::ENABLE_TABLES)
}

/// The scheme of a link destination in lowercase, or `None` for a relative destination (RFC 3986).
fn scheme(destination: &str) -> Option<String> {
    let (scheme, _) = destination.split_once(':')?;
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// The scheme of the provenance links of ADR 0051.
const TADA_SCHEME: &str = "tada";

fn is_tada_link(destination: &str) -> bool {
    scheme(destination).as_deref() == Some(TADA_SCHEME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_html_gives_no_manifest_entry_and_a_lint_warning() {
        let markdown = "Titel\n\n<script>alert(1)</script>\n";

        assert_eq!(extract(markdown), Ok(Manifest::default()));
        assert_eq!(
            lint(markdown),
            vec![LintWarning {
                line: 3,
                kind: LintKind::RawHtml
            }]
        );
    }
}
