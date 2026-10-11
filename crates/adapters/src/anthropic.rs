//! The model adapter for the Anthropic Messages API (ADRs 0047 and 0077).
//!
//! A thin client on `reqwest`, without an SDK. Tools are strict, the final answer uses structured output,
//! and the adapter never forces a tool choice. Each call has a timeout; a 429, a 5xx, an overload or a
//! connection error is repeated with a backoff. The API key goes only into the request header: it is never
//! in a log, an error or a `Debug` output.

use std::time::Duration;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Map, Value, json};
use tada_app::model::{
    Block, Model, ModelError, ModelRequest, ModelResponse, Role, Stop, ToolSpec, Usage,
};

/// The API version that the adapter speaks.
const API_VERSION: &str = "2023-06-01";

/// The connection settings of the adapter.
#[derive(Debug)]
pub struct AnthropicConfig {
    /// The base URL without `/v1`, for example `https://api.anthropic.com`.
    pub api_url: String,
    pub api_key: SecretString,
    /// The longest time of one attempt.
    pub call_timeout: Duration,
    /// How often a temporary error is repeated.
    pub retries: u8,
    /// The wait before the first repetition. It doubles for each further one.
    pub backoff: Duration,
}

/// The wait before the first repetition of a call in production.
pub const BACKOFF: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct AnthropicModel {
    client: reqwest::Client,
    messages_url: String,
    api_key: SecretString,
    call_timeout: Duration,
    retries: u8,
    backoff: Duration,
}

impl AnthropicModel {
    pub fn new(config: AnthropicConfig) -> Result<Self, ModelError> {
        // No redirects: `reqwest` would send the custom header `x-api-key` to the new host too.
        let client = reqwest::Client::builder()
            .timeout(config.call_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ModelError::Invalid("cannot build the HTTP client".to_owned()))?;
        Ok(Self {
            client,
            messages_url: format!("{}/v1/messages", config.api_url.trim_end_matches('/')),
            api_key: config.api_key,
            call_timeout: config.call_timeout,
            retries: config.retries,
            backoff: config.backoff,
        })
    }

    /// One attempt. `Temporary` is the only error that the caller repeats.
    async fn attempt(&self, body: &Value) -> Result<ModelResponse, ModelError> {
        let response = self
            .client
            .post(&self.messages_url)
            .timeout(self.call_timeout)
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", API_VERSION)
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status();
        // 529 is the overload answer of the API. Its body is not always JSON.
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(ModelError::Temporary);
        }
        if status.is_redirection() {
            return Err(ModelError::Invalid(format!(
                "the API answered {} with a redirect, which the adapter does not follow",
                status.as_u16()
            )));
        }
        if !status.is_success() {
            // Only the error type: the message of the API can repeat the input.
            let body: Value = response.json().await.unwrap_or_default();
            let kind = body["error"]["type"].as_str().unwrap_or("unknown");
            return Err(ModelError::Invalid(format!(
                "the API answered {}: {kind}",
                status.as_u16()
            )));
        }
        let body: Value = response.json().await.map_err(transport_error)?;
        parse_response(&body)
    }
}

/// The error of a request that got no complete answer. The text of a `reqwest` error can hold the URL,
/// so it does not go into the result.
/// Only an error before the API got the request is temporary. An error while the body of an answer comes in
/// is not repeated: the API can have done and billed the call already.
fn transport_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else if error.is_connect() {
        ModelError::Temporary
    } else {
        ModelError::Invalid("the answer of the API could not be read".to_owned())
    }
}

#[async_trait]
impl Model for AnthropicModel {
    async fn call(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let body = request_body(request);
        let mut wait = self.backoff;
        let mut retries = self.retries;
        loop {
            match self.attempt(&body).await {
                Err(ModelError::Temporary) if retries > 0 => {
                    tracing::warn!(
                        retries_left = retries,
                        "the model provider is not available; the call repeats"
                    );
                    retries -= 1;
                    tokio::time::sleep(wait).await;
                    wait = wait.saturating_mul(2);
                }
                result => return result,
            }
        }
    }
}

/// The JSON body of a request.
fn request_body(request: &ModelRequest) -> Value {
    let mut body = Map::new();
    body.insert("model".to_owned(), request.model_id.clone().into());
    body.insert("max_tokens".to_owned(), request.max_tokens.into());
    body.insert("system".to_owned(), request.system.clone().into());
    body.insert(
        "messages".to_owned(),
        request.messages.iter().map(message).collect(),
    );
    if !request.tools.is_empty() {
        body.insert("tools".to_owned(), request.tools.iter().map(tool).collect());
        // A forced tool choice fails with some features of current models (ADR 0047).
        body.insert("tool_choice".to_owned(), json!({"type": "auto"}));
    }
    let mut output = Map::new();
    output.insert("effort".to_owned(), request.effort.as_str().into());
    if let Some(schema) = &request.output_schema {
        output.insert(
            "format".to_owned(),
            json!({"type": "json_schema", "schema": strict(schema.clone())}),
        );
    }
    body.insert("output_config".to_owned(), Value::Object(output));
    Value::Object(body)
}

fn message(message: &tada_app::model::Message) -> Value {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let content: Vec<Value> = message
        .content
        .iter()
        .map(|block| match block {
            Block::Text(text) => json!({"type": "text", "text": text}),
            Block::ToolUse { id, name, input } => {
                json!({"type": "tool_use", "id": id, "name": name, "input": input})
            }
            Block::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": content,
                "is_error": is_error,
            }),
        })
        .collect();
    json!({"role": role, "content": content})
}

fn tool(tool: &ToolSpec) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": strict(tool.input_schema.clone()),
        "strict": true,
    })
}

/// The schema with `additionalProperties: false` on each object that does not set it.
/// Strict tools and structured output require it.
fn strict(mut schema: Value) -> Value {
    fn visit(value: &mut Value) {
        match value {
            Value::Object(object) => {
                let is_object = match object.get("type") {
                    Some(Value::String(kind)) => kind == "object",
                    Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "object"),
                    _ => false,
                };
                if is_object {
                    object
                        .entry("additionalProperties")
                        .or_insert(Value::Bool(false));
                }
                object.values_mut().for_each(visit);
            }
            Value::Array(items) => items.iter_mut().for_each(visit),
            _ => {}
        }
    }
    visit(&mut schema);
    schema
}

/// Reads a response body of the Messages API. The fake adapter replays recorded bodies with it.
pub fn parse_response(body: &Value) -> Result<ModelResponse, ModelError> {
    let invalid = |what: &str| ModelError::Invalid(format!("the response has no valid {what}"));
    let tokens = |name: &str| {
        body["usage"][name]
            .as_u64()
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(|| invalid("usage"))
    };
    let usage = Usage {
        input_tokens: tokens("input_tokens")?,
        output_tokens: tokens("output_tokens")?,
    };
    let stop = match body["stop_reason"].as_str() {
        Some("end_turn" | "stop_sequence") => Stop::EndTurn,
        Some("tool_use") => Stop::ToolUse,
        Some("max_tokens") => Stop::MaxTokens,
        Some("refusal") => return Err(ModelError::Refused { usage }),
        _ => return Err(invalid("stop_reason")),
    };
    let blocks = body["content"]
        .as_array()
        .ok_or_else(|| invalid("content"))?;
    let mut content = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => content.push(Block::Text(
                block["text"]
                    .as_str()
                    .ok_or_else(|| invalid("text"))?
                    .to_owned(),
            )),
            Some("tool_use") => content.push(Block::ToolUse {
                id: block["id"]
                    .as_str()
                    .ok_or_else(|| invalid("tool_use"))?
                    .to_owned(),
                name: block["name"]
                    .as_str()
                    .ok_or_else(|| invalid("tool_use"))?
                    .to_owned(),
                input: block["input"].clone(),
            }),
            // Other blocks, for example `thinking`, carry nothing that the runner uses.
            _ => {}
        }
    }
    Ok(ModelResponse {
        content,
        stop,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use tada_app::model::{Effort, Message};
    use tokio::net::TcpListener;

    use super::*;

    const KEY: &str = "sk-test-not-a-real-key";

    /// One answer of the fake API: a status, a body and a delay.
    #[derive(Clone)]
    struct Answer {
        status: u16,
        body: Value,
        delay: Duration,
    }

    fn ok(body: Value) -> Answer {
        Answer {
            status: 200,
            body,
            delay: Duration::ZERO,
        }
    }

    fn status(status: u16) -> Answer {
        Answer {
            status,
            body: json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
            delay: Duration::ZERO,
        }
    }

    #[derive(Default)]
    struct Fake {
        answers: Mutex<VecDeque<Answer>>,
        requests: Mutex<Vec<(HeaderMap, Value)>>,
    }

    async fn messages(
        State(fake): State<Arc<Fake>>,
        headers: HeaderMap,
        axum::Json(body): axum::Json<Value>,
    ) -> (StatusCode, axum::Json<Value>) {
        fake.requests.lock().unwrap().push((headers, body));
        let answer = fake.answers.lock().unwrap().pop_front().unwrap();
        tokio::time::sleep(answer.delay).await;
        (
            StatusCode::from_u16(answer.status).unwrap(),
            axum::Json(answer.body),
        )
    }

    /// A local fake of the Messages API. The server task ends with the test runtime.
    async fn fake_api(answers: Vec<Answer>) -> (Arc<Fake>, String) {
        let fake = Arc::new(Fake {
            answers: Mutex::new(answers.into()),
            requests: Mutex::default(),
        });
        let router = Router::new()
            .route("/v1/messages", post(messages))
            .with_state(fake.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await });
        (fake, url)
    }

    fn model(url: &str, call_timeout: Duration) -> AnthropicModel {
        AnthropicModel::new(AnthropicConfig {
            api_url: url.to_owned(),
            api_key: SecretString::from(KEY),
            call_timeout,
            retries: 2,
            backoff: Duration::from_millis(1),
        })
        .unwrap()
    }

    fn request() -> ModelRequest {
        ModelRequest {
            model_id: "claude-haiku-5-5".to_owned(),
            effort: Effort::Low,
            system: "You write a short digest.".to_owned(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![Block::Text("Two items.".to_owned())],
                },
                Message {
                    role: Role::Assistant,
                    content: vec![Block::ToolUse {
                        id: "toolu_1".to_owned(),
                        name: "list_actions".to_owned(),
                        input: json!({"event": "FLY28"}),
                    }],
                },
                Message {
                    role: Role::User,
                    content: vec![Block::ToolResult {
                        tool_use_id: "toolu_1".to_owned(),
                        content: "[]".to_owned(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![ToolSpec {
                name: "list_actions".to_owned(),
                description: "Lists the actions of an event.".to_owned(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "event": {"type": "string"},
                        "filter": {"type": ["object", "null"], "properties": {"status": {"type": "string"}}}
                    },
                    "required": ["event"]
                }),
            }],
            output_schema: Some(json!({
                "type": "object",
                "properties": {"intro": {"type": "string"}},
                "required": ["intro"]
            })),
            max_tokens: 1024,
        }
    }

    fn tool_use_answer() -> Value {
        json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "model": "claude-haiku-5-5",
            "content": [
                {"type": "text", "text": "I check the event."},
                {"type": "tool_use", "id": "toolu_2", "name": "list_actions", "input": {"event": "FLY28"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 120, "output_tokens": 30}
        })
    }

    fn end_answer() -> Value {
        json!({
            "content": [{"type": "text", "text": "{\"intro\":\"Hallo\"}"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        })
    }

    #[tokio::test]
    async fn anthropic_request_shape() {
        let (fake, url) = fake_api(vec![ok(tool_use_answer())]).await;
        let response = model(&url, Duration::from_secs(5))
            .call(&request())
            .await
            .unwrap();

        assert_eq!(
            response,
            ModelResponse {
                content: vec![
                    Block::Text("I check the event.".to_owned()),
                    Block::ToolUse {
                        id: "toolu_2".to_owned(),
                        name: "list_actions".to_owned(),
                        input: json!({"event": "FLY28"}),
                    },
                ],
                stop: Stop::ToolUse,
                usage: Usage {
                    input_tokens: 120,
                    output_tokens: 30,
                },
            }
        );

        let requests = fake.requests.lock().unwrap();
        let (headers, body) = &requests[0];
        assert_eq!(headers["x-api-key"], KEY);
        assert_eq!(headers["anthropic-version"], "2023-06-01");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["system"], "You write a short digest.");
        assert_eq!(
            body["messages"],
            json!([
                {"role": "user", "content": [{"type": "text", "text": "Two items."}]},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_1", "name": "list_actions", "input": {"event": "FLY28"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": "[]", "is_error": false}
                ]}
            ])
        );
        // Strict tools: each object of the schema rejects other properties.
        let tool = &body["tools"][0];
        assert_eq!(tool["name"], "list_actions");
        assert_eq!(tool["strict"], true);
        assert_eq!(tool["input_schema"]["additionalProperties"], false);
        assert_eq!(
            tool["input_schema"]["properties"]["filter"]["additionalProperties"],
            false
        );
        // No forced tool choice: current models reject it with some features (ADR 0047).
        assert_eq!(body["tool_choice"], json!({"type": "auto"}));
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            body["output_config"]["format"]["schema"]["additionalProperties"],
            false
        );
        for absent in ["output_format", "thinking"] {
            assert!(body.get(absent).is_none(), "{absent}");
        }
    }

    #[tokio::test]
    async fn a_request_without_tools_and_schema_sends_neither() {
        let (fake, url) = fake_api(vec![ok(end_answer())]).await;
        let plain = ModelRequest {
            tools: Vec::new(),
            output_schema: None,
            ..request()
        };
        let response = model(&url, Duration::from_secs(5))
            .call(&plain)
            .await
            .unwrap();
        assert_eq!(response.stop, Stop::EndTurn);
        let body = &fake.requests.lock().unwrap()[0].1;
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert_eq!(body["output_config"], json!({"effort": "low"}));
    }

    #[tokio::test]
    async fn retries_only_temporary() {
        // 429 and 529 are repeated; the third attempt succeeds.
        let (fake, url) = fake_api(vec![status(429), status(529), ok(end_answer())]).await;
        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert_eq!(response.unwrap().stop, Stop::EndTurn);
        assert_eq!(fake.requests.lock().unwrap().len(), 3);

        // A 5xx stops after two retries.
        let (fake, url) = fake_api(vec![
            status(500),
            status(502),
            status(503),
            ok(end_answer()),
        ])
        .await;
        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert_eq!(response, Err(ModelError::Temporary));
        assert_eq!(fake.requests.lock().unwrap().len(), 3);

        // A 400 is not repeated.
        let invalid = Answer {
            status: 400,
            body: json!({"type": "error", "error": {"type": "invalid_request_error", "message": "bad"}}),
            delay: Duration::ZERO,
        };
        let (fake, url) = fake_api(vec![invalid, ok(end_answer())]).await;
        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert!(
            matches!(response, Err(ModelError::Invalid(_))),
            "{response:?}"
        );
        assert_eq!(fake.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_redirect_is_an_error_and_the_key_stays_with_the_api() {
        // The other host records each request that reaches it.
        let (other, other_url) = fake_api(vec![ok(end_answer())]).await;
        let location = format!("{other_url}/v1/messages");
        let router = Router::new().route(
            "/v1/messages",
            post(move || {
                let location = location.clone();
                async move { (StatusCode::TEMPORARY_REDIRECT, [("location", location)]) }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await });

        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert!(
            matches!(&response, Err(ModelError::Invalid(text)) if text.contains("307")),
            "{response:?}"
        );
        assert!(other.requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_connection_error_is_temporary() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert_eq!(response, Err(ModelError::Temporary));
    }

    #[tokio::test]
    async fn call_timeout() {
        let slow = Answer {
            delay: Duration::from_secs(5),
            ..ok(end_answer())
        };
        let (fake, url) = fake_api(vec![slow, ok(end_answer())]).await;
        let response = model(&url, Duration::from_millis(200))
            .call(&request())
            .await;
        assert_eq!(response, Err(ModelError::Timeout));
        // A timeout is not repeated: the run has its own time limit.
        assert_eq!(fake.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_refusal_is_an_error() {
        let refusal = json!({
            "content": [],
            "stop_reason": "refusal",
            "usage": {"input_tokens": 10, "output_tokens": 0}
        });
        let (_, url) = fake_api(vec![ok(refusal)]).await;
        let response = model(&url, Duration::from_secs(5)).call(&request()).await;
        assert_eq!(
            response,
            Err(ModelError::Refused {
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 0,
                },
            })
        );
    }

    #[tokio::test]
    async fn the_key_is_in_no_error_and_no_debug_output() {
        let unauthorized = Answer {
            status: 401,
            body: json!({"type": "error", "error": {"type": "authentication_error", "message": KEY}}),
            delay: Duration::ZERO,
        };
        let (_, url) = fake_api(vec![unauthorized]).await;
        let model = model(&url, Duration::from_secs(5));
        let error = model.call(&request()).await.unwrap_err();
        assert!(!format!("{error} {error:?}").contains(KEY), "{error:?}");
        assert!(!format!("{model:?}").contains(KEY));
    }

    #[test]
    fn parses_a_max_tokens_stop_and_skips_unknown_blocks() {
        let body = json!({
            "content": [{"type": "thinking", "thinking": "..."}, {"type": "text", "text": "Hal"}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 1, "output_tokens": 2}
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.stop, Stop::MaxTokens);
        assert_eq!(response.content, [Block::Text("Hal".to_owned())]);
        assert!(matches!(
            parse_response(&json!({"content": []})),
            Err(ModelError::Invalid(_))
        ));
    }
}
