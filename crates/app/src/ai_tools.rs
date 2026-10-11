//! The contract of the tools of AI (ADR 0010, ADR 0040): one catalog for each transport.
//! The MCP server shows the tools to the AI clients of members, and an agent run gives them to a model.
//!
//! The tools themselves live in the crate `ai-tools`. It uses only the public interface of `app`,
//! so the compiler keeps each tool to queries and proposals with an `AiCaller` (ADR 0039).

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::caller::AiCaller;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// One tool of AI. The input and the output are JSON values, so each transport can carry them.
/// `P` holds the ports that the tool reads and proposes through.
#[async_trait]
pub trait AiTool<P: ?Sized>: Send + Sync {
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
    async fn call(&self, caller: &AiCaller, input: Value, ports: &P) -> Result<Value, AiToolError>;
}

/// The refusal or failure of a tool call: the problem code and the invalid values (ADR 0037).
/// It never holds the input, so a transport can show it to the agent.
pub struct AiToolError(Box<dyn CommandError + Send + Sync>);

impl AiToolError {
    /// A refusal that a tool decides itself, for example an input that does not match the schema.
    pub fn refusal(code: ProblemCode, errors: Vec<FieldError>) -> Self {
        Self::from(Refusal { code, errors })
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

/// The tools that AI can call with the ports `P`. Each transport and each agent run takes its tools from one catalog.
pub struct ToolCatalog<P: ?Sized> {
    tools: Vec<Arc<dyn AiTool<P>>>,
}

impl<P: ?Sized> ToolCatalog<P> {
    /// A catalog of `tools`.
    ///
    /// # Panics
    ///
    /// If two tools have the same name: the names are fixed in code, so it is a defect.
    pub fn new(tools: Vec<Arc<dyn AiTool<P>>>) -> Self {
        for (index, tool) in tools.iter().enumerate() {
            let name = tool.name();
            assert!(
                tools[..index].iter().all(|other| other.name() != name),
                "the catalog has two tools {name}"
            );
        }
        Self { tools }
    }

    /// The tools of the catalog, in the order of `new`.
    pub fn all(&self) -> impl Iterator<Item = &Arc<dyn AiTool<P>>> {
        self.tools.iter()
    }

    /// The tools that only read.
    pub fn read_only(&self) -> impl Iterator<Item = &Arc<dyn AiTool<P>>> {
        self.all().filter(|tool| tool.read_only())
    }

    /// The tool with the name `name`, if the catalog has it.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn AiTool<P>>> {
        self.all().find(|tool| tool.name() == name)
    }

    /// A catalog with only the tools `names`, in the order of this catalog, not of `names`.
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
            .all()
            .filter(|tool| names.contains(&tool.name()))
            .cloned()
            .collect();
        Self { tools }
    }
}

impl<P: ?Sized> Clone for ToolCatalog<P> {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
        }
    }
}

impl<P: ?Sized> fmt::Debug for ToolCatalog<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.all().map(|tool| tool.name()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A tool that answers its name. It has no ports.
    struct Named(&'static str, bool);

    #[async_trait]
    impl AiTool<()> for Named {
        fn name(&self) -> &'static str {
            self.0
        }

        fn description(&self) -> &'static str {
            "A test tool."
        }

        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }

        fn output_schema(&self) -> Value {
            json!({"type": "string"})
        }

        fn read_only(&self) -> bool {
            self.1
        }

        async fn call(&self, _: &AiCaller, _: Value, _: &()) -> Result<Value, AiToolError> {
            Ok(json!(self.0))
        }
    }

    fn catalog() -> ToolCatalog<()> {
        ToolCatalog::new(vec![
            Arc::new(Named("read_a", true)),
            Arc::new(Named("propose", false)),
            Arc::new(Named("read_b", true)),
        ])
    }

    fn names<'a>(tools: impl Iterator<Item = &'a Arc<dyn AiTool<()>>>) -> Vec<&'static str> {
        tools.map(|tool| tool.name()).collect()
    }

    #[test]
    fn read_only_leaves_out_each_tool_that_proposes() {
        assert_eq!(names(catalog().read_only()), ["read_a", "read_b"]);
    }

    #[test]
    fn only_keeps_the_named_tools_in_the_order_of_the_catalog() {
        let only = catalog().only(&["read_b", "read_a"]);
        assert_eq!(names(only.all()), ["read_a", "read_b"]);
        assert!(only.get("propose").is_none());
        assert_eq!(only.get("read_b").map(|tool| tool.name()), Some("read_b"));
    }

    #[test]
    #[should_panic(expected = "the catalog has no tool list_everything")]
    fn only_rejects_an_unknown_name() {
        let _ = catalog().only(&["list_everything"]);
    }

    #[test]
    #[should_panic(expected = "the catalog has two tools read_a")]
    fn a_catalog_rejects_two_tools_with_one_name() {
        let _ = ToolCatalog::<()>::new(vec![
            Arc::new(Named("read_a", true)),
            Arc::new(Named("read_a", true)),
        ]);
    }

    #[test]
    fn an_error_keeps_its_code_its_invalid_values_and_its_store_failure() {
        let error = AiToolError::refusal(
            ProblemCode::ValidationFailed,
            vec![FieldError::new("event_key", "pattern")],
        );
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
        assert_eq!(
            error.field_errors(),
            [FieldError::new("event_key", "pattern")]
        );
        assert!(error.store_error().is_none());
        let error = AiToolError::from(StoreError::Unavailable("down".into()));
        assert_eq!(error.code(), ProblemCode::Unavailable);
        assert!(error.store_error().is_some());
    }
}
