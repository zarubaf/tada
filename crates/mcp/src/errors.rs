//! The one error contract of all MCP tools (ADR 0037, ADR 0040).
//! The agent sees the problem code and the JSON pointers of the invalid values, never the input itself.

use axum::http::request::Parts;
use rmcp::ErrorData;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::CallToolResponse;
use serde_json::json;
use tada_app::caller::AiCaller;
use tada_app::problem::{CommandError, FieldError, ProblemCode};

/// The error of a tool call.
#[derive(Debug)]
pub(crate) struct ToolError(ErrorData);

impl ToolError {
    /// The problem `code` with the invalid values `errors`.
    pub(crate) fn problem(code: ProblemCode, errors: &[FieldError]) -> Self {
        let errors: Vec<_> = errors
            .iter()
            .map(|error| json!({"pointer": error.pointer(), "code": error.code}))
            .collect();
        Self(match code {
            ProblemCode::ValidationFailed => ErrorData::invalid_params(
                code.meaning(),
                Some(json!({"code": code.as_str(), "errors": errors})),
            ),
            ProblemCode::NotFound => {
                ErrorData::resource_not_found(code.meaning(), Some(json!({"code": code.as_str()})))
            }
            _ => ErrorData::internal_error(code.meaning(), Some(json!({"code": code.as_str()}))),
        })
    }

    /// A wiring error of the server. The agent sees no detail.
    pub(crate) fn internal() -> Self {
        Self(ErrorData::internal_error(
            ProblemCode::Internal.meaning(),
            None,
        ))
    }
}

/// The error of an `app` command or query. A store failure goes to the log; the agent sees the code only.
impl<E: CommandError> From<E> for ToolError {
    fn from(error: E) -> Self {
        if let Some(store_error) = error.store_error() {
            tracing::error!(error = %crate::guard::error_chain(store_error), "the store failed");
        }
        Self::problem(error.code(), error.field_errors())
    }
}

impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        Err(self.0)
    }
}

/// The AI caller that the guard found for the request.
pub(crate) fn caller(parts: &Parts) -> Result<&AiCaller, ToolError> {
    // The guard runs before each request, so a missing caller is a wiring error: fail closed.
    parts
        .extensions
        .get::<AiCaller>()
        .ok_or_else(ToolError::internal)
}
