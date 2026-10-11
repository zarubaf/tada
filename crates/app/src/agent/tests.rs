use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use jiff::civil::Date;
use jiff::{SignedDuration, Timestamp};
use serde::Deserialize;
use tada_domain::RecordVersion;
use tada_domain::identity::OrganizationRole;
use tada_domain::ids::{OrganizationId, UserId};
use uuid::Uuid;

use super::*;
use crate::ai_cost::{Month, Price};
use crate::audit::AuditEvent;
use crate::caller::{ActorKind, AiOrigin, AiPm, Channel, OrgScope, ServiceIdentity};
use crate::model::{ModelResponse, Stop};
use crate::proposals::MayPropose;
use crate::tokens::TokenScope;

const START: &str = "2026-10-12T06:30:00Z";

/// A clock that a model call moves on.
#[derive(Debug)]
struct TestClock(Mutex<Timestamp>);

impl TestClock {
    fn new() -> Self {
        Self(Mutex::new(START.parse().unwrap()))
    }

    fn advance(&self, by: SignedDuration) {
        let mut now = self.0.lock().unwrap();
        *now += by;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}

/// A model that answers each call with the next scripted result. Each call takes `duration` on `clock`.
#[derive(Debug)]
struct ScriptModel {
    results: Mutex<VecDeque<Result<ModelResponse, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
    clock: Arc<TestClock>,
    duration: SignedDuration,
}

impl ScriptModel {
    fn new(clock: &Arc<TestClock>, results: Vec<Result<ModelResponse, ModelError>>) -> Self {
        Self {
            results: Mutex::new(results.into()),
            requests: Mutex::default(),
            clock: clock.clone(),
            duration: SignedDuration::from_secs(1),
        }
    }

    fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl Model for ScriptModel {
    async fn call(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.requests.lock().unwrap().push(request.clone());
        self.clock.advance(self.duration);
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(ModelError::Invalid("no scripted result left".to_owned())))
    }
}

/// The usage of 10 000 input and 1 000 output tokens: CHF 0.015 at the test price.
const USAGE: Usage = Usage {
    input_tokens: 10_000,
    output_tokens: 1_000,
};

fn tool_call(id: &str, name: &str, input: Value) -> Result<ModelResponse, ModelError> {
    Ok(ModelResponse {
        content: vec![
            Block::Text("I check the event.".to_owned()),
            Block::ToolUse {
                id: id.to_owned(),
                name: name.to_owned(),
                input,
            },
        ],
        stop: Stop::ToolUse,
        usage: USAGE,
    })
}

fn answer(text: &str) -> Result<ModelResponse, ModelError> {
    Ok(ModelResponse {
        content: vec![Block::Text(text.to_owned())],
        stop: Stop::EndTurn,
        usage: USAGE,
    })
}

/// A cost store with one price for all models: CHF 1 for each million input tokens, CHF 5 for output tokens.
#[derive(Debug, Default)]
struct TestCost {
    no_price: bool,
    records: Mutex<Vec<(OrgScope, UsageRecord)>>,
}

impl TestCost {
    fn records(&self) -> Vec<UsageRecord> {
        let records = self.records.lock().unwrap();
        records.iter().map(|(_, record)| record.clone()).collect()
    }
}

#[async_trait]
impl AiCostStore for TestCost {
    async fn price(&self, _: OrgScope, _: &str, _: Date) -> Result<Option<Price>, StoreError> {
        Ok((!self.no_price).then_some(Price {
            input_per_mtok: Chf::from_millirappen(100_000),
            output_per_mtok: Chf::from_millirappen(500_000),
        }))
    }

    async fn month_usage(&self, _: OrgScope, _: Month) -> Result<Chf, StoreError> {
        Ok(Chf::ZERO)
    }

    async fn record(&self, scope: OrgScope, usage: &UsageRecord) -> Result<(), StoreError> {
        self.records.lock().unwrap().push((scope, usage.clone()));
        Ok(())
    }

    async fn cap(&self, _: OrgScope) -> Result<Option<(Chf, RecordVersion)>, StoreError> {
        Ok(None)
    }

    async fn set_cap(
        &self,
        _: OrgScope,
        _: Chf,
        _: RecordVersion,
        _: &AuditEvent,
    ) -> Result<Option<RecordVersion>, StoreError> {
        Ok(None)
    }

    async fn ai_enabled(&self, _: OrgScope) -> Result<bool, StoreError> {
        Ok(true)
    }
}

/// The calls of the test tools: the tool name, the caller and the input.
type Calls = Arc<Mutex<Vec<(&'static str, AiCaller, Value)>>>;

/// A tool that records its calls and answers a fixed result.
struct Recorder {
    name: &'static str,
    read_only: bool,
    calls: Calls,
}

#[async_trait]
impl AiTool<()> for Recorder {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        "A test tool."
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {"event_key": {"type": "string"}}})
    }

    fn output_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn call(&self, caller: &AiCaller, input: Value, _: &()) -> Result<Value, AiToolError> {
        self.calls
            .lock()
            .unwrap()
            .push((self.name, caller.clone(), input));
        Ok(json!({"actions": [{"local_id": "ACT-001", "status": "open"}]}))
    }
}

fn catalog(calls: &Calls) -> ToolCatalog<()> {
    let tool = |name, read_only| -> Arc<dyn AiTool<()>> {
        Arc::new(Recorder {
            name,
            read_only,
            calls: calls.clone(),
        })
    };
    ToolCatalog::new(vec![
        tool("list_actions", true),
        tool("propose_changeset", false),
    ])
}

const LIMITS: AiLimits = AiLimits {
    max_turns: 6,
    max_cost: Chf::from_millirappen(5_000),
    run_timeout: SignedDuration::from_secs(60),
    call_timeout: SignedDuration::from_secs(20),
    call_retries: 2,
};

const TASK: AgentTask = AgentTask {
    name: "pm-digest",
    prompt_version: 1,
    system: "Answer with the summary.",
    max_tokens: 1_000,
};

fn member() -> MemberCaller {
    MemberCaller::new(
        UserId::from_uuid(Uuid::from_u128(1)),
        OrganizationId::from_uuid(Uuid::from_u128(2)),
        OrganizationRole::Member,
    )
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
struct Summary {
    summary: String,
}

/// Only the event `FLY28` is in the limits of the test runs.
fn fly28(_: &'static str, input: &Value) -> bool {
    input["event_key"] == "FLY28"
}

/// A run with `limits` against `model`.
async fn run(
    model: &ScriptModel,
    cost: &TestCost,
    calls: &Calls,
    limits: AiLimits,
) -> AgentRun<Summary> {
    let tools = catalog(calls);
    let ports = AgentPorts {
        model,
        tools: &tools,
        tool_ports: &(),
        cost,
        clock: &*model.clock,
        limits,
    };
    run_agent(&ports, member(), &TASK, "the input".to_owned(), &fly28).await
}

fn tool_results(request: &ModelRequest) -> Vec<(String, bool)> {
    request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|block| match block {
            Block::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn runner_answers_after_a_tool_call_and_records_each_call() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(
        &clock,
        vec![
            tool_call("t1", "list_actions", json!({"event_key": "FLY28"})),
            answer(r#"{"summary": "One action is open."}"#),
        ],
    );
    let (cost, calls) = (TestCost::default(), Calls::default());
    let run = run(&model, &cost, &calls, LIMITS).await;

    assert_eq!(
        run.result.unwrap(),
        Summary {
            summary: "One action is open.".to_owned()
        }
    );
    assert_eq!(run.model_id, "claude-haiku-5-5");
    assert_eq!((run.task, run.prompt_version), ("pm-digest", 1));
    let model_step = AgentStep::Model {
        usage: USAGE,
        cost: Chf::from_millirappen(1_500),
    };
    assert_eq!(
        run.steps,
        [
            model_step,
            AgentStep::Tool {
                tool: "list_actions",
                outcome: ToolOutcome::Done
            },
            model_step,
        ]
    );
    assert_eq!(run.cost, Chf::from_millirappen(3_000));

    let records = cost.records();
    assert_eq!(records.len(), 2);
    for record in &records {
        assert_eq!(record.task, "pm-digest");
        assert_eq!(record.prompt_version, 1);
        assert_eq!(record.model_id, "claude-haiku-5-5");
        assert_eq!(record.event_id, None);
        assert_eq!(record.usage, USAGE);
        assert_eq!(record.cost, Chf::from_millirappen(1_500));
    }
    assert!(
        cost.records
            .lock()
            .unwrap()
            .iter()
            .all(|(scope, _)| *scope == member().scope())
    );

    let requests = model.requests();
    let first = &requests[0];
    assert_eq!(first.model_id, "claude-haiku-5-5");
    assert_eq!(first.system, TASK.system);
    assert_eq!(first.max_tokens, 1_000);
    assert_eq!(
        first.messages,
        [Message {
            role: Role::User,
            content: vec![Block::Text("the input".to_owned())]
        }]
    );
    assert!(first.output_schema.is_some());
    // The second call holds the tool call of the model and the result of the tool.
    let second = &requests[1];
    assert_eq!(second.messages.len(), 3);
    assert_eq!(second.messages[1].role, Role::Assistant);
    let results = tool_results(second);
    assert_eq!(results.len(), 1);
    assert!(!results[0].1);
    assert!(results[0].0.contains("ACT-001"));
}

#[tokio::test]
async fn a_tool_acts_as_the_ai_pm_for_the_member_with_the_read_scope() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(
        &clock,
        vec![
            tool_call("t1", "list_actions", json!({"event_key": "FLY28"})),
            answer(r#"{"summary": "ok"}"#),
        ],
    );
    let (cost, calls) = (TestCost::default(), Calls::default());
    run(&model, &cost, &calls, LIMITS).await.result.unwrap();

    let calls = calls.lock().unwrap();
    let (_, caller, input) = &calls[0];
    assert_eq!(*input, json!({"event_key": "FLY28"}));
    assert_eq!(caller.origin(), AiOrigin::Agent { task: "pm-digest" });
    assert_eq!(caller.token_scope(), TokenScope::Read);
    assert!(!caller.credential_allows_proposals());
    let actor = caller.actor();
    assert_eq!(actor.kind(), ActorKind::Ai);
    assert_eq!(actor.id(), AiPm::ID);
    assert_eq!(actor.principal(), Some(Uuid::from_u128(1)));
    assert_eq!(actor.channel(), Channel::Job);
    assert_eq!(AiPm::NAME, "ai-pm");
}

#[tokio::test]
async fn write_tool_is_not_offered() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(
        &clock,
        vec![
            tool_call(
                "t1",
                "propose_changeset",
                json!({"event_key": "FLY28", "changes": []}),
            ),
            answer(r#"{"summary": "ok"}"#),
        ],
    );
    let (cost, calls) = (TestCost::default(), Calls::default());
    let run = run(&model, &cost, &calls, LIMITS).await;

    let requests = model.requests();
    let offered: Vec<&str> = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(offered, ["list_actions"]);
    assert!(calls.lock().unwrap().is_empty(), "no tool ran");
    assert_eq!(run.steps[1], AgentStep::NotOffered);
    let results = tool_results(&requests[1]);
    assert_eq!(results, [(json!({"error": "forbidden"}).to_string(), true)]);
    assert!(run.result.is_ok());
}

#[tokio::test]
async fn a_tool_call_outside_the_limits_does_not_run() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(
        &clock,
        vec![
            tool_call("t1", "list_actions", json!({"event_key": "OTHER1"})),
            answer(r#"{"summary": "ok"}"#),
        ],
    );
    let (cost, calls) = (TestCost::default(), Calls::default());
    let run = run(&model, &cost, &calls, LIMITS).await;

    assert!(calls.lock().unwrap().is_empty(), "no tool ran");
    assert_eq!(
        run.steps[1],
        AgentStep::Tool {
            tool: "list_actions",
            outcome: ToolOutcome::OutsideLimits
        }
    );
    let results = tool_results(&model.requests()[1]);
    assert_eq!(results, [(json!({"error": "forbidden"}).to_string(), true)]);
    assert!(run.result.is_ok());
}

#[tokio::test]
async fn runner_stops_at_turn_limit() {
    let clock = Arc::new(TestClock::new());
    let calls_of_model = (0..10)
        .map(|n| {
            tool_call(
                &format!("t{n}"),
                "list_actions",
                json!({"event_key": "FLY28"}),
            )
        })
        .collect();
    let model = ScriptModel::new(&clock, calls_of_model);
    let (cost, calls) = (TestCost::default(), Calls::default());
    let limits = AiLimits {
        max_turns: 3,
        max_cost: Chf::from_millirappen(100_000),
        ..LIMITS
    };
    let run = run(&model, &cost, &calls, limits).await;

    assert!(matches!(run.result, Err(AgentError::Turns)), "{run:?}");
    assert_eq!(model.requests().len(), 3);
    assert_eq!(cost.records().len(), 3);
}

#[tokio::test]
async fn runner_stops_at_cost_limit() {
    let clock = Arc::new(TestClock::new());
    let calls_of_model = (0..10)
        .map(|n| {
            tool_call(
                &format!("t{n}"),
                "list_actions",
                json!({"event_key": "FLY28"}),
            )
        })
        .collect();
    let model = ScriptModel::new(&clock, calls_of_model);
    let (cost, calls) = (TestCost::default(), Calls::default());
    let limits = AiLimits {
        max_turns: 10,
        ..LIMITS
    };
    let run = run(&model, &cost, &calls, limits).await;

    // Each call costs CHF 0.015: the fourth call takes the run to 0.06, above the limit of 0.05.
    assert!(matches!(run.result, Err(AgentError::Cost)), "{run:?}");
    assert_eq!(model.requests().len(), 4);
    assert_eq!(cost.records().len(), 4, "the last call is recorded too");
    assert_eq!(
        calls.lock().unwrap().len(),
        3,
        "no tool runs after the limit"
    );
    assert_eq!(run.cost, Chf::from_millirappen(6_000));
}

#[tokio::test]
async fn runner_stops_at_timeout() {
    let clock = Arc::new(TestClock::new());
    let calls_of_model = (0..10)
        .map(|n| {
            tool_call(
                &format!("t{n}"),
                "list_actions",
                json!({"event_key": "FLY28"}),
            )
        })
        .collect();
    let mut model = ScriptModel::new(&clock, calls_of_model);
    model.duration = SignedDuration::from_secs(25);
    let (cost, calls) = (TestCost::default(), Calls::default());
    let run = run(&model, &cost, &calls, LIMITS).await;

    // The calls end after 25, 50 and 75 seconds: the third ends after the limit of 60 seconds.
    assert!(matches!(run.result, Err(AgentError::Timeout)), "{run:?}");
    assert_eq!(model.requests().len(), 3);
    assert_eq!(cost.records().len(), 3);
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_model_without_a_price_is_not_called() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(&clock, vec![answer(r#"{"summary": "ok"}"#)]);
    let cost = TestCost {
        no_price: true,
        ..TestCost::default()
    };
    let run = run(&model, &cost, &Calls::default(), LIMITS).await;

    assert!(matches!(run.result, Err(AgentError::NoPrice)), "{run:?}");
    assert!(model.requests().is_empty());
}

#[tokio::test]
async fn a_refusal_records_its_usage() {
    let clock = Arc::new(TestClock::new());
    let model = ScriptModel::new(&clock, vec![Err(ModelError::Refused { usage: USAGE })]);
    let cost = TestCost::default();
    let run = run(&model, &cost, &Calls::default(), LIMITS).await;

    assert!(
        matches!(
            run.result,
            Err(AgentError::Model(ModelError::Refused { .. }))
        ),
        "{run:?}"
    );
    assert_eq!(cost.records().len(), 1);
}

#[tokio::test]
async fn an_invalid_answer_is_an_output_error_without_its_text() {
    let clock = Arc::new(TestClock::new());
    let secret = "Lena Muster owes CHF 500";
    for response in [
        Ok(ModelResponse {
            content: vec![Block::Text(secret.to_owned())],
            stop: Stop::ToolUse,
            usage: USAGE,
        }),
        answer(&format!(r#"{{"summary": 5, "note": "{secret}"}}"#)),
        answer(secret),
        Ok(ModelResponse {
            content: vec![Block::Text(secret.to_owned())],
            stop: Stop::MaxTokens,
            usage: USAGE,
        }),
    ] {
        let model = ScriptModel::new(&clock, vec![response]);
        let run = run(&model, &TestCost::default(), &Calls::default(), LIMITS).await;
        let error = run.result.unwrap_err();
        assert!(matches!(error, AgentError::Output(_)), "{error:?}");
        assert!(!error.to_string().contains("Lena"), "{error}");
    }
}
