# 0039. Actors and service identities

- Status: Proposed
- Date: 2026-10-06

## Context

Members act through the web client and Telegram.
The worker and the AI PM act without a member.
AI tools must have less authority than a member (ADR 0010).
The audit log must show who did each change, through which channel, and for whom.
External clients and an MCP interface can come later.

## Decision

Actors:

- Each `app` command and query takes an `Actor`. The type has these kinds:
  - `Member(UserId)`: a signed-in person.
  - `Service(ServiceId)`: a named role of tada itself.
  - `Ai { on_behalf_of }`: AI that acts for a member or a service. It has the `AiCaller` capability of ADR 0010 and never more rights than the actor it acts for.
- Each call also carries a `Channel`: `web`, `telegram`, `worker`, `api-token` or `cli`.
- The audit log stores the actor kind, the actor ID, `on_behalf_of`, the channel and the request ID.

Members:

- A member's rights come only from the organization and event memberships at the time of the call.
- The Telegram gateway acts as the linked member (ADR 0011). The channel is `telegram`.

Service identities:

- The service identities are fixed in code: `worker`, `ai-pm`, `telegram-gateway` and `bootstrap`.
- Each service identity has a fixed list of allowed commands in code. For example, `ai-pm` can read due work and create reminder intents, but it cannot accept a proposal.
- A service identity works in one organization at a time. A job carries its organization ID.
- Service identities are not network credentials. The roles of the `tada` binary create them in process (ADR 0002).

API tokens (later):

- When an external client or an MCP interface needs access, members create personal API tokens.
- A token belongs to one member and has a scope (`read`, `propose` or `write`), an expiry and a name.
- The database stores only a hash of the token (ADR 0008). The token starts with `tada_pat_`, so that secret scanners can find it.
- A token never has more rights than its member has at the time of the call.

## Consequences

- A permission check always knows the real person behind an AI call or a Telegram message.
- The audit log can answer "who changed this, how and for whom" for each record.
- A new service identity needs a code change and review, not a configuration change.

## Alternatives

- One technical "system" user for all background work: the audit log could not separate the worker from the AI PM.
- Service identities as database users with roles: an operator could widen their rights without review.
- API tokens from the start: no client needs them in Slice 1.
