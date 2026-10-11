//! The agent runner (ADR 0077): one loop from a model call to tool calls to tool results, until the model gives
//! its final answer or a limit of the run stops it.
//!
//! The runner is the only creator of an agent caller (ADR 0010): each run acts for its member with the rights of
//! the `read` scope, as the service identity `ai-pm`. It offers the model only the tools of its catalog that
//! read, and it runs a tool only if the tool input stays in the limits of the run, for example the events of a
//! digest. A tool result is data for the model; the runner never reads it as an instruction.
//!
//! Each model call records its usage and cost (`ai_usage`). Each tool call writes one structured log event with
//! the task, the prompt version, the model ID, the tool name and the outcome, and no free text. The run also
//! returns these steps, for the evaluation and the log of the caller.
//!
//! The runner checks the time and the cost limits before and after each model call. The adapter stops each
//! attempt of a call after `AiLimits::call_timeout` and retries it at most `AiLimits::call_retries` times, so a
//! run can end at most one call with its retries after its time limit, and its cost can go above
//! `AiLimits::max_cost` by at most the cost of one call.

use std::fmt;

use jiff::tz::TimeZone;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::ai_cost::{AiCostStore, Chf, UsageRecord};
use crate::ai_tools::{AiTool, AiToolError, ToolCatalog};
use crate::caller::{AiCaller, MemberCaller};
use crate::clock::Clock;
use crate::model::{
    AiLimits, Block, Message, Model, ModelError, ModelRequest, Role, Stop, TaskModels, ToolSpec,
    Usage,
};
use crate::problem::ProblemCode;
use crate::store::StoreError;

/// An AI task that an agent runs: its name in `ai/models.json`, its prompt and the size of an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentTask {
    /// The name, for example `pm-digest`. The usage table and the audit actor record it.
    pub name: &'static str,
    /// The `n` of the prompt file `ai/prompts/{name}/v{n}.md`.
    pub prompt_version: u16,
    /// The text of the prompt file.
    pub system: &'static str,
    /// The most tokens of one answer.
    pub max_tokens: u32,
}

/// The ports of a run. `tools` reach their data through `tool_ports`.
pub struct AgentPorts<'a, P: ?Sized> {
    pub model: &'a dyn Model,
    pub tools: &'a ToolCatalog<P>,
    pub tool_ports: &'a P,
    pub cost: &'a dyn AiCostStore,
    pub clock: &'a dyn Clock,
    pub limits: AiLimits,
}

impl<P: ?Sized> fmt::Debug for AgentPorts<'_, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentPorts")
            .field("tools", self.tools)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// Why a run gave no answer. The messages hold no text of the model and no data of the input.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The model wanted more tool calls after the last turn of `AiLimits::max_turns`.
    #[error("the run reached its limit of model calls")]
    Turns,
    /// The cost of the run went above `AiLimits::max_cost`.
    #[error("the run reached its cost limit")]
    Cost,
    /// The run took longer than `AiLimits::run_timeout`.
    #[error("the run reached its time limit")]
    Timeout,
    /// `ai/models.json` names no model for the task.
    #[error("no model is set for the task")]
    NoModel,
    /// The price table has no price for the model, so the cost of a call is not known.
    #[error("the price table has no price for the model")]
    NoPrice,
    #[error(transparent)]
    Model(ModelError),
    /// The final answer is not valid. The text names the problem, not the answer.
    #[error("the answer of the model is not valid: {0}")]
    Output(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// What happened to one tool call of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOutcome {
    /// The tool ran and gave its result to the model.
    Done,
    /// The tool refused the call or failed, with this problem code.
    Failed(ProblemCode),
    /// The input was outside the limits of the run. The tool did not run.
    OutsideLimits,
}

impl ToolOutcome {
    /// `ok`, `outside-limits` or the problem code of the failure.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "ok",
            Self::Failed(code) => code.as_str(),
            Self::OutsideLimits => "outside-limits",
        }
    }
}

/// One step of a run. It holds no free text: a tool name is the name of the catalog, never the text of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStep {
    /// A model call with its tokens and its cost.
    Model { usage: Usage, cost: Chf },
    /// A call of the offered tool `tool`.
    Tool {
        tool: &'static str,
        outcome: ToolOutcome,
    },
    /// A call of a tool that the run did not offer, for example a tool that proposes. Nothing ran.
    NotOffered,
}

/// The result of a run and its steps.
#[derive(Debug)]
pub struct AgentRun<T> {
    pub task: &'static str,
    pub prompt_version: u16,
    /// Empty if `ai/models.json` names no model for the task.
    pub model_id: String,
    pub steps: Vec<AgentStep>,
    /// The cost of all model calls of the run.
    pub cost: Chf,
    pub result: Result<T, AgentError>,
}

/// The permission to create an agent caller (`AiCaller::for_agent`). Its field is private, so only this module
/// creates one, and only the runner creates agent callers.
#[derive(Debug)]
pub(crate) struct AgentGrant(());

/// Decides if a tool input stays in the limits of a run.
pub type InLimits<'a> = &'a (dyn Fn(&'static str, &Value) -> bool + Sync);

/// Runs `task` for `principal` on `input` and returns the final answer as `T`.
///
/// The model gets the read tools of `ports.tools`. A tool call runs only if `in_limits` accepts its input.
/// The run stops at the first limit of `ports.limits`.
pub async fn run_agent<T, P>(
    ports: &AgentPorts<'_, P>,
    principal: MemberCaller,
    task: &AgentTask,
    input: String,
    in_limits: InLimits<'_>,
) -> AgentRun<T>
where
    T: DeserializeOwned + JsonSchema,
    P: ?Sized + Sync,
{
    let mut trace = Trace::default();
    let result = run_loop(ports, principal, task, input, in_limits, &mut trace).await;
    AgentRun {
        task: task.name,
        prompt_version: task.prompt_version,
        model_id: trace.model_id,
        steps: trace.steps,
        cost: trace.cost,
        result,
    }
}

/// What `run_loop` writes while it runs: the model ID, the steps and the cost.
#[derive(Default)]
struct Trace {
    model_id: String,
    steps: Vec<AgentStep>,
    cost: Chf,
}

/// The loop of `run_agent`.
async fn run_loop<T, P>(
    ports: &AgentPorts<'_, P>,
    principal: MemberCaller,
    task: &AgentTask,
    input: String,
    in_limits: InLimits<'_>,
    run: &mut Trace,
) -> Result<T, AgentError>
where
    T: DeserializeOwned + JsonSchema,
    P: ?Sized + Sync,
{
    let models = TaskModels::of_build();
    let task_model = models.get(task.name).ok_or(AgentError::NoModel)?;
    run.model_id.clone_from(&task_model.model);
    let scope = principal.scope();
    let caller = AiCaller::for_agent(AgentGrant(()), principal, task.name);
    let start = ports.clock.now();
    let deadline = start
        .saturating_add(ports.limits.run_timeout)
        .map_err(|_| AgentError::Timeout)?;
    let price = ports
        .cost
        .price(
            scope,
            &task_model.model,
            start.to_zoned(TimeZone::UTC).date(),
        )
        .await?
        .ok_or(AgentError::NoPrice)?;

    // Only the tools that read: a tool that proposes is never offered, also if the catalog has it.
    let offered: Vec<&std::sync::Arc<dyn AiTool<P>>> = ports.tools.read_only().collect();
    let mut request = ModelRequest {
        model_id: task_model.model.clone(),
        effort: task_model.effort,
        system: task.system.to_owned(),
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text(input)],
        }],
        tools: offered
            .iter()
            .map(|tool| ToolSpec {
                name: tool.name().to_owned(),
                description: tool.description().to_owned(),
                input_schema: tool.input_schema(),
            })
            .collect(),
        output_schema: Some(schema_of::<T>()),
        max_tokens: task.max_tokens,
    };

    for _ in 0..ports.limits.max_turns {
        if ports.clock.now() >= deadline {
            return Err(AgentError::Timeout);
        }
        let response = ports.model.call(&request).await;
        let usage = match &response {
            Ok(response) => Some(response.usage),
            Err(ModelError::Refused { usage }) => Some(*usage),
            Err(_) => None,
        };
        if let Some(usage) = usage {
            let cost = price.cost(usage);
            run.steps.push(AgentStep::Model { usage, cost });
            let record = UsageRecord {
                event_id: None,
                task: task.name,
                prompt_version: task.prompt_version,
                model_id: task_model.model.clone(),
                usage,
                cost,
                at: ports.clock.now(),
            };
            ports.cost.record(scope, &record).await?;
            run.cost = run.cost.checked_add(cost).ok_or(AgentError::Cost)?;
        }
        let response = response.map_err(AgentError::Model)?;
        if run.cost > ports.limits.max_cost {
            return Err(AgentError::Cost);
        }
        if ports.clock.now() >= deadline {
            return Err(AgentError::Timeout);
        }
        match response.stop {
            Stop::EndTurn => return final_answer(&response.content),
            Stop::MaxTokens => {
                return Err(AgentError::Output("the answer was cut off".to_owned()));
            }
            Stop::ToolUse => {}
        }
        let mut results = Vec::new();
        for block in &response.content {
            let Block::ToolUse { id, name, input } = block else {
                continue;
            };
            let (step, content, is_error) =
                call_tool(&offered, &caller, name, input, in_limits, ports.tool_ports).await;
            log_tool_step(task, &task_model.model, step);
            run.steps.push(step);
            results.push(Block::ToolResult {
                tool_use_id: id.clone(),
                content,
                is_error,
            });
        }
        if results.is_empty() {
            return Err(AgentError::Output(
                "the model waits for a tool but called none".to_owned(),
            ));
        }
        request.messages.push(Message {
            role: Role::Assistant,
            content: response.content,
        });
        request.messages.push(Message {
            role: Role::User,
            content: results,
        });
    }
    Err(AgentError::Turns)
}

/// Runs one tool call of the model, if the run offered the tool and the input is in the limits.
/// Returns the step, the content of the tool result and whether it is an error.
async fn call_tool<P: ?Sized + Sync>(
    offered: &[&std::sync::Arc<dyn AiTool<P>>],
    caller: &AiCaller,
    name: &str,
    input: &Value,
    in_limits: InLimits<'_>,
    tool_ports: &P,
) -> (AgentStep, String, bool) {
    let forbidden = || json!({"error": ProblemCode::Forbidden.as_str()}).to_string();
    let Some(tool) = offered.iter().find(|tool| tool.name() == name) else {
        return (AgentStep::NotOffered, forbidden(), true);
    };
    let tool_name = tool.name();
    if !in_limits(tool_name, input) {
        let step = AgentStep::Tool {
            tool: tool_name,
            outcome: ToolOutcome::OutsideLimits,
        };
        return (step, forbidden(), true);
    }
    match tool.call(caller, input.clone(), tool_ports).await {
        Ok(output) => {
            let step = AgentStep::Tool {
                tool: tool_name,
                outcome: ToolOutcome::Done,
            };
            (step, output.to_string(), false)
        }
        Err(error) => {
            let step = AgentStep::Tool {
                tool: tool_name,
                outcome: ToolOutcome::Failed(error.code()),
            };
            (step, tool_error(&error).to_string(), true)
        }
    }
}

/// One structured log event for each tool call (ADR 0077): the task, the prompt version, the model ID, the tool
/// and the outcome. It holds no input and no result of the tool, and no text of the model.
fn log_tool_step(task: &AgentTask, model_id: &str, step: AgentStep) {
    let (tool, outcome) = match step {
        AgentStep::Tool { tool, outcome } => (tool, outcome.as_str()),
        AgentStep::NotOffered | AgentStep::Model { .. } => ("", "not-offered"),
    };
    tracing::info!(
        task = task.name,
        prompt_version = task.prompt_version,
        model_id,
        tool,
        outcome,
        "an agent run called a tool"
    );
}

/// The result of a failed tool call for the model: the problem code and the invalid fields, as MCP shows them.
fn tool_error(error: &AiToolError) -> Value {
    let fields: Vec<Value> = error
        .field_errors()
        .iter()
        .map(|field| json!({"field": field.field, "code": field.code}))
        .collect();
    if fields.is_empty() {
        json!({"error": error.code().as_str()})
    } else {
        json!({"error": error.code().as_str(), "fields": fields})
    }
}

/// The final answer: the text of the response as JSON of `T`. An error never holds the text.
fn final_answer<T: DeserializeOwned>(content: &[Block]) -> Result<T, AgentError> {
    let text: String = content
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    serde_json::from_str(&text)
        .map_err(|_| AgentError::Output("the answer does not match its schema".to_owned()))
}

/// The JSON Schema (draft 2020-12) of the final answer, without the top-level title, which names the Rust type.
fn schema_of<T: JsonSchema>() -> Value {
    let mut schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value();
    if let Some(object) = schema.as_object_mut() {
        object.remove("title");
        object.remove("description");
    }
    schema
}

#[cfg(test)]
mod tests;
