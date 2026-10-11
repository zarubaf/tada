//! The tools that read the documents of an event (ADR 0040).

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::events::EventInput;
use super::{AiToolError, AiToolPorts, TypedTool, find_event};
use tada_app::ai_queries;
use tada_app::caller::AiCaller;
use tada_app::domain::ids::DocumentVersionId;
use tada_app::paging::PageLimit;
use tada_app::views::{DocumentList, DocumentSummaryView, DraftVersionView};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DocumentVersionInput {
    /// The ID of a draft version, from `list_documents`.
    version_id: Uuid,
}

/// The description of `list_documents`, with the page size of `app`.
static LIST_DOCUMENTS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "List the documents of an event, the newest first, each with its readable ID, its record version and its newest version. \
A version is an upload (a file) or a draft (Markdown). To propose a new draft version of a document, send its version as expected_version. \
The list holds at most the {} newest documents and has no next page. If more is true, the event has older documents that this tool cannot show: tell the member.",
        PageLimit::MAX
    )
});

pub(super) struct ListDocuments;

#[async_trait]
impl TypedTool for ListDocuments {
    type Input = EventInput;
    type Output = DocumentList;
    const NAME: &'static str = "list_documents";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_DOCUMENTS
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: EventInput,
        ports: &AiToolPorts,
    ) -> Result<DocumentList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let page =
            ai_queries::list_documents(caller, event.id, None, None, PageLimit::LARGEST, ports)
                .await?;
        Ok(DocumentList {
            documents: page.items.iter().map(DocumentSummaryView::from).collect(),
            more: page.next.is_some(),
        })
    }
}

pub(super) struct GetDocumentVersion;

#[async_trait]
impl TypedTool for GetDocumentVersion {
    type Input = DocumentVersionInput;
    type Output = DraftVersionView;
    const NAME: &'static str = "get_document_version";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Get a draft version of a document: its Markdown, its status and its provenance manifest, the exact fact versions and source passages that it cites. \
An approved version never changes: write a new draft from the Markdown, cite the current fact versions of get_event_profile, and propose it with propose_changeset. \
An upload version has no Markdown and is not found here."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: DocumentVersionInput,
        ports: &AiToolPorts,
    ) -> Result<DraftVersionView, AiToolError> {
        let id = DocumentVersionId::from_uuid(input.version_id);
        let draft = ai_queries::get_draft(caller, id, ports).await?;
        Ok(DraftVersionView::from(&draft))
    }
}
