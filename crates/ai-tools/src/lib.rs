//! The tools of AI (ADR 0010, ADR 0040): the read tools and the proposal tool of the `app` catalog.
//! The MCP server shows them to the AI clients of members, and an agent run gives them to a model.
//!
//! This crate uses only the public interface of `app`. Each tool gets an `AiCaller`, and the compiler
//! rejects a call of a command that does not take one, so a tool can only read and propose (ADR 0039).
//! The names are stable; the descriptions tell the agent the rules.

mod documents;
mod events;
mod propose;
mod work;

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use tada_app::ai_tools::{AiTool, AiToolError, ToolCatalog};
use tada_app::caller::AiCaller;
use tada_app::clock::Clock;
use tada_app::documents::DocumentStore;
use tada_app::domain::events::{Event, EventKey};
use tada_app::events::{self as event_queries, EventStore};
use tada_app::facts::FactStore;
use tada_app::identity::IdentityStore;
use tada_app::parties::PartyStore;
use tada_app::problem::{FieldError, ProblemCode};
use tada_app::proposals::ProposalStore;
use tada_app::sources::SourceStore;
use tada_app::work::WorkStore;
use tada_app::workstreams::WorkstreamStore;

/// The ports that the tools read and propose through.
#[derive(Debug, Clone)]
pub struct AiToolPorts {
    pub events: Arc<dyn EventStore>,
    pub identity: Arc<dyn IdentityStore>,
    pub facts: Arc<dyn FactStore>,
    pub sources: Arc<dyn SourceStore>,
    pub proposals: Arc<dyn ProposalStore>,
    /// The documents that the document tools read and that draft proposals target.
    pub documents: Arc<dyn DocumentStore>,
    pub clock: Arc<dyn Clock>,
    /// The workstreams, promisors and work records that proposals name.
    pub workstreams: Arc<dyn WorkstreamStore>,
    pub parties: Arc<dyn PartyStore>,
    pub work: Arc<dyn WorkStore>,
}

/// All tools.
pub fn standard_catalog() -> ToolCatalog<AiToolPorts> {
    ToolCatalog::new(vec![
        Arc::new(Typed(events::ListEvents)),
        Arc::new(Typed(events::GetEventSchema)),
        Arc::new(Typed(events::GetEventProfile)),
        Arc::new(Typed(events::SearchSources)),
        Arc::new(Typed(events::GetSourcePassage)),
        Arc::new(Typed(documents::ListDocuments)),
        Arc::new(Typed(documents::GetDocumentVersion)),
        Arc::new(Typed(work::ListWorkstreams)),
        Arc::new(Typed(work::ListActions)),
        Arc::new(Typed(work::ListCommitments)),
        Arc::new(Typed(work::SearchParties)),
        Arc::new(Typed(propose::ProposeChangeset)),
    ])
}

/// One tool with its input and output types. `Typed` makes an `AiTool` of it.
#[async_trait]
trait TypedTool: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + JsonSchema;

    const NAME: &'static str;
    const READ_ONLY: bool;

    fn description() -> &'static str;

    /// The input schema. Overwrite it for a tool that takes no input.
    fn input_schema() -> Value {
        schema_of::<Self::Input>()
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: Self::Input,
        ports: &AiToolPorts,
    ) -> Result<Self::Output, AiToolError>;
}

/// The `AiTool` of a typed tool: it reads the input from JSON and writes the output as JSON.
struct Typed<T>(T);

#[async_trait]
impl<T: TypedTool> AiTool<AiToolPorts> for Typed<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn description(&self) -> &'static str {
        T::description()
    }

    fn input_schema(&self) -> Value {
        T::input_schema()
    }

    fn output_schema(&self) -> Value {
        schema_of::<T::Output>()
    }

    fn read_only(&self) -> bool {
        T::READ_ONLY
    }

    async fn call(
        &self,
        caller: &AiCaller,
        input: Value,
        ports: &AiToolPorts,
    ) -> Result<Value, AiToolError> {
        // The error of serde can repeat input values, so the agent sees the code only (ADR 0037).
        let input = serde_json::from_value(input)
            .map_err(|_| AiToolError::refusal(ProblemCode::MalformedRequest, Vec::new()))?;
        let output = self.0.run(caller, input, ports).await?;
        serde_json::to_value(output)
            .map_err(|_| AiToolError::refusal(ProblemCode::Internal, Vec::new()))
    }
}

/// The JSON Schema of `T` as MCP wants it: draft 2020-12, without the top-level title and description,
/// which name the Rust type and are noise to the model.
fn schema_of<T: JsonSchema>() -> Value {
    let schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>();
    let mut schema = schema.to_value();
    if let Some(object) = schema.as_object_mut() {
        object.remove("title");
        object.remove("description");
    }
    schema
}

/// The input schema of a tool without input: an object without properties.
fn no_input_schema() -> Value {
    json!({"type": "object", "properties": Map::new()})
}

/// The invalid value `field` with the code `code`.
fn invalid(field: &'static str, code: &'static str) -> AiToolError {
    AiToolError::refusal(
        ProblemCode::ValidationFailed,
        vec![FieldError::new(field, code)],
    )
}

/// The event with the key `key`, if the caller can read it.
async fn find_event(
    caller: &AiCaller,
    key: &str,
    ports: &AiToolPorts,
) -> Result<Event, AiToolError> {
    let key = EventKey::parse(key)
        .map_err(|error| invalid("event_key", event_queries::key_error_code(error)))?;
    Ok(event_queries::find_event(caller, &key, &*ports.events, &*ports.identity).await?)
}

#[cfg(test)]
mod tests;
