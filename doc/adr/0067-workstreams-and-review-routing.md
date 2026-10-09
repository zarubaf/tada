# 0067. Workstreams and review routing

- Status: Proposed
- Date: 2026-10-09
- Amends: [0050](0050-proposals-and-review.md), [0052](0052-event-roles-and-ownership.md)

## Context

ADR 0052 names two gaps for Slice 2: workstreams as records with leads, and routing of proposals to the owner of the work.
ADR 0050 lets the event managers review all proposals.
PRODUCT.md wants a proposal to reach the person who owns the work, not always the project manager.
A supplier proposal for the ground operations must reach the lead of that workstream.
A reviewer must lose the right to a proposal when the role or the ownership ends.

## Decision

Workstreams:

- A workstream is an event record with a name, a lead and a status: `active` or `closed`.
  It has no readable ID, because ADR 0038 has no prefix for it.
- The lead is a member of the event with the contributor or manager role.
- An event manager creates and renames a workstream, closes it and changes its lead.
- A work record can have a workstream.
  The workstream is optional.
- A closed workstream accepts no new records and is not a valid target of a proposal.
  Existing records keep it.
- A workstream from another event or another organization is refused with `validation-failed` and the field code `unknown-record`.
- A lead who is not a member with the contributor or manager role in the event is refused with `validation-failed` and the field code `unknown-member`.
  This holds for a lead from another event or another organization too.

Permissions of direct commands:

| Command                                                  | Who                                                                         |
| -------------------------------------------------------- | --------------------------------------------------------------------------- |
| Create, rename, close a workstream; change its lead      | event manager                                                               |
| Create an action or a commitment                         | event contributor or manager                                                |
| Change an action or a commitment (fields, status, owner) | its owner, the lead of its workstream, an event manager                     |
| Make a commitment firm (ADR 0068)                        | the same as for a change of a commitment                                    |
| Read the event memberships (names and roles)             | each member with an event role in the event, an organization owner or admin |

- Viewers read and change nothing.
- An organization owner or admin acts as event manager in each event (ADR 0052).
- Each member who can read the event also reads its membership list, with names and roles only.
  A contributor needs the list to give work to another contributor.
  This widens ADR 0052, where only managers see the memberships.
  Only managers change them.
- A request for a change with no field is refused with `validation-failed` and no field error.
  This holds for workstreams too (ADR 0068).

Routing of proposals:

1. A change of an existing action or commitment goes to its owner.
2. A new action or commitment in a workstream goes to the workstream lead.
3. A new action or commitment without a workstream goes to the event managers.
4. All other operations go to the event managers.
   This includes `CreatePerson` and `CreateInstitution` in a changeset of an event.
   Changesets of the organization go to the owners and admins, as before.
5. A proposal that creates a record also goes to the reviewers of each open proposal that refers to that record.
   In Slice 2a, this is a new person or a new institution that a new commitment names as its promisor.
   The dependent need not be selected.
   Example: a new person and a commitment from that person.
   The lead of the commitment can accept both.
   Consequence: that lead can also reject the person alone.
   Each other dependency keeps its own reviewers by rules 1 to 4.
   So an entry in `depends_on` cannot move a proposal to another reviewer.
6. The Review Inbox of an event manager shows only proposals with no other reviewer, and overdue proposals.
   A named owner or lead who has no contributor or manager role in the event now is no reviewer.
   Such a proposal goes to the event managers.
   A proposal is overdue when it is open for more than 3 days.
   The constant `OVERDUE_AFTER` lives next to the constant for stale proposals.
   Event managers keep the right to review each proposal of their event.
   A contributor reads the changesets of the event that hold a proposal for the contributor.
   A viewer reads none.
7. An apply request stays all or nothing (ADR 0050).
   The caller must be a reviewer of each selected proposal and of each dependency.
   Otherwise the request returns `forbidden` and changes nothing.
8. tada computes the reviewer set at read time and at apply time from the current state: the current owner and the current lead.
   It does not store the set.
   A change of the owner or the lead moves the open proposals with it.
   A member who loses the event role stops seeing and applying the proposals routed to that member at once.
   The proposal then goes to the event managers.

## Consequences

- The workstream lead reviews supplier proposals without the project manager as a relay.
- The event manager sees less noise, and the overdue rule stops proposals from waiting forever.
- Each read of the inbox and each apply computes the reviewers.
  The queries need care, but no reviewer table can become wrong.
- A proposal that depends on a proposal of another reviewer can need two reviewers.
  Rule 5 lets one reviewer accept a new promisor with its commitment.
  A changeset that mixes other proposals can need a manager and a lead.
- The reviewers are computed from the owner, the lead and their current roles, so the apply reads the memberships of the event.
- The 3 days are provisional.
  The stale constant of 14 days is provisional too.

## Alternatives

- A required workstream for each work record: it forces a structure on small events that need none.
- Stored reviewers on each proposal: they go out of date when an owner or a lead changes, and a former owner could still apply.
- Event managers review all proposals, as in Slice 1: it keeps the project manager as the relay that PRODUCT.md wants to remove.
- Partial apply, where the caller accepts only what the caller may review: it can leave a dependency half applied (ADR 0050).
