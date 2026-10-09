# 0039. Callers, actors and service identities

- Status: Accepted
- Date: 2026-10-06
- Amended by: [0070](0070-ai-security-review-replaces-the-external-review.md)

## Context

Members act through the web client and Telegram.
The worker and the AI PM act without a member.
ADR 0010 requires that the compiler rejects AI calls to commands other than queries and proposals.
A runtime value alone cannot make sure of this; distinct types can.
The audit log must show who did each change, through which channel, and for whom.
External clients and an MCP interface can come later. If AI drives them, they must not write outside ADR 0010.

## Decision

Callers (authorization, checked at compile time):

- Each `app` command and query takes a caller type that states what kind of caller may use it:
  - `MemberCaller`: a signed-in member. The memberships at the time of the call give the rights.
  - `ServiceCaller<S>`: a service identity `S`. Each service identity has its own type.
  - `AiCaller`: AI that acts for a principal (ADR 0010).
- A command that AI must not call does not accept `AiCaller`. A command that a service must not call does not accept its `ServiceCaller`. The compiler rejects other calls.
- A `Principal` is either a member or a service identity. It is never AI. `AiCaller` holds the `Principal` it acts for, so AI cannot act for AI.
- An `AiCaller` never has more rights than its principal.

Actors (audit, derived):

- Each caller type gives an `Actor` value for the audit log: the kind (member, service or AI), the ID, the principal for AI, the channel and the request ID.
- The `Actor` value is for records only. Code never checks permissions with it.
- The channel is one of `web`, `telegram`, `job`, `api-token` and `cli`.

Members:

- The Telegram gateway acts as the linked member (ADR 0011). The channel is `telegram`.

Service identities:

- The service identities are fixed in code: `job-runner`, `ai-pm`, `telegram-gateway` and `bootstrap`.
- Each service identity has its own caller type, so the set of commands it can call is fixed in code and visible in review.
- For `ai-pm`, this set is an upper bound. The policies that owners configure decide what it actually does, for example which reminders it sends.
- A service identity works in one organization at a time. It reads only the records that its commands need in that organization.
- `bootstrap` is the only service identity without an organization. It can only create an organization and its owner invitation (ADR 0036).
- Infrastructure queries without an organization scope exist only in `store-pg`, for example "claim the next due job". Each one is named in the code and returns the organization ID, so that the handler continues with a scoped caller.
- Service identities are not network credentials. The process roles of the `tada` binary create them in process (ADR 0002).

API tokens (from Slice 1, for MCP clients; ADR 0040):

- Members create personal API tokens for external clients, for example MCP clients.
- A token belongs to one member and one organization, and has a scope, an expiry and a name.
- At first, the only scopes are `read` and `propose`. A token caller is an `AiCaller` for its member, because an AI can be behind any token. A `write` scope needs a new ADR.
- The database stores only a hash of the token (ADR 0008). The token starts with `tada_pat_`, so that secret scanners can find it.

## Consequences

- AI cannot accept a proposal or change accepted state, even through a bug in a handler; the code does not compile.
- A permission check always knows the real person behind an AI call or a Telegram message.
- The audit log can answer "who changed this, how and for whom" for each record.
- A new service identity needs a code change and review, not a configuration change.

## Alternatives

- One `Actor` enum on each command: a runtime check only, so a wrong match arm lets AI write.
- One technical "system" user for all background work: the audit log could not separate the job runner from the AI PM.
- Service identities as database users with roles: an operator could widen their rights without review.
- API tokens with a `write` scope from the start: an AI behind a token could change accepted state.
