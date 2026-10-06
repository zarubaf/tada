# 0010. One model provider port

- Status: Accepted
- Date: 2026-10-06

## Context

AI extracts proposals, answers questions and writes drafts.
AI spend must stay inside the budget, and a cap must not block manual planning.
The organization must accept the processing region and the terms of the model provider.

## Decision

- The `app` crate defines a `Model` port.
- The port takes a prompt, the tools and an output type. `schemars` generates the JSON Schema from the Rust type. The port returns validated output and usage.
- Anthropic publishes no official Rust SDK. The `model` crate is a thin `reqwest` client for the Messages API, with no AI framework.
- Each call records the model ID, the prompt version, the token usage and the event.
- A per-event and per-organization quota pauses optional AI work when it reaches its limit.
- AI tools call the same domain commands and queries as the API, with the permissions of the caller.

## Consequences

- A second provider is one new adapter.
- Usage reports per event come from one table.
- We must check the data processing terms before we process real club data.

## Alternatives

- Vercel AI SDK or LangChain: a framework and its abstractions for one provider.
- A self-hosted model: hardware cost above the budget.
