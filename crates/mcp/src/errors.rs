//! The one error contract of all MCP tools (ADR 0037, ADR 0040).
//!
//! A problem that a command or query decides, for example `forbidden` or `validation-failed`, is a tool result with
//! `isError`: clients show tool results to the model, so the agent can correct its call. Only a failure of tada
//! itself (`internal`, `unavailable`) is a JSON-RPC error. The agent sees the problem code and the JSON pointers of
//! the invalid values, never the input itself.

use axum::http::request::Parts;
use rmcp::ErrorData;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResponse, CallToolResult};
use serde_json::{Value, json};
use tada_app::caller::AiCaller;
use tada_app::problem::{CommandError, FieldError, ProblemCode};

/// The error of a tool call.
#[derive(Debug)]
pub(crate) enum ToolError {
    /// The structured content of a tool result with `isError`: the problem code and the invalid values.
    Problem(Value),
    /// A failure of tada itself.
    Fault(ErrorData),
}

impl ToolError {
    /// The problem `code` with the invalid values `errors`.
    pub(crate) fn problem(code: ProblemCode, errors: &[FieldError]) -> Self {
        match code {
            ProblemCode::Internal | ProblemCode::Unavailable => Self::Fault(
                ErrorData::internal_error(code.meaning(), Some(json!({"code": code.as_str()}))),
            ),
            _ => {
                let errors: Vec<_> = errors
                    .iter()
                    .map(|error| json!({"pointer": error.pointer(), "code": error.code}))
                    .collect();
                Self::Problem(json!({"code": code.as_str(), "errors": errors}))
            }
        }
    }

    /// A wiring error of the server. The agent sees no detail.
    pub(crate) fn internal() -> Self {
        Self::Fault(ErrorData::internal_error(
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
        match self {
            Self::Problem(content) => Ok(CallToolResult::structured_error(content).into()),
            Self::Fault(error) => Err(error),
        }
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

#[cfg(test)]
mod tests {
    use rmcp::model::ErrorCode;

    use super::*;

    fn answer(error: ToolError) -> Result<CallToolResult, ErrorData> {
        match error.into_call_tool_result()? {
            CallToolResponse::Complete(result) => Ok(result),
            _ => panic!("not a complete result"),
        }
    }

    #[test]
    fn a_problem_of_the_app_is_a_tool_result_and_a_failure_of_tada_is_a_json_rpc_error() {
        let invalid = FieldError::new("proposals/0/evidence", "evidence-missing");
        let result = answer(ToolError::problem(
            ProblemCode::ValidationFailed,
            &[invalid],
        ))
        .unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content,
            Some(json!({
                "code": "validation-failed",
                "errors": [{"pointer": "/proposals/0/evidence", "code": "evidence-missing"}],
            }))
        );
        for code in [
            ProblemCode::Forbidden,
            ProblemCode::NotFound,
            ProblemCode::MalformedRequest,
        ] {
            let result = answer(ToolError::problem(code, &[])).unwrap();
            assert_eq!(result.is_error, Some(true), "{code:?}");
            assert_eq!(
                result.structured_content,
                Some(json!({"code": code.as_str(), "errors": []}))
            );
        }
        for code in [ProblemCode::Internal, ProblemCode::Unavailable] {
            let error = answer(ToolError::problem(code, &[])).unwrap_err();
            assert_eq!(error.code, ErrorCode::INTERNAL_ERROR, "{code:?}");
            assert_eq!(error.data, Some(json!({"code": code.as_str()})));
        }
        assert_eq!(
            answer(ToolError::internal()).unwrap_err().code,
            ErrorCode::INTERNAL_ERROR
        );
    }
}
