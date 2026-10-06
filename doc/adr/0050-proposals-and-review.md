# 0050. Proposals and review

- Status: Accepted
- Date: 2026-10-06

## Context

AI creates only proposals (ADR 0010). Members accept them in the Review Inbox (ADR 0040).
One intake message can create many related proposals: a new person, a participation for that person, a new field definition and a fact that uses it.
A reviewer must be able to correct a value without a new round with the agent.
If the target record changed after the proposal, the proposal must not overwrite it (see [ARCHITECTURE.md](../ARCHITECTURE.md)).
Clients can send the UUID of a new record (ADR 0038).

## Decision

Proposal content:

- A proposal has one typed operation, as a Rust `enum`. The operations are:
  - create an entity of layer 1 (ADR 0049),
  - set a fact: a new state (`Accepted`, `Assumption` or `Unknown`) with its value,
  - add a field definition, add a choice value, or deprecate a field,
  - add or change a participation,
  - create a document draft (ADR 0051).
- A new record gets its UUIDv7 from the agent in the proposal. Other proposals of the same changeset refer to it by this UUID. tada rejects a proposal whose new UUID already exists.
- An operation on a record carries `expected_version: Option<n>`. `Some(n)` means: the current version must be `n`. `None` means: the record must not exist yet.
  This also covers the first fact of a field: two proposals for the first fact cannot both apply.
- Each proposal has evidence: a source version and the supporting passage (ADR 0040). A proposal without evidence is rejected at creation.
- A source version is one of these kinds:
  - a member's text from MCP, the web client or Telegram,
  - an uploaded file (ADR 0009),
  - a web page snapshot: the agent sends the URL, the retrieval time and the text it used. tada stores them and does not fetch the page itself,
  - a review edit (see below).
- A passage is a character range over the normalized text of the source version, plus the exact quote. For a PDF, it also has the page number.
- Each proposal records its author (the caller of ADR 0039) and a short reason.
- A proposal never changes after creation.

Changesets:

- The proposals of one intake form a changeset.
- A proposal can depend on other proposals of the same changeset, for example a fact on its new field definition.
- At creation, tada checks that the dependencies form a directed graph without cycles inside the changeset. Otherwise it rejects the changeset.

Review:

- The reviewer sees the changeset as a list with the evidence next to each proposal.
- For a new person or institution, the review shows existing records with similar names as duplicate candidates. The reviewer can link the proposal to an existing record instead.
- The reviewer can select proposals, reject proposals, or edit a value before acceptance.
- An edited value creates a source version of the kind `review`, with the reviewer as author. It is the evidence of the edited value. The original proposal and its evidence stay unchanged.

Apply:

- `POST /api/v1/changesets/{changeset_id}/apply` applies a selection (ADR 0044).
- Selecting a proposal also selects all its dependencies.
- An apply request is all or nothing. If the selection contains a rejected proposal, or a proposal that no longer matches its expected version, the request changes nothing. It returns `invalid-transition` or `record-version-conflict` (ADR 0037).
- A separate transaction then records the conflict for the proposals concerned, so that the conflict stays visible.
- A successful apply runs in one transaction. Event-local IDs are assigned in this transaction (ADR 0038).
- Silence never accepts. An open proposal older than 14 days shows as stale, but it does not change.

Status:

- Review results are separate, append-only records: accepted, accepted with edit, rejected, conflict or withdrawn.
- The status of a proposal is derived from its latest review result. Without one, the proposal is open.
- API values are `open`, `accepted`, `accepted-with-edit`, `rejected`, `conflict` and `withdrawn` (ADR 0044).
- A conflicting proposal does not apply. An agent or the reviewer creates a new proposal from the current version.

Routing and rights:

- In Slice 1, the event manager reviews all proposals of an event (ADR 0052).
- Routing to workstream leads comes in Slice 2 (ADR 0052).

## Consequences

- One apply request accepts a related set of records, or nothing.
- The audit log shows the AI proposal, the human edit with its own evidence, and the accepted value separately.
- Dependencies make the review more complex than single field patches, but they prevent half-applied intakes.

## Alternatives

- Field patches only, for example JSON Patch: they cannot create linked entities in one step.
- Temporary references inside a changeset: a second ID scheme next to the client UUIDs of ADR 0038.
- Partial apply with skipped conflicts: some proposals of an intake could apply without the ones they depend on.
- Automatic acceptance of the author's own intake: the agent's interpretation would become accepted state without a check.
- Acceptance through MCP: an AI could accept its own proposals, against ADR 0010.
