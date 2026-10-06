# 0050. Proposals and review

- Status: Proposed
- Date: 2026-10-06

## Context

AI creates only proposals (ADR 0010). Members accept them in the Review Inbox (ADR 0040).
One intake message can create many related proposals: a new person, a role for that person, a new field definition and a fact that uses it.
A reviewer must be able to correct a value without a new round with the agent.
If the target record changed after the proposal, the proposal must not overwrite it (see [ARCHITECTURE.md](../ARCHITECTURE.md)).

## Decision

Proposal content:

- A proposal has one typed operation, as a Rust `enum`. The operations are:
  - create an entity of layer 1 (ADR 0049),
  - set a fact, mark a fact as unknown, or change its status,
  - add a field definition, add a choice value, deprecate a field or merge two fields,
  - add or change a participation,
  - create a document draft (ADR 0051).
- An operation on an existing record names the record and its expected version.
- Each proposal has evidence: a source version and the supporting passage (ADR 0040). A proposal without evidence is rejected at creation.
- A source version is one of these kinds:
  - a member's text from MCP, the web client or Telegram,
  - an uploaded file (ADR 0009),
  - a web page snapshot: the agent sends the URL, the retrieval time and the text it used. tada stores them and does not fetch the page itself.
- Each proposal records its author (the caller of ADR 0039) and a short reason.

Changesets:

- The proposals of one intake form a changeset.
- A proposal can depend on another proposal of the same changeset, for example a fact on its new field definition.
- A proposal can refer to a record that its changeset creates, through a temporary reference.

Review:

- The reviewer sees the changeset as a list with the evidence next to each proposal.
- The reviewer can accept all, accept a selection, reject, or edit a value before acceptance.
- An edited acceptance stores the proposed value and the accepted value. The reviewer becomes the author of the accepted change.
- If a proposal depends on a rejected proposal, it cannot be accepted.
- The selected proposals of a changeset are applied in one transaction. Event-local IDs are assigned in this transaction (ADR 0038).
- Silence never accepts. An open proposal older than 14 days shows as stale, but it does not change.

Status and conflicts:

- A proposal has the status `open`, `accepted`, `accepted_with_edit`, `rejected`, `conflict` or `withdrawn`.
- If the expected version of a target record no longer matches at acceptance, the proposal goes to `conflict`. It changes nothing.
  An agent or the reviewer can then create a new proposal from the current version.
- A proposal never changes after creation. Review results are separate records.

Routing:

- In Slice 1, the event manager reviews all proposals of an event.
- Routing to workstream leads (PRODUCT.md) comes in Slice 2, through the owner of the target record.

Rights:

- A field definition for one event: the event manager accepts it (ADR 0049).
- All other proposals: the owner of the target record or the event manager.

## Consequences

- One review step can accept a whole intake, with all its related records, or nothing of it.
- The audit log shows the AI proposal, the human edit and the accepted value separately.
- Temporary references and dependencies make the proposal format more complex than a simple field patch.

## Alternatives

- Field patches only, for example JSON Patch: they cannot create linked entities in one step.
- Automatic acceptance of the author's own intake: the agent's interpretation would become accepted state without a check.
- Acceptance through MCP: an AI could accept its own proposals, against ADR 0010.
