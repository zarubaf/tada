//! What a reader needs to show a draft (ADR 0051, ADR 0058): its Markdown, its lint warnings, and the target of each
//! `tada:` link for this reader. The server never formats a value; the client formats it and shows the state.

use std::collections::BTreeMap;

use tada_domain::RecordVersion;
use tada_domain::documents::DraftMarkdown;
use tada_domain::ids::{DocumentId, DocumentVersionId, EventId, FactId, SourceVersionId};
use tada_domain::sources::Evidence;

use super::{
    DocumentReads, ReadDocumentError, StoredDraft, VersionView, check_may_read, get_document,
};
use crate::access::{self, Principal};
use crate::drafts::{self, LintWarning, ProvenanceManifest};
use crate::facts::{FactStore, FactVersionRef};
use crate::identity::IdentityStore;
use crate::sources::SourceStore;
use crate::store::StoreError;

/// The target of a `tada:` link for one reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The cited fact version with its state: accepted, an assumption or unknown.
    Fact(FactVersionRef),
    /// The cited passage with its quote.
    Source(Evidence),
    /// The reader cannot see the target. The client shows „entfernt“.
    Hidden,
}

/// The target of each `tada:` link of a draft, by the canonical text of the link (ADR 0058).
pub type Links = BTreeMap<String, Resolution>;

/// A draft as one reader sees it: the same shape for a draft version and for a draft proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRendering {
    pub markdown: DraftMarkdown,
    pub lint_warnings: Vec<LintWarning>,
    pub links: Links,
}

/// A draft version as one reader sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendering {
    pub version: VersionView,
    pub draft: DraftRendering,
}

/// The target of each link of `manifest` for `reader`, in a draft of the event `event`.
///
/// A fact link resolves to its fact version if it is a fact version of the event.
/// A source link resolves to its passage if the source version is citable in the event for the reader:
/// the one rule of `access::citable_reach` for evidence and drafts.
/// Each other link is `Hidden`, so a reader never sees a target that the access rules do not give.
pub(crate) async fn resolve_links(
    reader: &impl Principal,
    event: EventId,
    manifest: &ProvenanceManifest,
    identity: &dyn IdentityStore,
    facts: &dyn FactStore,
    sources: &dyn SourceStore,
) -> Result<Links, StoreError> {
    let scope = reader.scope();
    let mut links = Links::new();
    let cited: Vec<(FactId, RecordVersion)> = manifest
        .facts
        .iter()
        .map(|fact| (fact.fact_id, fact.version))
        .collect();
    let found = if cited.is_empty() {
        Vec::new()
    } else {
        facts.fact_versions(scope, event, &cited).await?
    };
    for fact in &manifest.facts {
        let resolution = found
            .iter()
            .find(|version| (version.fact_id, version.number) == (fact.fact_id, fact.version))
            .map_or(Resolution::Hidden, |version| {
                Resolution::Fact(version.clone())
            });
        links.insert(fact.uri(), resolution);
    }
    let ids: Vec<SourceVersionId> = manifest
        .sources
        .iter()
        .map(|source| source.source_version_id)
        .collect();
    let readable: Vec<SourceVersionId> = if ids.is_empty() {
        Vec::new()
    } else {
        let reach = access::citable_reach(reader, event, identity).await?;
        sources
            .texts(scope, &reach, &ids)
            .await?
            .into_iter()
            .map(|text| text.id)
            .collect()
    };
    for source in &manifest.sources {
        let resolution = if readable.contains(&source.source_version_id) {
            Resolution::Source(source.clone())
        } else {
            Resolution::Hidden
        };
        links.insert(drafts::source_uri(source), resolution);
    }
    Ok(links)
}

/// The draft version `id`, with its Markdown and its manifest, for a reader of its event.
/// An upload version has no Markdown: it is not found here.
pub async fn get_draft(
    reader: &impl Principal,
    id: DocumentVersionId,
    stores: DocumentReads<'_>,
) -> Result<StoredDraft, ReadDocumentError> {
    let draft = stores
        .documents
        .draft(reader.scope(), id)
        .await?
        .ok_or(ReadDocumentError::NotFound)?;
    check_may_read(reader, draft.event_id, stores.identity).await?;
    Ok(draft)
}

/// The draft version `id` as `reader` sees it: its Markdown, its lint warnings and the target of each link.
pub async fn render_context(
    reader: &impl Principal,
    id: DocumentVersionId,
    stores: DocumentReads<'_>,
) -> Result<Rendering, ReadDocumentError> {
    let draft = get_draft(reader, id, stores).await?;
    let links = resolve_links(
        reader,
        draft.event_id,
        &draft.manifest,
        stores.identity,
        stores.facts,
        stores.sources,
    )
    .await?;
    Ok(Rendering {
        version: draft.version,
        draft: DraftRendering {
            // The lint is deterministic, so it gives the warnings that the proposal of the version had.
            lint_warnings: drafts::lint(draft.markdown.as_str()),
            markdown: draft.markdown,
            links,
        },
    })
}

/// True if the newest version of the document cites an older version of a fact (ADR 0051): the document shows
/// „Fakten geändert“. tada derives it at each read and never rewrites the document.
pub async fn facts_changed(
    reader: &impl Principal,
    document_id: DocumentId,
    stores: DocumentReads<'_>,
) -> Result<bool, ReadDocumentError> {
    get_document(reader, document_id, stores).await?;
    Ok(stores
        .documents
        .facts_changed(reader.scope(), document_id)
        .await?)
}
