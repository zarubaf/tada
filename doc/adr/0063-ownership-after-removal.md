# 0063. Ownership after removal

- Status: Proposed
- Date: 2026-10-08

## Context

[ADR 0052](0052-event-roles-and-ownership.md) says that each work record, for example an open question, has one owner: a member of the event.
It also says that documents have an owner, who requests approval.
tada checks this rule when it assigns an owner.
A member can leave the organization, and an owner, an admin or an event manager can remove a membership.
After the removal, the open questions and documents of the member keep the member as owner, and the owner cannot read the event.
Nothing records this decision.
[ADR 0051](0051-document-drafts-and-provenance.md) also does not say who owns a document that a draft creates.

## Decision

- tada checks the rule "the owner is a member of the event" when it assigns an owner: at the creation of a record and at each later change of the owner.
- A removal of an organization membership or an event membership keeps the owner ID of each record.
  The evidence, the audit log and the history refer to this ID, so tada does not erase it.
- A removal does not wait for a reassignment. Slice 1 has no command that assigns a new owner.
- The web client shows an owner without an event role as a former member.
  An event manager can assign a new owner when such a command exists.
- The member who accepts a draft proposal owns the new document of the draft.
  This member is also `uploaded_by` of the draft version.

## Consequences

- A removal never fails because of old records.
- A record can have an owner who is not a member of the event. Each reader of the owner must handle this case.
  For example, the review routing of Slice 2 must send a proposal for such a record to the event managers.
- A later command that assigns a new owner checks the new owner with the same rule.
- The web tasks that show open questions and documents show the former member.

## Alternatives

- Refuse the removal while the member owns open records: an owner could not remove an inactive member, because Slice 1 has no command to assign a new owner.
- Clear the owner at the removal: the owner columns do not allow NULL, and the history would lose the owner.
