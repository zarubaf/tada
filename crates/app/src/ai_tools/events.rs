//! The tools that read events, their facts and their sources (ADR 0040).

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{AiToolError, AiToolPorts, TypedTool, find_event, no_input_schema};
use crate::caller::AiCaller;
use crate::domain::events::EventKey;
use crate::events;
use crate::facts;
use crate::paging::PageLimit;
use crate::search::{self, SearchRequest};
use crate::sources::{self, PassageRequest};
use crate::views::{
    EventList, EventSchema, EventView, PassageView, ProfileView, SearchHitView, SearchResult,
};

/// The input of the tools that read one event.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EventInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    pub(super) event_key: String,
}

/// A tool without input. It ignores the arguments of the call.
#[derive(Deserialize, JsonSchema)]
pub(super) struct NoInput {}

/// The description of `list_events`, with the page size of `app`.
static LIST_EVENTS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "List the events that the member can read, in the order of their keys. The other tools take the key. \
The list holds at most {} events and has no next page. If more is true, the member can read more events than this tool can show: tell the member.",
        PageLimit::MAX
    )
});

pub(super) struct ListEvents;

#[async_trait]
impl TypedTool for ListEvents {
    type Input = NoInput;
    type Output = EventList;
    const NAME: &'static str = "list_events";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_EVENTS
    }

    fn input_schema() -> Value {
        no_input_schema()
    }

    async fn run(
        &self,
        caller: &AiCaller,
        _input: NoInput,
        ports: &AiToolPorts,
    ) -> Result<EventList, AiToolError> {
        let page = events::list_events(caller, None, PageLimit::LARGEST, &*ports.events).await?;
        Ok(EventList {
            events: page.items.iter().map(EventView::from).collect(),
            more: page.next.is_some(),
        })
    }
}

pub(super) struct GetEventSchema;

#[async_trait]
impl TypedTool for GetEventSchema {
    type Input = EventInput;
    type Output = EventSchema;
    const NAME: &'static str = "get_event_schema";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Get the field catalog of an event: the active fields with their meaning and the JSON Schema of their values, and the kinds of records. \
Use an existing field first; propose a new field only if no field has the meaning."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: EventInput,
        ports: &AiToolPorts,
    ) -> Result<EventSchema, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let catalog =
            facts::get_field_catalog(caller, event.id, &*ports.identity, &*ports.facts).await?;
        Ok(EventSchema::new(&event, &catalog))
    }
}

pub(super) struct GetEventProfile;

#[async_trait]
impl TypedTool for GetEventProfile {
    type Input = EventInput;
    type Output = ProfileView;
    const NAME: &'static str = "get_event_profile";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Get what the team plans for an event: the accepted facts, the assumptions, the unknowns, the open questions and the open proposals, each in its own list. \
Each fact names its version and the source passages that support it. \
A field without a fact is not listed: compare with get_event_schema to see which fields nobody has addressed yet. \
Only accepted facts are confirmed. Never fill in an unknown, and never present an assumption or an open proposal as accepted."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: EventInput,
        ports: &AiToolPorts,
    ) -> Result<ProfileView, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let (identity, facts) = (&*ports.identity, &*ports.facts);
        let profile = facts::get_event_profile(caller, event.id, identity, facts).await?;
        let catalog = facts::get_field_catalog(caller, event.id, identity, facts).await?;
        Ok(ProfileView::new(&event, &profile, &catalog))
    }
}

pub(super) struct SearchSources;

#[async_trait]
impl TypedTool for SearchSources {
    type Input = SearchRequest;
    type Output = SearchResult;
    const NAME: &'static str = "search_sources";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Search the source texts that the member can read, for example the words of members and documents. \
Each hit names its source version and a snippet with its offsets. Cite a hit as a passage: source_version_id, start and end."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        request: SearchRequest,
        ports: &AiToolPorts,
    ) -> Result<SearchResult, AiToolError> {
        let hits = search::search_sources(
            caller,
            request,
            &*ports.events,
            &*ports.identity,
            &*ports.sources,
        )
        .await?;
        Ok(SearchResult {
            hits: hits.iter().map(SearchHitView::from).collect(),
        })
    }
}

pub(super) struct GetSourcePassage;

#[async_trait]
impl TypedTool for GetSourcePassage {
    type Input = PassageRequest;
    type Output = PassageView;
    const NAME: &'static str = "get_source_passage";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Get the exact text of a range of a source version, to quote it in a citation. \
The offsets count characters, as in the evidence of a fact and in a search hit."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        request: PassageRequest,
        ports: &AiToolPorts,
    ) -> Result<PassageView, AiToolError> {
        let passage =
            sources::get_source_passage(caller, request, &*ports.identity, &*ports.sources).await?;
        Ok(PassageView::from(&passage))
    }
}
