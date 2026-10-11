//! The tools of AI (ADR 0010, ADR 0040): one catalog for each transport.
//! The MCP server shows them to the AI clients of members, and an agent run gives them to a model.
//!
//! Each tool runs with an `AiCaller`, so it can call only queries and proposals (ADR 0039).
//! It reads what the member of the caller reads and no more.
//! The names are stable; the descriptions tell the agent the rules.

mod documents;
mod events;
mod propose;
mod work;

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::caller::AiCaller;
use crate::clock::Clock;
use crate::documents::DocumentStore;
use crate::domain::events::{Event, EventKey};
use crate::events::{self as event_queries, EventStore};
use crate::facts::FactStore;
use crate::identity::IdentityStore;
use crate::parties::PartyStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::ProposalStore;
use crate::sources::SourceStore;
use crate::store::StoreError;
use crate::work::WorkStore;
use crate::workstreams::WorkstreamStore;

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

/// One tool of AI. The input and the output are JSON values, so each transport can carry them.
#[async_trait]
pub trait AiTool: Send + Sync {
    /// The stable name, for example `list_events`.
    fn name(&self) -> &'static str;

    /// What the tool does and the rules for the agent.
    fn description(&self) -> &'static str;

    /// The JSON Schema (draft 2020-12) of the input: an object, without a top-level title or description.
    fn input_schema(&self) -> Value;

    /// The JSON Schema (draft 2020-12) of the output, without a top-level title or description.
    fn output_schema(&self) -> Value;

    /// True if the tool only reads. Each other tool proposes: it never changes accepted state.
    fn read_only(&self) -> bool;

    /// Runs the tool with the rights of `caller`. An input that does not match the input schema is `malformed-request`.
    async fn call(
        &self,
        caller: &AiCaller,
        input: Value,
        ports: &AiToolPorts,
    ) -> Result<Value, AiToolError>;
}

/// The refusal or failure of a tool call: the problem code and the invalid values (ADR 0037).
/// It never holds the input, so a transport can show it to the agent.
pub struct AiToolError(Box<dyn CommandError + Send + Sync>);

impl AiToolError {
    /// The invalid value `field` with the code `code`.
    fn invalid(field: &'static str, code: &'static str) -> Self {
        Self::from(Refusal {
            code: ProblemCode::ValidationFailed,
            errors: vec![FieldError::new(field, code)],
        })
    }

    fn of(code: ProblemCode) -> Self {
        Self::from(Refusal {
            code,
            errors: Vec::new(),
        })
    }

    pub fn code(&self) -> ProblemCode {
        self.0.code()
    }

    /// The invalid values of a `validation-failed` error.
    pub fn field_errors(&self) -> &[FieldError] {
        self.0.field_errors()
    }

    /// The store failure, if this is one. The transport logs it; the agent sees the code only.
    pub fn store_error(&self) -> Option<&StoreError> {
        self.0.store_error()
    }
}

impl<E: CommandError + Send + Sync + 'static> From<E> for AiToolError {
    fn from(error: E) -> Self {
        Self(Box::new(error))
    }
}

impl fmt::Debug for AiToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiToolError")
            .field("code", &self.code())
            .field("errors", &self.field_errors())
            .finish_non_exhaustive()
    }
}

/// A refusal that a tool decides itself, for example an input that does not match the schema.
#[derive(Debug)]
struct Refusal {
    code: ProblemCode,
    errors: Vec<FieldError>,
}

impl CommandError for Refusal {
    fn code(&self) -> ProblemCode {
        self.code
    }

    fn field_errors(&self) -> &[FieldError] {
        &self.errors
    }
}

/// The tools that AI can call. Each transport and each agent run takes its tools from here.
#[derive(Clone)]
pub struct ToolCatalog {
    tools: Vec<Arc<dyn AiTool>>,
}

impl ToolCatalog {
    /// All tools.
    pub fn standard() -> Self {
        let tools: Vec<Arc<dyn AiTool>> = vec![
            Arc::new(events::ListEvents),
            Arc::new(events::GetEventSchema),
            Arc::new(events::GetEventProfile),
            Arc::new(events::SearchSources),
            Arc::new(events::GetSourcePassage),
            Arc::new(documents::ListDocuments),
            Arc::new(documents::GetDocumentVersion),
            Arc::new(work::ListWorkstreams),
            Arc::new(work::ListActions),
            Arc::new(work::ListCommitments),
            Arc::new(work::SearchParties),
            Arc::new(propose::ProposeChangeset),
        ];
        Self { tools }
    }

    /// The tools of the catalog, in the order of `standard`.
    pub fn all(&self) -> impl Iterator<Item = &dyn AiTool> {
        self.tools.iter().map(|tool| &**tool)
    }

    /// The tools that only read.
    pub fn read_only(&self) -> impl Iterator<Item = &dyn AiTool> {
        self.all().filter(|tool| tool.read_only())
    }

    /// The tool with the name `name`, if the catalog has it.
    pub fn get(&self, name: &str) -> Option<&dyn AiTool> {
        self.all().find(|tool| tool.name() == name)
    }

    /// A catalog with only the tools `names`.
    ///
    /// # Panics
    ///
    /// If this catalog has no tool with one of the names: the names are fixed in code, so it is a defect.
    #[must_use]
    pub fn only(&self, names: &[&str]) -> Self {
        for name in names {
            assert!(self.get(name).is_some(), "the catalog has no tool {name}");
        }
        let tools = self
            .tools
            .iter()
            .filter(|tool| names.contains(&tool.name()))
            .cloned()
            .collect();
        Self { tools }
    }
}

impl fmt::Debug for ToolCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.all().map(AiTool::name))
            .finish()
    }
}

/// One tool with its input and output types. The catalog makes an `AiTool` of it.
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

#[async_trait]
impl<T: TypedTool> AiTool for T {
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
            .map_err(|_| AiToolError::of(ProblemCode::MalformedRequest))?;
        let output = self.run(caller, input, ports).await?;
        serde_json::to_value(output).map_err(|_| AiToolError::of(ProblemCode::Internal))
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

/// The event with the key `key`, if the caller can read it.
async fn find_event(
    caller: &AiCaller,
    key: &str,
    ports: &AiToolPorts,
) -> Result<Event, AiToolError> {
    let key = EventKey::parse(key)
        .map_err(|error| AiToolError::invalid("event_key", event_queries::key_error_code(error)))?;
    Ok(event_queries::find_event(caller, &key, &*ports.events, &*ports.identity).await?)
}

#[cfg(test)]
mod tests;
