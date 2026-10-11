# 0077. Agent runner in Rust

- Status: Proposed
- Date: 2026-10-11
- Amends: [0047](0047-model-adapter.md)

## Context

The AI features are the main value of tada: the AI PM digest, the intake of slice 2d, Ask Event and the PM brief.
The digest of slice 2c uses AI wording, and the intake of slice 2d reuses the same adapter.
They need one safe loop that calls a model, runs tools and ends.
[ADR 0010](0010-model-provider.md) limits AI to queries and proposals through `AiCaller`, and the compiler enforces this.
[ADR 0047](0047-model-adapter.md) defines tasks, prompts, caps and the evaluation gate, and it expects a model adapter.
The MCP tools today are bound to `rmcp` and read `AiCaller` from the HTTP request.

Existing agent frameworks do not fit:

- The Claude Agent SDK, LangChain and LangGraph are Python or TypeScript libraries.
  They would need a second runtime next to the Rust binary ([ADR 0003](0003-runtime-and-tooling.md)).
- A framework runs its own tool loop in its own runtime.
  The tools would sit outside the compile-time authority of `AiCaller` in the Rust code.
- Anthropic Managed Agents run on hosted infrastructure of the provider.
  The hosted runtime would hold the tools and the data outside `AiCaller`, and it is another hosted service to check under ADR 0045.
- The Tool Runner of Anthropic has no Rust SDK.

## Decision

Model port and client:

- The `app` crate gets a `Model` port.
  It takes messages, tools with JSON Schema from `schemars` and a structured result.
  It returns content, tool calls and usage.
- The `adapters` crate implements it with a thin client for the Anthropic Messages API on the new direct dependency `reqwest`.
  It uses strict tools and structured output, and no SDK.
- A fake adapter replays recorded responses for tests and for the evaluation in CI.
- Each call has a timeout of 20 seconds.
  A 429, a 5xx or an overload answer is retried at most 2 times with a backoff.

Tool catalog:

- The tool definitions move from the MCP server into an `app::ai_tools` catalog.
  A tool has a name, a description, an input schema and a run function that takes `&AiCaller`.
- The MCP server and the agent runner both use the catalog.
  A tool outside `AiCaller` does not compile.
- A tool gets only an `AiCaller` and the opaque `AiToolPorts`.
  Code outside `app` cannot read the scope from a caller, and the AI tools hold no store, so they reach data only through the read and propose functions of `app::ai_queries`.
- `AiCaller` gets an origin: a token for MCP, or an agent run with a task name.
- For `pm-digest`, the worker builds the `AiCaller` from the member caller of the recipient with the read scope.

Runner:

- `app::agent` loops from model call to tool calls to tool results until the model ends.
- The limits per run are 6 model turns, CHF 0.05 and 60 seconds in total.
  Each run stops at the first limit.
- Each step is recorded with the task, the prompt version, the model ID, the tool name and the usage.
  The audit log holds no free text ([ADR 0039](0039-actors-and-identities.md)).
- Tool results are data.
  The runner never treats text in a result as an instruction.

Files and settings:

- `ai/models.json` maps each task to a model ID and an effort.
  The file is JSON, because `serde_json` is a dependency and `toml` is not.
- `pm-digest` uses `claude-haiku-5-5` with low effort.
- Prompts are files in `ai/prompts/{task}/v{n}.md` ([ADR 0047](0047-model-adapter.md)).
- A change of a model ID or a prompt needs the evaluation gate.
  So these two stay in files.
- The operator sets the other values.
  Each has a default and is validated at start.
  `tada settings` lists them:
  - `TADA_MODEL_API_KEY_FILE`, `TADA_MODEL_API_URL` (default `https://api.anthropic.com`) and `TADA_MODEL_PROVIDER_CHECKED_ON`.
  - `TADA_AI_DEFAULT_MONTHLY_CAP_CHF` (default 25).
  - `TADA_AI_RUN_MAX_TURNS` (6), `TADA_AI_RUN_MAX_COST_CHF` (0.05) and `TADA_AI_RUN_TIMEOUT` (60 seconds).
  - `TADA_AI_CALL_TIMEOUT` (20 seconds) and `TADA_AI_CALL_RETRIES` (2).
- The role does not start if a key is set without `TADA_MODEL_PROVIDER_CHECKED_ON` ([ADR 0045](0045-data-protection.md)).

Cost control:

- `ai_price` holds the price per million tokens in Swiss francs for each model and validity date.
  The operator converts from the provider currency.
- `ai_usage` holds the organization, the optional event, the task, the prompt version, the model ID, the tokens, the cost and the time.
- The cost of a digest goes to the organization.
- `ai_cap` holds the monthly cap of an organization in Swiss francs.
  A new organization gets the default from the setting.
  An owner or admin changes it ([ADR 0047](0047-model-adapter.md)).
- AI work runs only if the organization switched it on, the key and the check date are set, and the usage of the month is below the cap.
  The check is before a run, so the usage can go above the cap by at most the cost limit of each run that started below it.
  The cap month is the calendar month in UTC.
- No cache applies to `pm-digest`, because each input is new.

Task `pm-digest`:

- The input is the item list (opaque reference, kind, title, event name, due date, status, reason), the locale and today's date.
  It holds no person names.
- The tools are read-only and limited to the records of the digest.
- The output is an intro and one line for each reference.
- The runner checks that each reference appears exactly once, no unknown reference appears, and the lengths are in range.
- The buttons always come from the rules and never from the model.
- On any failure, the template digest is sent.
- The evaluation set `eval/pm-digest/` holds invented cases: German and English, an injection in a title, a missing item, an extra item, an over-long text and a tool call outside the limits.
  CI replays recorded responses.
  A Python script does the live run with a fixed cost cap ([ADR 0034](0034-python-for-scripts.md)).

## Consequences

- One loop serves all AI features.
  A new task adds a prompt, a model line, tools from the catalog and an evaluation set.
- The compiler keeps AI inside `AiCaller`, also for tada's own agents.
- We own the client code for one provider API. This is small, because the API is plain JSON over HTTPS.
- A second provider needs a second adapter behind the same port.
- The operator can tune cap and limits without a release.
  Model and prompt changes still pass the gate.
- `reqwest` is a new direct dependency.

## Alternatives

- Claude Agent SDK: Python and TypeScript only, so it needs a second runtime, and its tool loop sits outside `AiCaller`.
- LangChain or LangGraph as a sidecar: a second runtime and a second deployment, and the same bypass of `AiCaller`.
- Managed Agents: a hosted runtime for tools and data outside `AiCaller`, and another service to check under ADR 0045.
- Tool Runner of Anthropic: no Rust SDK.
- Limits and cap as constants in code: each change would need a release.
  Operators set them as settings instead.
