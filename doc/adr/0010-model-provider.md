# 0010. One model provider port with limited AI authority

- Status: Proposed
- Date: 2026-10-06

## Context

AI extracts proposals, answers questions and writes drafts.
AI proposals are never accepted state (see [ARCHITECTURE.md](../ARCHITECTURE.md)).
Documents and emails are untrusted input; a prompt injection must not cause a write.
AI spend must stay inside the budget, and a cap must not block manual planning.
Source texts contain personal data of members and third parties.

## Decision

The port:

- The `app` crate defines a `Model` port.
- The port takes a prompt, the tools and an output type. `schemars` generates the JSON Schema from the Rust type. The port returns validated output and usage.
- Anthropic publishes no official Rust SDK. The adapter is a thin `reqwest` client for the Messages API, with no AI framework.
- Each call records the model ID, the prompt version, the token usage and the event.
- A quota for each event and each organization pauses optional AI work at its limit.

AI authority:

- AI tools get an `AiCaller` capability. It is a separate Rust type from a member's capability.
- With `AiCaller`, a tool can call only queries and `propose_*` commands.
- A write with `AiCaller` is possible only through a policy that names the command (for example, internal reminders). The compiler rejects other calls.
- AI tools run with the permissions of the member or service identity that started them, never more.

Data protection:

- No real personal data goes to a model provider before the data processing ADR (see [roadmap](../roadmap.md)) is accepted.
- Each organization has a switch that turns off all model calls.

## Consequences

- A second provider is one new adapter.
- Usage reports for each event come from one table.
- A prompt injection can at most create a proposal, which a person reviews.

## Alternatives

- AI tools with the full command set: a prompt injection could change accepted state.
- An AI framework: abstractions for one provider that we do not need.
- A self-hosted model: hardware cost above the budget.
