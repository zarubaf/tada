# 0061. Audit subject and role detail

- Status: Proposed
- Date: 2026-10-07

## Context

ADR 0039 requires that the audit log answers "who changed this, how and for whom".
An audit event names the actor, the action, the record kind and the record ID.
A change of a membership has no record ID of its own, so the event cannot name the member whom the change is about.
The event also does not show which role the member had before and after the change.
For example, the acceptance of an invitation can raise the role of an existing member, and the log showed no trace of it.
ADR 0045 forbids personal data and free text in the log.
The names of actions were free strings in three places, and later tasks add more actions.

## Decision

- The column `audit_event.subject_user_id` names the member whom an audit event is about: the audit subject.
  It has no foreign key, like `actor_id`, because the log outlives users.
- The column `audit_event.detail` holds only the keys `old_role` and `new_role`.
  Their values come from the closed set of organization roles and event roles.
  A missing key means "no role": `old_role` is missing for a new membership, `new_role` for a removed one.
  An event without a role change has no detail.
- The `app` type `AuditRole` is the only value that can enter `detail`.
  A `CHECK` constraint in the database enforces the same rule.
- `CHECK` constraints also limit `actor_kind` and `channel` to the values of ADR 0039.
- The actions are a closed enum, `AuditAction`. It gives each action its one name and its record kind.
  A new action is a new variant of the enum.
- The record kind of an event membership change is `event_membership`. Its record ID is the event and its subject is the member.

## Consequences

- The log answers "whom did this change affect and how" without personal data.
- The acceptance of an invitation records the role before and after if the membership is new or its role changed.
- A new kind of detail needs a new ADR and a migration of the `CHECK` constraint.
- The structured export (ADR 0059) must include `subject_user_id` and `detail`.
- Two indexes support the queries "the log of one organization in time order" and "the events about one member".

## Alternatives

- A free `jsonb` detail without a constraint: it would let free text and personal data into the log.
- A separate audit table for each record kind: more tables and queries for the same rule.
- Record the member in `record_id`: the event would lose the event that the membership belongs to.
