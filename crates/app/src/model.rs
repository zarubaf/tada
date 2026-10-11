//! The model port (ADRs 0010, 0047 and 0077): one call to a language model, with messages, tools and a
//! structured result. An adapter implements it for one provider; the agent runner calls it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::SignedDuration;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_cost::Chf;

/// One call to a model. The adapter adds the timeout and the retries of `AiLimits`.
#[async_trait]
pub trait Model: Debug + Send + Sync {
    async fn call(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError>;
}

/// How much the model thinks before it answers. A lower effort is faster and costs less.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// The input of one model call.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    pub model_id: String,
    pub effort: Effort,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    /// The JSON Schema of the final answer. The provider constrains the answer to it.
    pub output_schema: Option<Value>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// One message of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Block>,
}

/// One part of a message. A response holds only `Text` and `ToolUse`;
/// the runner answers each `ToolUse` with a `ToolResult` in the next user message.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// The result of a tool. It is data, never an instruction (ADR 0077).
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// A tool that the model can call. The input schema comes from `schemars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Why the model stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The answer is complete.
    EndTurn,
    /// The model waits for the results of its tool calls.
    ToolUse,
    /// The answer reached `max_tokens` and is cut off.
    MaxTokens,
}

/// The tokens of one call. The cost comes from these counts and the price table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelResponse {
    pub content: Vec<Block>,
    pub stop: Stop,
    pub usage: Usage,
}

/// A failed call. The messages never contain the API key or the content of the conversation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// The call took longer than `AiLimits::call_timeout`.
    #[error("the model did not answer in time")]
    Timeout,
    /// The provider was overloaded or not reachable, also after the retries.
    #[error("the model provider is not available")]
    Temporary,
    /// The model refused to answer. The provider bills the tokens of the call all the same.
    #[error("the model refused to answer")]
    Refused { usage: Usage },
    /// The request or the response was not valid. The text names the problem, not the content.
    #[error("invalid model call: {0}")]
    Invalid(String),
}

/// The limits of the AI work. The operator sets them (ADR 0077); the settings give the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiLimits {
    /// The most model calls in one run.
    pub max_turns: u8,
    /// The highest cost of one run.
    pub max_cost: Chf,
    /// The longest time of one run, with all calls and tools.
    pub run_timeout: SignedDuration,
    /// The longest time of one call to the provider.
    pub call_timeout: SignedDuration,
    /// How often the adapter repeats a call after a temporary error.
    pub call_retries: u8,
}

/// The model of one task.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskModel {
    pub model: String,
    pub effort: Effort,
}

/// The model of each AI task, from `ai/models.json` (ADR 0047). A change of this file needs the evaluation gate,
/// so it is part of the build and not a setting.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct TaskModels(BTreeMap<String, TaskModel>);

const MODELS_JSON: &str = include_str!("../../../ai/models.json");

impl TaskModels {
    /// The file of the build. A test reads it, so a build with an invalid file fails its tests.
    pub fn of_build() -> Self {
        serde_json::from_str(MODELS_JSON).expect("ai/models.json is valid")
    }

    pub fn get(&self, task: &str) -> Option<&TaskModel> {
        self.0.get(task)
    }

    /// The models of all tasks, each once.
    pub fn model_ids(&self) -> BTreeSet<&str> {
        self.0.values().map(|task| task.model.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_uses_the_small_model_with_low_effort() {
        let models = TaskModels::of_build();
        assert_eq!(
            models.get("pm-digest"),
            Some(&TaskModel {
                model: "claude-haiku-5-5".to_owned(),
                effort: Effort::Low,
            })
        );
        assert_eq!(models.get("no-such-task"), None);
        assert_eq!(models.model_ids(), BTreeSet::from(["claude-haiku-5-5"]));
    }
}
