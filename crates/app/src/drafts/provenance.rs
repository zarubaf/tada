//! The resolved provenance manifest of a draft proposal (ADR 0051): the typed targets that each link cites.
//!
//! `extract` gives the links as the draft spells them. When tada stores a draft proposal, it resolves each link
//! against the stored fact versions and source versions, and fixes this manifest with the proposal.

use tada_domain::RecordVersion;
use tada_domain::ids::{FactId, ProposalId};
use tada_domain::sources::Evidence;

use super::LintWarning;

/// The fact versions and source passages that a draft cites, in the order of their first link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProvenanceManifest {
    pub facts: Vec<CitedFact>,
    /// Each cited passage with the quote of its range.
    pub sources: Vec<Evidence>,
}

/// An exact fact version: accepted, an assumption or unknown, never an open proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitedFact {
    pub fact_id: FactId,
    pub version: RecordVersion,
}

/// What tada fixes with a draft proposal at its creation: the manifest and the lint warnings for the review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftProvenance {
    pub proposal_id: ProposalId,
    pub manifest: ProvenanceManifest,
    pub lint_warnings: Vec<LintWarning>,
}

impl CitedFact {
    /// The canonical link of the fact version (ADR 0058), for example `tada:fact/<uuid>?v=3`.
    /// The client finds each link of a draft by this exact text.
    pub fn uri(&self) -> String {
        format!(
            "tada:fact/{}?v={}",
            self.fact_id.as_uuid(),
            self.version.get()
        )
    }
}

/// The canonical link of a cited passage (ADR 0058), for example `tada:source/<uuid>#4-12`.
pub fn source_uri(evidence: &Evidence) -> String {
    format!(
        "tada:source/{}#{}-{}",
        evidence.source_version_id.as_uuid(),
        evidence.passage.start,
        evidence.passage.end
    )
}

#[cfg(test)]
mod tests {
    use tada_domain::ids::SourceVersionId;
    use tada_domain::sources::Passage;
    use uuid::Uuid;

    use super::*;
    use crate::drafts::extract;

    #[test]
    fn the_extraction_reads_each_canonical_link_back() {
        let fact = CitedFact {
            fact_id: FactId::from_uuid(Uuid::from_u128(0x0190_f3a2_7b1c_7d4e_8f00_0000_0000_0001)),
            version: RecordVersion::new(12).unwrap(),
        };
        let evidence = Evidence {
            source_version_id: SourceVersionId::from_uuid(Uuid::from_u128(0xab)),
            passage: Passage {
                start: 4,
                end: 120,
                quote: String::new(),
                page: None,
            },
        };
        let markdown = format!(
            "Am [{}]({}) [hier]({}).\n",
            "",
            fact.uri(),
            source_uri(&evidence)
        );
        let manifest = extract(&markdown).unwrap();

        assert_eq!(manifest.facts[0].fact_id, fact.fact_id.as_uuid());
        assert_eq!(manifest.facts[0].version, fact.version);
        let source = &manifest.sources[0];
        assert_eq!(
            (source.source_version_id, source.start, source.end),
            (evidence.source_version_id.as_uuid(), 4, 120)
        );
    }
}
