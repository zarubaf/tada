//! The checks of draft proposals (ADR 0051): tada resolves each link of a draft and fixes its provenance manifest.

use std::collections::HashMap;

use tada_domain::RecordVersion;
use tada_domain::ids::{EventId, FactId, SourceVersionId};
use tada_domain::proposals::{DraftDocument, Operation, Proposal};
use tada_domain::sources::Passage;

use super::{MayPropose, ProposeError, ProposeStores, finish};
use crate::access::{self, AccessError};
use crate::drafts::{self, CitedFact, CitedPassage, DraftProvenance, ProvenanceManifest};
use crate::problem::FieldError;
use crate::sources::SourceVersionText;
use crate::store::StoreError;

/// The entry code of a link whose target does not exist, is not visible for the caller, or is an open proposal.
/// The three cases look the same, so a caller cannot find out which records exist (ADR 0006).
const LINK_NOT_FOUND: &str = "link-not-found";

/// Resolves the links of each draft proposal and gives its provenance manifest and lint warnings (ADR 0051).
///
/// 1. The Markdown follows the link rules of ADR 0058.
/// 2. An existing document is a document of the event of the proposal.
/// 3. Each fact link cites a stored fact version of the event: accepted, an assumption or unknown.
///    An open proposal has no fact version, so a link to it does not resolve.
/// 4. Each source link cites a source version that the caller can see, and its range is inside the text.
pub(super) async fn check_drafts(
    caller: &impl MayPropose,
    proposals: &[Proposal],
    stores: ProposeStores<'_>,
) -> Result<Vec<DraftProvenance>, ProposeError> {
    let mut errors = Vec::new();
    let mut checked = Vec::new();
    let mut visible = Visibility::default();
    for (index, proposal) in proposals.iter().enumerate() {
        let Operation::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } = &proposal.operation
        else {
            continue;
        };
        let path = |field: &str| format!("proposals/{index}/operation/{field}");
        let before = errors.len();
        if let DraftDocument::Existing { document_id, .. } = document {
            let found = stores
                .documents
                .get(caller.scope(), *document_id)
                .await?
                .is_some_and(|stored| stored.event_id == *event_id);
            if !found {
                errors.push(FieldError::new(
                    path("document/existing/document_id"),
                    "unknown-document",
                ));
            }
        }
        let links = match drafts::extract(markdown.as_str()) {
            Ok(links) => links,
            Err(error) => {
                errors.push(FieldError::new(path("markdown"), error.entry_code()));
                continue;
            }
        };
        let facts = resolve_facts(caller, *event_id, &links.facts, stores).await?;
        let sources = resolve_sources(caller, &links.sources, stores, &mut visible).await?;
        let manifest = match (facts, sources) {
            (Ok(facts), Ok(sources)) => ProvenanceManifest { facts, sources },
            (facts, sources) => {
                let codes = [facts.err(), sources.err()];
                errors.extend(
                    codes
                        .into_iter()
                        .flatten()
                        .map(|code| FieldError::new(path("markdown"), code)),
                );
                continue;
            }
        };
        if errors.len() == before {
            checked.push(DraftProvenance {
                proposal_id: proposal.id,
                manifest,
                lint_warnings: drafts::lint(markdown.as_str()),
            });
        }
    }
    finish(errors)?;
    Ok(checked)
}

/// The cited fact versions, or `link-not-found` if one is not a fact version of the event.
async fn resolve_facts(
    caller: &impl MayPropose,
    event: EventId,
    links: &[drafts::FactLink],
    stores: ProposeStores<'_>,
) -> Result<Result<Vec<CitedFact>, &'static str>, StoreError> {
    let cited: Vec<(FactId, RecordVersion)> = links
        .iter()
        .map(|link| (FactId::from_uuid(link.fact_id), link.version))
        .collect();
    if cited.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let existing = stores
        .facts
        .existing_versions(caller.scope(), event, &cited)
        .await?;
    if !cited.iter().all(|version| existing.contains(version)) {
        return Ok(Err(LINK_NOT_FOUND));
    }
    Ok(Ok(cited
        .into_iter()
        .map(|(fact_id, version)| CitedFact { fact_id, version })
        .collect()))
}

/// The cited passages with their quotes, or the entry code of the first link that does not resolve.
async fn resolve_sources(
    caller: &impl MayPropose,
    links: &[drafts::SourceLink],
    stores: ProposeStores<'_>,
    visible: &mut Visibility,
) -> Result<Result<Vec<CitedPassage>, &'static str>, StoreError> {
    let ids: Vec<SourceVersionId> = links
        .iter()
        .map(|link| SourceVersionId::from_uuid(link.source_version_id))
        .collect();
    if ids.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let texts: HashMap<SourceVersionId, SourceVersionText> = stores
        .sources
        .texts(caller.scope(), &ids)
        .await?
        .into_iter()
        .map(|text| (text.id, text))
        .collect();
    let mut cited = Vec::new();
    for (link, id) in links.iter().zip(ids) {
        let Some(source) = texts.get(&id) else {
            return Ok(Err(LINK_NOT_FOUND));
        };
        if !visible.sees(caller, source.event_id, stores).await? {
            return Ok(Err(LINK_NOT_FOUND));
        }
        match cited_passage(source, link) {
            Some(passage) => cited.push(passage),
            None => return Ok(Err("out-of-range")),
        }
    }
    Ok(Ok(cited))
}

/// The passage of a source link with the quote of its range, or `None` if the source version has no text for the range.
/// Evidence in drafts and in proposals is a passage of a source version (ADR 0050); this is the one place that maps a link to it.
fn cited_passage(source: &SourceVersionText, link: &drafts::SourceLink) -> Option<CitedPassage> {
    let text = source.text.as_ref()?;
    let passage = Passage::of_range(text.as_str(), link.start, link.end).ok()?;
    Some(CitedPassage {
        source_version_id: source.id,
        passage,
    })
}

/// The events that the caller can read, asked once each.
#[derive(Default)]
struct Visibility(HashMap<Option<EventId>, bool>);

impl Visibility {
    /// True if the caller can read a source item of the event `event`.
    /// A source item without an event belongs to the organization: only owners and admins see it (ADR 0052).
    async fn sees(
        &mut self,
        caller: &impl MayPropose,
        event: Option<EventId>,
        stores: ProposeStores<'_>,
    ) -> Result<bool, StoreError> {
        if let Some(known) = self.0.get(&event) {
            return Ok(*known);
        }
        let sees = match event {
            None => access::sees_all_events(caller),
            Some(event) => match access::event_access(caller, event, stores.identity).await {
                Ok(access) => access.can_read(),
                Err(AccessError::NotFound) => false,
                Err(AccessError::Store(error)) => return Err(error),
            },
        };
        self.0.insert(event, sees);
        Ok(sees)
    }
}
