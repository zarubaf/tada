# 0080. Workstream proposals

- Status: Proposed
- Date: 2026-10-13

## Context

[ADR 0067](0067-workstreams-and-review-routing.md) lets only an event manager create a workstream, through a direct command.
An AI client that plans a new event through MCP ([ADR 0040](0040-ai-intake-through-mcp.md)) cannot group the first actions and commitments in a workstream.
It must wait until a manager creates the workstream, and then propose the records again ([#75](https://github.com/zarubaf/tada/issues/75)).
A contributor who leads the new workstream must not approve its creation.

## Decision

We add the operation `create-workstream` to the changesets.

- The operation carries the UUID of the new workstream, the event, the name and the lead.
  The workstream starts `active`.
- The name has 1 to 200 characters.
  No other workstream of the event has the same name, whatever its case.
  A closed workstream keeps its name.
  A taken name returns `validation-failed` with the field code `taken`, also for a name that an earlier proposal of the changeset takes.
- The lead is a member of the event with the contributor or manager role, as in ADR 0067.
  Any other lead returns `validation-failed` with the field code `unknown-member`.
- A new action or commitment can name a new workstream of the same event in the same changeset.
  It then depends on the proposal that creates the workstream (ADR 0050).
- Only the event managers review the operation.
  The future lead does not.
- The operation has no review edit.
  A reviewer who needs another name or lead rejects the proposal.
- The apply refuses a workstream whose ID or name another workstream has at the apply.
  The proposal then gets the status `conflict`, also when a concurrent apply takes the name first.
- The apply checks the role of the lead again.
  If the lead lost the contributor or manager role after the proposal, the proposal gets the status `conflict`.

## Consequences

- An agent proposes a workstream and its first records in one changeset, and a manager accepts them together.
- A selected action that depends on a new workstream needs a manager, because the apply needs a reviewer of each dependency (ADR 0067, rule 7).
- The routing needs no new rule.
  An operation that changes no record and names no workstream goes to the event managers.

## Alternatives

- Route the new workstream to its future lead: the lead would approve the own role, and ADR 0067 lets only managers create workstreams.
- Let the agent create the workstream directly: changes by an agent go through review (ADR 0040).
- Let a new record pass its reviewers on to the new workstream by rule 5: a lead of another workstream could then create a workstream.
