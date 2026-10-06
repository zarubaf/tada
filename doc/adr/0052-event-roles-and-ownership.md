# 0052. Event roles and record ownership

- Status: Proposed
- Date: 2026-10-06

## Context

ADRs 0049 and 0050 let the event manager accept field definitions and review proposals, but no ADR defines this role.
The original architecture brief named the event roles manager, contributor and viewer, with workstream ownership inside an event.
Organization roles (owner, admin, member) decide who belongs to the organization (glossary).
Facts have no individual owner, but work records, for example actions, do.
PRODUCT.md wants proposals routed to the person who owns the work, not always to the project manager.

## Decision

Event roles:

- An event membership gives a member one event role:

| Event role        | Rights                                                                                                                                                                     |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| event manager     | All rights of a contributor. Reviews and applies proposals, accepts field definitions and choice values, manages event memberships and participations, approves documents. |
| event contributor | Reads the event. Creates proposals and work records. Changes the work records that the contributor owns.                                                                   |
| event viewer      | Reads the event. Creates no proposals.                                                                                                                                     |

- An organization owner or admin can act as event manager in each event of the organization.
- An event has at least one event manager. The project manager (PM) of an event usually has this role.
- Event roles are access rights. Participation roles (ADR 0049) describe the involvement of people and institutions and give no access.
- Each command checks the event role at the time of the call (ADR 0039). An AI caller never has more rights than its principal.

Record ownership:

- Each work record (action, milestone, decision, risk, requirement, commitment, open question) has one owner: a member of the event.
- Facts and field definitions have no individual owner. The event managers are responsible for them.
- Documents have an owner, who requests approval. An event manager approves.

Review routing:

- Slice 1: the event managers review all proposals of their event.
- Slice 2: a proposal that targets a work record goes to the owner of that record. A proposal for a new record in a workstream goes to the workstream lead. The event managers see only proposals without an owner or overdue proposals.

Not now:

- Workstreams as records, with leads, come in Slice 2 through a new ADR.
- Fine-grained rights for single fields or records.

## Consequences

- A small club event needs one event manager and some contributors, nothing more.
- The rule "AI never has more rights than its principal" has a concrete meaning: a viewer's AI client cannot propose.
- Slice 2 can add routing without a change to the role model.

## Alternatives

- Organization roles only: every member of the club would see and change every event.
- Fact owners: too much administration for details such as "entry fee policy".
- Free role definitions for each organization: a role designer that nobody needs yet.
