//! The read tools of ADR 0040. The names are stable; the descriptions tell the agent the rules.

use std::sync::Arc;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tada_app::caller::AiCaller;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::SourceVersionId;
use tada_app::events::{self, EventStore};
use tada_app::facts::{self, FactStore};
use tada_app::identity::IdentityStore;
use tada_app::paging::PageLimit;
use tada_app::problem::{CommandError, FieldError, ProblemCode};
use tada_app::search::{self, SearchRequest};
use tada_app::sources::{self, SourceStore};
use tada_app::views::{
    EventList, EventSchema, EventView, PassageView, ProfileView, SearchHitView, SearchResult,
};
use uuid::Uuid;

use crate::McpState;

/// The rules for each agent, in the `initialize` answer.
const INSTRUCTIONS: &str = "tada holds the planning data of the events of a club. \
Accepted facts are confirmed; assumptions are not confirmed; unknowns have no value. \
Rules: use the existing fields of get_event_schema first. Never fill in an unknown value and never present an assumption or an open proposal as accepted. \
Cite each statement with the source_version_id and the passage (start, end) that supports it; get_source_passage gives the exact quote.";

/// The tools of one request. They read with the rights of the member of the token.
#[derive(Debug, Clone)]
pub(crate) struct Tools {
    events: Arc<dyn EventStore>,
    identity: Arc<dyn IdentityStore>,
    facts: Arc<dyn FactStore>,
    sources: Arc<dyn SourceStore>,
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

#[tool_router]
impl Tools {
    pub(crate) fn new(state: &McpState) -> Self {
        Self {
            events: state.events.clone(),
            identity: state.identity.clone(),
            facts: state.facts.clone(),
            sources: state.sources.clone(),
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
    ) -> Result<Json<EventList>, ErrorData> {
        let caller = caller(&parts)?;
        let page = events::list_events(
            caller,
            None,
            PageLimit::new(PageLimit::MAX).unwrap_or_default(),
            &*self.events,
        )
        .await
        .map_err(tool_error)?;
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
    ) -> Result<Json<EventSchema>, ErrorData> {
        let caller = caller(&parts)?;
        let event = self.event(caller, &input.event_key).await?;
        let catalog = facts::get_field_catalog(caller, event.id, &*self.identity, &*self.facts)
            .await
            .map_err(tool_error)?;
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
    ) -> Result<Json<ProfileView>, ErrorData> {
        let caller = caller(&parts)?;
        let event = self.event(caller, &input.event_key).await?;
        let profile = facts::get_event_profile(caller, event.id, &*self.identity, &*self.facts)
            .await
            .map_err(tool_error)?;
        let catalog = facts::get_field_catalog(caller, event.id, &*self.identity, &*self.facts)
            .await
            .map_err(tool_error)?;
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
    ) -> Result<Json<SearchResult>, ErrorData> {
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
        .await
        .map_err(tool_error)?;
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
    ) -> Result<Json<PassageView>, ErrorData> {
        let caller = caller(&parts)?;
        let passage = sources::get_source_passage(
            caller,
            SourceVersionId::from_uuid(input.source_version_id),
            input.start,
            input.end,
            &*self.identity,
            &*self.sources,
        )
        .await
        .map_err(tool_error)?;
        Ok(Json(PassageView::from(&passage)))
    }
}

impl Tools {
    /// The event with the key `key`, if the caller can read it.
    async fn event(
        &self,
        caller: &AiCaller,
        key: &str,
    ) -> Result<tada_app::domain::events::Event, ErrorData> {
        let key = EventKey::parse(key).map_err(|error| {
            invalid(&[FieldError::new("event_key", events::key_error_code(error))])
        })?;
        events::find_event(caller, &key, &*self.events, &*self.identity)
            .await
            .map_err(tool_error)
    }
}

#[tool_handler]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tada", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// The AI caller that the guard found for the request.
fn caller(parts: &Parts) -> Result<&AiCaller, ErrorData> {
    // The guard runs before each request, so a missing caller is a wiring error: fail closed.
    parts
        .extensions
        .get::<AiCaller>()
        .ok_or_else(|| ErrorData::internal_error(ProblemCode::Internal.meaning(), None))
}

/// The error of an `app` query as a JSON-RPC error with the problem code (ADR 0037).
/// A store failure goes to the log; the agent sees the code only.
fn tool_error(error: impl CommandError) -> ErrorData {
    if let Some(store_error) = error.store_error() {
        tracing::error!(error = %crate::guard::error_chain(store_error), "the store failed");
    }
    let code = error.code();
    if code == ProblemCode::ValidationFailed {
        return invalid(error.field_errors());
    }
    let data = Some(json!({"code": code.as_str()}));
    match code {
        ProblemCode::NotFound => ErrorData::resource_not_found(code.meaning(), data),
        _ => ErrorData::internal_error(code.meaning(), data),
    }
}

fn invalid(errors: &[FieldError]) -> ErrorData {
    let code = ProblemCode::ValidationFailed;
    let errors: Vec<_> = errors
        .iter()
        // A JSON pointer into the arguments of the tool, as in the problems of the API (ADR 0037).
        .map(|error| json!({"pointer": format!("/{}", error.field), "code": error.code}))
        .collect();
    ErrorData::invalid_params(
        code.meaning(),
        Some(json!({"code": code.as_str(), "errors": errors})),
    )
}
