//! The provenance manifest of a draft (ADR 0051) and the link rules of ADR 0058.

use std::fmt;
use std::str::FromStr;

use pulldown_cmark::{Event, LinkType, Tag, TagEnd};
use tada_domain::RecordVersion;
use uuid::Uuid;

use super::{TADA_SCHEME, normalize_line_endings, parser, scheme};
use crate::problem::ProblemCode;

/// The fact versions and source passages that a draft cites, in the order of their first link.
/// Each target is in the list once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub facts: Vec<FactLink>,
    pub sources: Vec<SourceLink>,
}

/// A fact link: `[](tada:fact/<fact-uuid>?v=<n>)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactLink {
    pub fact_id: Uuid,
    pub version: RecordVersion,
}

/// A source link: `[words](tada:source/<source-version-uuid>#<start>-<end>)`.
///
/// `Debug` hides the text, because logs must not contain document content (ADR 0035).
#[derive(Clone, PartialEq, Eq)]
pub struct SourceLink {
    pub source_version_id: Uuid,
    /// The first character of the passage in the source version.
    pub start: u32,
    /// The character after the passage.
    pub end: u32,
    /// The text of the link in the draft.
    pub text: String,
}

impl fmt::Debug for SourceLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceLink")
            .field("source_version_id", &self.source_version_id)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("text", &"[redacted]")
            .finish()
    }
}

/// A draft that breaks a link rule of ADR 0051 or ADR 0058.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DraftError {
    #[error("a fact link has text")]
    FactLinkWithText,
    #[error("a tada link is malformed")]
    MalformedTadaLink,
    #[error("a link scheme is not allowed")]
    SchemeNotAllowed,
    #[error("a draft cannot contain images")]
    ImageNotAllowed,
}

impl DraftError {
    /// All codes that this error can give, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[ProblemCode::ValidationFailed];

    pub fn code(&self) -> ProblemCode {
        ProblemCode::ValidationFailed
    }

    /// The code of the entry in the `errors` list of the problem.
    pub fn entry_code(&self) -> &'static str {
        match self {
            Self::FactLinkWithText => "fact-link-with-text",
            Self::MalformedTadaLink => "tada-link-malformed",
            Self::SchemeNotAllowed => "scheme-not-allowed",
            Self::ImageNotAllowed => "image-not-allowed",
        }
    }
}

/// Extracts the provenance manifest of a draft and checks its link rules.
///
/// Raw HTML stays text: it gives no link and no manifest entry.
pub fn extract(markdown: &str) -> Result<Manifest, DraftError> {
    let markdown = normalize_line_endings(markdown);
    let mut manifest = Manifest::default();
    // CommonMark links cannot contain links, so one open link is enough.
    let mut open: Option<OpenLink> = None;
    for event in parser(&markdown) {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                open = Some(OpenLink {
                    target: target(link_type, &dest_url)?,
                    text: String::new(),
                    has_content: false,
                });
            }
            Event::End(TagEnd::Link) => {
                if let Some(link) = open.take() {
                    manifest.add(link)?;
                }
            }
            event => {
                // A remote image loads when a reviewer opens the draft, so its address could
                // send draft content to a third party (ADR 0058).
                if let Event::Start(Tag::Image { .. }) = &event {
                    return Err(DraftError::ImageNotAllowed);
                }
                if let Some(link) = &mut open {
                    link.add_content(&event);
                }
            }
        }
    }
    Ok(manifest)
}

/// What a link destination points to.
enum Target {
    /// A `https` or `mailto` destination. It is not part of the manifest.
    External,
    Fact(FactLink),
    /// A source passage. The text comes from the link.
    Source(SourceLink),
}

/// A link that the parser started and did not end yet.
struct OpenLink {
    target: Target,
    text: String,
    has_content: bool,
}

impl OpenLink {
    fn add_content(&mut self, event: &Event<'_>) {
        self.has_content = true;
        match event {
            Event::Text(text) | Event::Code(text) => self.text.push_str(text),
            Event::SoftBreak | Event::HardBreak => self.text.push(' '),
            _ => {}
        }
    }
}

impl Manifest {
    fn add(&mut self, link: OpenLink) -> Result<(), DraftError> {
        match link.target {
            Target::External => {}
            Target::Fact(_) if link.has_content => return Err(DraftError::FactLinkWithText),
            Target::Fact(fact) => {
                if !self.facts.contains(&fact) {
                    self.facts.push(fact);
                }
            }
            Target::Source(source) => {
                let cited = self.sources.iter().any(|other| {
                    (other.source_version_id, other.start, other.end)
                        == (source.source_version_id, source.start, source.end)
                });
                if !cited {
                    self.sources.push(SourceLink {
                        text: link.text,
                        ..source
                    });
                }
            }
        }
        Ok(())
    }
}

/// Checks the scheme of a link destination and parses a `tada:` destination.
fn target(link_type: LinkType, destination: &str) -> Result<Target, DraftError> {
    // The parser gives `<anna@example.org>` without its implied `mailto:` scheme.
    if link_type == LinkType::Email {
        return Ok(Target::External);
    }
    match scheme(destination).as_deref() {
        Some("https" | "mailto") => Ok(Target::External),
        // Only the canonical form, so that the client can look up a link by its exact text.
        Some(TADA_SCHEME) => destination
            .strip_prefix("tada:")
            .and_then(tada_target)
            .ok_or(DraftError::MalformedTadaLink),
        _ => Err(DraftError::SchemeNotAllowed),
    }
}

/// Parses the part after `tada:`: `fact/<uuid>?v=<n>` or `source/<uuid>#<start>-<end>`.
fn tada_target(path: &str) -> Option<Target> {
    if let Some(rest) = path.strip_prefix("fact/") {
        let (id, version) = rest.split_once("?v=")?;
        Some(Target::Fact(FactLink {
            fact_id: hyphenated_uuid(id)?,
            version: RecordVersion::new(decimal(version)?)?,
        }))
    } else if let Some(rest) = path.strip_prefix("source/") {
        let (id, range) = rest.split_once('#')?;
        let (start, end) = range.split_once('-')?;
        let (start, end) = (decimal(start)?, decimal(end)?);
        if start >= end {
            return None;
        }
        Some(Target::Source(SourceLink {
            source_version_id: hyphenated_uuid(id)?,
            start,
            end,
            text: String::new(),
        }))
    } else {
        None
    }
}

/// A UUID in its lowercase hyphenated form only, so that each target has one spelling.
fn hyphenated_uuid(text: &str) -> Option<Uuid> {
    let canonical = text.len() == 36 && !text.bytes().any(|b| b.is_ascii_uppercase());
    canonical.then(|| Uuid::try_parse(text).ok())?
}

/// A number of ASCII digits only: no sign, no space and no leading zero.
fn decimal<T: FromStr>(text: &str) -> Option<T> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    let canonical = digits && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACT: &str = "0190f3a2-7b1c-7d4e-8f00-000000000001";
    const OTHER_FACT: &str = "0190f3a2-7b1c-7d4e-8f00-000000000002";
    const SOURCE: &str = "0190f3a2-7b1c-7d4e-8f00-0000000000a1";

    fn uuid(text: &str) -> Uuid {
        Uuid::parse_str(text).expect("valid UUID")
    }

    fn version(n: i64) -> RecordVersion {
        RecordVersion::new(n).expect("valid version")
    }

    #[test]
    fn a_fact_link_gives_its_fact_version() {
        let manifest = extract(&format!("Das Fest ist am [](tada:fact/{FACT}?v=3).\n"));

        assert_eq!(
            manifest,
            Ok(Manifest {
                facts: vec![FactLink {
                    fact_id: uuid(FACT),
                    version: version(3)
                }],
                sources: vec![],
            })
        );
    }

    #[test]
    fn a_fact_link_with_text_is_rejected() {
        let error = extract(&format!("Budget: [20'000](tada:fact/{FACT}?v=3).\n"));

        assert_eq!(error, Err(DraftError::FactLinkWithText));
        assert_eq!(
            DraftError::FactLinkWithText.entry_code(),
            "fact-link-with-text"
        );
    }

    #[test]
    fn a_fact_link_with_any_content_is_rejected() {
        for content in [" ", "`x`", "*x*", "<b>"] {
            let markdown = format!("[{content}](tada:fact/{FACT}?v=3)\n");
            assert_eq!(
                extract(&markdown),
                Err(DraftError::FactLinkWithText),
                "{markdown}"
            );
        }
    }

    #[test]
    fn a_source_link_gives_its_passage_and_text() {
        let manifest = extract(&format!(
            "Die Anmeldung schliesst [am 1. April](tada:source/{SOURCE}#12-40).\n"
        ));

        assert_eq!(
            manifest,
            Ok(Manifest {
                facts: vec![],
                sources: vec![SourceLink {
                    source_version_id: uuid(SOURCE),
                    start: 12,
                    end: 40,
                    text: "am 1. April".to_owned(),
                }],
            })
        );
    }

    #[test]
    fn a_target_is_in_the_manifest_once_in_the_order_of_its_first_link() {
        let markdown = format!(
            "[](tada:fact/{OTHER_FACT}?v=1) [](tada:fact/{FACT}?v=2) [](tada:fact/{OTHER_FACT}?v=1).\n\
             [a](tada:source/{SOURCE}#1-2) [b](tada:source/{SOURCE}#1-2) [c](tada:source/{SOURCE}#3-4).\n"
        );

        let manifest = extract(&markdown).expect("valid draft");

        assert_eq!(
            manifest.facts,
            vec![
                FactLink {
                    fact_id: uuid(OTHER_FACT),
                    version: version(1)
                },
                FactLink {
                    fact_id: uuid(FACT),
                    version: version(2)
                },
            ]
        );
        let passages: Vec<_> = manifest
            .sources
            .iter()
            .map(|s| (s.start, s.end, s.text.as_str()))
            .collect();
        assert_eq!(passages, vec![(1, 2, "a"), (3, 4, "c")]);
    }

    #[test]
    fn a_scheme_outside_the_allow_list_is_rejected() {
        for markdown in [
            "[Text](javascript:alert(1))",
            "[Text](JavaScript:alert(1))",
            "<javascript:alert(1)>",
            "[Text](http://example.org)",
            "[Text](data:text/html,x)",
            "[Text](relative/path)",
            "[Text](#anchor)",
            "[Text][ref]\n\n[ref]: javascript:alert(1)",
            "| a |\n| - |\n| [x](ftp://example.org) |",
        ] {
            assert_eq!(
                extract(markdown),
                Err(DraftError::SchemeNotAllowed),
                "{markdown}"
            );
        }
        assert_eq!(
            DraftError::SchemeNotAllowed.entry_code(),
            "scheme-not-allowed"
        );
    }

    #[test]
    fn every_image_is_rejected() {
        for markdown in [
            "![Logo](https://example.org/logo.png)",
            "![Bild](http://example.org/a.png)",
            "![](https://attacker.example/?d=Budget)",
            &format!("![](tada:fact/{FACT}?v=3)"),
            "[![Logo](https://example.org/logo.png)](https://example.org)",
            "![Bild][ref]\n\n[ref]: https://example.org/a.png",
        ] {
            assert_eq!(
                extract(markdown),
                Err(DraftError::ImageNotAllowed),
                "{markdown}"
            );
        }
        assert_eq!(
            DraftError::ImageNotAllowed.entry_code(),
            "image-not-allowed"
        );
    }

    #[test]
    fn allowed_schemes_give_no_manifest_entry() {
        for markdown in [
            "[Website](https://example.org)",
            "[Mail](mailto:anna@example.org)",
            "<https://example.org>",
            "<anna@example.org>",
        ] {
            assert_eq!(extract(markdown), Ok(Manifest::default()), "{markdown}");
        }
    }

    #[test]
    fn a_malformed_tada_link_is_rejected() {
        for destination in [
            format!("tada:fact/{FACT}"),
            format!("tada:fact/{FACT}?v="),
            format!("tada:fact/{FACT}?v=0"),
            format!("tada:fact/{FACT}?v=-1"),
            format!("tada:fact/{FACT}?v=x"),
            format!("tada:fact/{FACT}?v=3&x=1"),
            format!("tada:fact/{FACT}?v=3#1-2"),
            "tada:fact/not-a-uuid?v=1".to_owned(),
            format!("tada:fact/{}?v=1", FACT.replace('-', "")),
            format!("tada:source/{SOURCE}"),
            format!("tada:source/{SOURCE}#5"),
            format!("tada:source/{SOURCE}#5-5"),
            format!("tada:source/{SOURCE}#9-3"),
            format!("tada:source/{SOURCE}#a-3"),
            format!("tada:source/{SOURCE}#1-99999999999"),
            format!("tada:event/{FACT}"),
            "tada:".to_owned(),
            // Only the canonical spelling: lowercase scheme and UUID, no leading zeros.
            format!("TADA:fact/{FACT}?v=3"),
            format!("Tada:source/{SOURCE}#1-2"),
            format!("tada:fact/{}?v=3", FACT.to_uppercase()),
            format!("tada:source/{}#1-2", SOURCE.to_uppercase()),
            format!("tada:fact/{FACT}?v=03"),
            format!("tada:source/{SOURCE}#01-2"),
            format!("tada:source/{SOURCE}#1-02"),
            format!("tada:source/{SOURCE}#00-2"),
        ] {
            let markdown = format!("[](<{destination}>)");
            assert_eq!(
                extract(&markdown),
                Err(DraftError::MalformedTadaLink),
                "{markdown}"
            );
        }
        assert_eq!(
            DraftError::MalformedTadaLink.entry_code(),
            "tada-link-malformed"
        );
    }

    #[test]
    fn a_passage_can_start_at_zero() {
        let manifest = extract(&format!("[a](tada:source/{SOURCE}#0-2)")).expect("valid draft");

        assert_eq!(manifest.sources[0].start, 0);
    }

    #[test]
    fn debug_hides_the_text_of_a_source_link() {
        let manifest =
            extract(&format!("[Budget von Anna](tada:source/{SOURCE}#1-2)")).expect("valid draft");

        let debug = format!("{manifest:?}");

        assert!(!debug.contains("Budget von Anna"), "{debug}");
        assert!(debug.contains("[redacted]"), "{debug}");
        assert!(debug.contains(SOURCE), "{debug}");
    }

    #[test]
    fn raw_html_is_accepted_as_text_without_a_link() {
        for markdown in [
            "<script>alert(1)</script>\n",
            "Text <a href=\"javascript:alert(1)\">x</a>.\n",
            &format!("<a href=\"tada:fact/{FACT}?v=3\"></a>\n"),
        ] {
            assert_eq!(extract(markdown), Ok(Manifest::default()), "{markdown}");
        }
    }

    #[test]
    fn line_endings_do_not_change_the_manifest() {
        let lf = format!("Am [](tada:fact/{FACT}?v=3).\n\n[a](tada:source/{SOURCE}#1-2)\n");

        assert_eq!(extract(&lf.replace('\n', "\r\n")), extract(&lf));
        assert_eq!(extract(&lf.replace('\n', "\r")), extract(&lf));
        assert_ne!(extract(&lf), Ok(Manifest::default()));
    }

    #[test]
    fn every_error_code_is_in_codes() {
        for error in [
            DraftError::FactLinkWithText,
            DraftError::MalformedTadaLink,
            DraftError::SchemeNotAllowed,
            DraftError::ImageNotAllowed,
        ] {
            assert!(DraftError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
