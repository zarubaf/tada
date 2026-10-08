//! The read tools of ADR 0040, also for documents, and the server handler of all tools; `propose` has the proposal tool.
//! The names are stable; the descriptions tell the agent the rules.

use std::sync::Arc;

use axum::http::request::Parts;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::{Extension, IntoCallToolResult, ToolCallContext};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use tada_app::caller::AiCaller;
use tada_app::clock::Clock;
use tada_app::documents::{self, DocumentReads, DocumentStore};
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::{DocumentVersionId, SourceVersionId};
use tada_app::events::{self, EventStore};
use tada_app::facts::{self, FactStore};
use tada_app::identity::IdentityStore;
use tada_app::paging::PageLimit;
use tada_app::problem::{FieldError, ProblemCode};
use tada_app::proposals::ProposalStore;
use tada_app::search::{self, SearchRequest};
use tada_app::sources::{self, SourceStore};
use tada_app::views::{
    DocumentList, DocumentSummaryView, DraftVersionView, EventList, EventSchema, EventView,
    PassageView, ProfileView, SearchHitView, SearchResult,
};
use uuid::Uuid;

use crate::McpState;
use crate::errors::{ToolError, caller};

/// The rules for each agent, in the `initialize` answer.
const INSTRUCTIONS: &str = "tada holds the planning data of the events of a club. \
Accepted facts are confirmed; assumptions are not confirmed; unknowns have no value. \
Rules: use the existing fields of get_event_schema first. Never fill in an unknown value and never present an assumption or an open proposal as accepted. \
Cite each statement with the source_version_id and the passage (start, end) that supports it; get_source_passage gives the exact quote. \
Propose changes with propose_changeset; the member reviews them in tada, and no tool accepts, rejects or deletes.";

/// The tools of one request. They read and propose with the rights of the member of the token.
#[derive(Debug, Clone)]
pub(crate) struct Tools {
    events: Arc<dyn EventStore>,
    pub(crate) identity: Arc<dyn IdentityStore>,
    pub(crate) facts: Arc<dyn FactStore>,
    pub(crate) sources: Arc<dyn SourceStore>,
    pub(crate) proposals: Arc<dyn ProposalStore>,
    pub(crate) documents: Arc<dyn DocumentStore>,
    pub(crate) clock: Arc<dyn Clock>,
}

/// The input of the tools that read one event.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    event_key: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchInput {
    /// The key of one event. Without it, the search reads each event that the member can read.
    #[serde(default)]
    event_key: Option<String>,
    /// The words to find, in the syntax of a web search: words, `"a phrase"`, `or` and `-word`.
    query: String,
    /// The largest number of hits, 1 to 50. The default is 10.
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PassageInput {
    source_version_id: Uuid,
    /// The offset of the first character, in characters of the normalized text.
    start: u32,
    /// The offset after the last character.
    end: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentVersionInput {
    /// The ID of a draft version, from `list_documents`.
    version_id: Uuid,
}

#[tool_router(router = read_tools)]
impl Tools {
    pub(crate) fn new(state: &McpState) -> Self {
        Self {
            events: state.events.clone(),
            identity: state.identity.clone(),
            facts: state.facts.clone(),
            sources: state.sources.clone(),
            proposals: state.proposals.clone(),
            documents: state.documents.clone(),
            clock: state.clock.clone(),
        }
    }

    #[tool(
        name = "list_events",
        description = "List the events that the member can read, with their keys. The other tools take the key.",
        annotations(read_only_hint = true)
    )]
    async fn list_events(
        &self,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<EventList>, ToolError> {
        let caller = caller(&parts)?;
        let page = events::list_events(
            caller,
            None,
            PageLimit::new(PageLimit::MAX).unwrap_or_default(),
            &*self.events,
        )
        .await?;
        Ok(Json(EventList {
            events: page.items.iter().map(EventView::from).collect(),
            more: page.next.is_some(),
        }))
    }

    #[tool(
        name = "get_event_schema",
        description = "Get the field catalog of an event: the active fields with their meaning and the JSON Schema of their values, and the kinds of records. \
Use an existing field first; propose a new field only if no field has the meaning.",
        annotations(read_only_hint = true)
    )]
    async fn get_event_schema(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<EventInput>,
    ) -> Result<Json<EventSchema>, ToolError> {
        let caller = caller(&parts)?;
        let event = self.event(caller, &input.event_key).await?;
        let catalog =
            facts::get_field_catalog(caller, event.id, &*self.identity, &*self.facts).await?;
        Ok(Json(EventSchema::new(&event, &catalog)))
    }

    #[tool(
        name = "get_event_profile",
        description = "Get what the team plans for an event: the accepted facts, the assumptions, the unknowns, the open questions and the open proposals, each in its own list. \
Each fact names its version and the source passages that support it. \
A field without a fact is not listed: compare with get_event_schema to see which fields nobody has addressed yet. \
Only accepted facts are confirmed. Never fill in an unknown, and never present an assumption or an open proposal as accepted.",
        annotations(read_only_hint = true)
    )]
    async fn get_event_profile(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<EventInput>,
    ) -> Result<Json<ProfileView>, ToolError> {
        let caller = caller(&parts)?;
        let event = self.event(caller, &input.event_key).await?;
        let profile =
            facts::get_event_profile(caller, event.id, &*self.identity, &*self.facts).await?;
        let catalog =
            facts::get_field_catalog(caller, event.id, &*self.identity, &*self.facts).await?;
        Ok(Json(ProfileView::new(&event, &profile, &catalog)))
    }

    #[tool(
        name = "search_sources",
        description = "Search the source texts that the member can read, for example the words of members and documents. \
Each hit names its source version and a snippet with its offsets. Cite a hit as a passage: source_version_id, start and end.",
        annotations(read_only_hint = true)
    )]
    async fn search_sources(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<SearchInput>,
    ) -> Result<Json<SearchResult>, ToolError> {
        let caller = caller(&parts)?;
        let request = SearchRequest {
            event_key: input.event_key,
            query: input.query,
            limit: input.limit,
        };
        let hits = search::search_sources(
            caller,
            request,
            &*self.events,
            &*self.identity,
            &*self.sources,
        )
        .await?;
        Ok(Json(SearchResult {
            hits: hits.iter().map(SearchHitView::from).collect(),
        }))
    }

    #[tool(
        name = "get_source_passage",
        description = "Get the exact text of a range of a source version, to quote it in a citation. \
The offsets count characters, as in the evidence of a fact and in a search hit.",
        annotations(read_only_hint = true)
    )]
    async fn get_source_passage(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<PassageInput>,
    ) -> Result<Json<PassageView>, ToolError> {
        let caller = caller(&parts)?;
        let passage = sources::get_source_passage(
            caller,
            SourceVersionId::from_uuid(input.source_version_id),
            input.start,
            input.end,
            &*self.identity,
            &*self.sources,
        )
        .await?;
        Ok(Json(PassageView::from(&passage)))
    }
}

#[tool_router(router = document_tools)]
impl Tools {
    #[tool(
        name = "list_documents",
        description = "List the documents of an event, the newest first, each with its readable ID, its record version and its newest version. \
A version is an upload (a file) or a draft (Markdown). To propose a new draft version of a document, send its version as expected_version. \
The list holds at most the 200 newest documents and has no next page. If more is true, the event has older documents that this tool cannot show: tell the member.",
        annotations(read_only_hint = true)
    )]
    async fn list_documents(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<EventInput>,
    ) -> Result<Json<DocumentList>, ToolError> {
        let caller = caller(&parts)?;
        let event = self.event(caller, &input.event_key).await?;
        let page = documents::list_documents(
            caller,
            event.id,
            None,
            None,
            PageLimit::new(PageLimit::MAX).unwrap_or_default(),
            self.document_reads(),
        )
        .await?;
        Ok(Json(DocumentList {
            documents: page.items.iter().map(DocumentSummaryView::from).collect(),
            more: page.next.is_some(),
        }))
    }

    #[tool(
        name = "get_document_version",
        description = "Get a draft version of a document: its Markdown, its status and its provenance manifest, the exact fact versions and source passages that it cites. \
An approved version never changes: write a new draft from the Markdown, cite the current fact versions of get_event_profile, and propose it with propose_changeset. \
An upload version has no Markdown and is not found here.",
        annotations(read_only_hint = true)
    )]
    async fn get_document_version(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<DocumentVersionInput>,
    ) -> Result<Json<DraftVersionView>, ToolError> {
        let caller = caller(&parts)?;
        let draft = documents::get_draft(
            caller,
            DocumentVersionId::from_uuid(input.version_id),
            self.document_reads(),
        )
        .await?;
        Ok(Json(DraftVersionView::from(&draft)))
    }
}

impl Tools {
    /// All tools of the server.
    fn all_tools() -> ToolRouter<Self> {
        Self::read_tools() + Self::document_tools() + Self::propose_tools()
    }

    fn document_reads(&self) -> DocumentReads<'_> {
        DocumentReads {
            identity: &*self.identity,
            documents: &*self.documents,
            facts: &*self.facts,
            sources: &*self.sources,
        }
    }

    /// The event with the key `key`, if the caller can read it.
    async fn event(
        &self,
        caller: &AiCaller,
        key: &str,
    ) -> Result<tada_app::domain::events::Event, ToolError> {
        let key = EventKey::parse(key).map_err(|error| {
            let error = FieldError::new("event_key", events::key_error_code(error));
            ToolError::problem(ProblemCode::ValidationFailed, &[error])
        })?;
        Ok(events::find_event(caller, &key, &*self.events, &*self.identity).await?)
    }
}

#[tool_handler(router = Self::all_tools())]
impl ServerHandler for Tools {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let call = ToolCallContext::new(self, request, context);
        match Self::all_tools().call(call).await? {
            // rmcp answers arguments that do not match the input schema with a text that can repeat input values
            // (ADR 0037). Each refusal of a tool has structured content, so the result without it is that answer.
            CallToolResponse::Complete(result)
                if result.is_error == Some(true) && result.structured_content.is_none() =>
            {
                ToolError::problem(ProblemCode::MalformedRequest, &[]).into_call_tool_result()
            }
            response => Ok(response),
        }
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tada", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
