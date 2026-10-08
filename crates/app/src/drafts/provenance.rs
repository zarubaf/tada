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
