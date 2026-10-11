//! A model that replays recorded responses, for tests and for the evaluation in CI (ADR 0077).

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;
use tada_app::model::{Model, ModelError, ModelRequest, ModelResponse};

use crate::anthropic::parse_response;

/// Answers each call with the next recorded result, and keeps the requests for the assertions of a test.
/// A call after the last result is `ModelError::Invalid`.
#[derive(Debug, Default)]
pub struct FakeModel {
    results: Mutex<VecDeque<Result<ModelResponse, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
}

impl FakeModel {
    pub fn new(results: impl IntoIterator<Item = Result<ModelResponse, ModelError>>) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            requests: Mutex::default(),
        }
    }

    /// Replays bodies of the Messages API, for example the recorded responses of an evaluation case.
    pub fn recorded(bodies: &[Value]) -> Self {
        Self::new(bodies.iter().map(parse_response))
    }

    /// The requests so far, in order.
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl Model for FakeModel {
    async fn call(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        self.results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| Err(ModelError::Invalid("no recorded response left".to_owned())))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tada_app::model::{Block, Effort, Stop};

    use super::*;

    fn request() -> ModelRequest {
        ModelRequest {
            model_id: "claude-haiku-5-5".to_owned(),
            effort: Effort::Low,
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            output_schema: None,
            max_tokens: 100,
        }
    }

    #[tokio::test]
    async fn replays_recorded_bodies_in_order() {
        let model = FakeModel::recorded(&[
            json!({"content": [{"type": "text", "text": "Hallo"}], "stop_reason": "end_turn",
                   "usage": {"input_tokens": 3, "output_tokens": 1}}),
            json!({"content": [], "stop_reason": "refusal", "usage": {"input_tokens": 3, "output_tokens": 0}}),
        ]);
        let first = model.call(&request()).await.unwrap();
        assert_eq!(first.content, [Block::Text("Hallo".to_owned())]);
        assert_eq!(first.stop, Stop::EndTurn);
        assert!(matches!(
            model.call(&request()).await,
            Err(ModelError::Refused { .. })
        ));
        assert!(matches!(
            model.call(&request()).await,
            Err(ModelError::Invalid(_))
        ));
        assert_eq!(model.requests().len(), 3);
    }
}
