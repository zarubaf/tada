# 0068. Actions and commitments

- Status: Proposed
- Date: 2026-10-09
- Amends: [0050](0050-proposals-and-review.md)

## Context

Slice 2a adds the work records that members and the AI PM follow: actions and commitments.
The glossary says that a conditional commitment stays conditional, and that no signature or approval is invented.
A commitment comes from a person or an institution (ADR 0069).
The audit log keeps the history of changes, but a reader of a record also needs the evidence of each accepted change.
ADR 0038 defines the prefixes `ACT` and `COM`.
ADR 0052 defines the owner of a work record.

## Decision

Storage:

- Each kind has its own typed table with the current row and a record version.
  The audit log keeps the history.
- A new table `record_evidence` keeps the evidence of each accepted change.
  Each row holds the organization, the record, the record version that the change produced, the source version, the offsets, the quote and the page.
  The record is one foreign key for each kind, which can be empty, with a `CHECK` that exactly one is set.
  One row exists for each passage of an accepted proposal that created or changed an action, a commitment, a person or an institution.
- The record view shows the evidence of each version with the capture time of the source version.
- The evidence follows the source reach of the caller (`app::access::source_reach`).
  A person or an institution belongs to the organization, but its evidence cites a source of an event.
  A reader without access to that source does not see the passage.
  Evidence from a changeset of the organization has no event, so only owners and admins see it.
- Direct commands carry no evidence in 2a.

Action (`ACT`, event scope):

- Fields: title (at most 200 characters), optional description (at most 4000), owner, optional workstream, optional due date (a calendar date, JSON name `due_date`) and status.
- The owner is a member of the event with the contributor or manager role.
- Statuses: `open`, `in-progress`, `blocked`, `done`, `canceled`.
- Transitions:
  - `open` and `in-progress` change into each other.
  - `open` and `in-progress` change to `blocked`.
    `blocked` changes to `open` or `in-progress`.
  - `open`, `in-progress` and `blocked` change to `done` or `canceled`.
  - `done` changes to `open` (reopen).
  - `canceled` is final.
- A change to the current status is not a transition.
  It returns `invalid-transition`.

Commitment (`COM`, event scope):

- Fields: text (at most 500 characters), promisor, owner, optional workstream, optional due date (`due_date`) and optional condition text (at most 500).
- The promisor is exactly one person or one institution of the organization.
- The owner is the member of the event who follows the commitment up.
  The owner has the contributor or manager role.
- The initial status is `conditional` if the condition text is set, else `firm`.
- Transitions:
  - `conditional` changes to `firm`.
    This needs a reason (at most 500 characters).
    The condition text stays as history.
  - `conditional` and `firm` change to `fulfilled`, `broken` or `withdrawn`.
  - `fulfilled`, `broken` and `withdrawn` are final.
  - A change to the current status returns `invalid-transition`.
- A command cannot remove the condition text of a conditional commitment.
  The only way to `firm` is the explicit "make firm" command, or an accepted proposal of that change.
  The command to change a commitment refuses the status `firm`.
- The "make firm" command needs a reason.
  The reason is stored on the commitment as `firm_reason`.
  The audit log records who made it firm and when.
  It holds no free text (ADR 0039).
- Who may change a record: its owner, the lead of its workstream and an event manager (ADR 0067).
  The same rule covers "make firm".
  A viewer changes nothing.

Operations in proposals (`domain::proposals::Operation`):

- `CreateAction`, `CreateCommitment`, `ChangeActionStatus`, `ChangeActionDue` and `ChangeCommitmentStatus`.
  The create operations carry the UUID of the new record, as in ADR 0050.
  The change operations carry `expected_version`.
- A `CreateCommitment` names its promisor as a person or an institution.
- A change to `firm` through a proposal is the AI path for "condition met".
  The owner reviews it.
  The evidence is the passage, for example the signed order.
  The reason of the proposal becomes `firm_reason` of the commitment, so it has at most 500 characters.
  The accepted change writes its evidence with the new record version.
- The JSON name of the due date is `due_date` in the operations and in the direct API.
  The operation `CreateInstitution` names the institution kind `institution_kind`, because `kind` is the tag of the operation.
  The direct API of institutions uses `kind` (ADR 0069).
- The review edit supports the text fields, the due date and the condition text of the create operations.
  An edit of the owner or the workstream is a field edit too.
  The same rules as for a direct command apply.
  An edit cannot remove the condition of a proposed commitment.
  An edit that adds a condition makes the commitment start `conditional`.
  An edited owner who is not a contributor or a manager of the event is refused with `validation-failed` on `edits/i/fields/owner`.
  The record keeps the passages of the proposal and the edited values as a source version of the kind `review`.
- A proposal that names a workstream conflicts if the workstream closes before the apply.
  A proposal of a new action or commitment conflicts if its owner is no longer a contributor or a manager of the event at the apply, edited or not.
  A status change conflicts if its transition is not allowed from the status of the record at the apply.
  Two changes of one record in one changeset are checked in order, each against the status that the earlier one wrote.
- The operations listed here cover the proposals of 2a.

Not in 2a:

- Proposals that change the owner of a record.
  The AI PM never reassigns owners.
- Proposals that create workstreams.

Concurrency and errors:

- A change carries `expected_version`.
  If it does not match, the command returns `record-version-conflict`.
  Of two changes with the same version, one wins.
- A forbidden transition, or a change to the current status, returns `invalid-transition`.
- A text over its limit, or an unknown promisor, owner or workstream, returns `validation-failed` with a field code (ADR 0037).
  The field code for a promisor or a workstream of another organization or event is `unknown-record`.
  The field code for an owner (or a lead) who is not a contributor or a manager of the event is `unknown-member`.
- Readable IDs are given when the record becomes accepted state (ADR 0038).

Conventions of the five record kinds (workstreams, actions, commitments, persons and institutions):

- Client ID: a create takes an optional ID that must be a UUIDv7.
  Without it, the server chooses one.
  An ID that is not a UUIDv7 returns the field code `not-uuid-v7` on `id`.
  An ID that any record of the kind holds, in any organization, returns the field code `taken`.
  A retry with the same ID returns `taken` too, because a create is not idempotent.
- Empty change: a PATCH with no field to change returns `validation-failed` with no field error and writes nothing.
- Order of checks in a change: access, record found, empty change, version, values (all field errors at once), then the transition.
- Time source: the app clock gives `created_at` and `updated_at` of each write.
  The store never uses the database clock for these tables.
- One write path: the direct command and the apply of a proposal call the same insert and update of a table.
- Paging: a list pages by the local number of the record, with a cursor (ADR 0044).
  `next_cursor` is absent on the last page.
- Reads: a read works for a member and for the AI client of that member, with the rights of the member.
  A command needs a member.
- Views: each action and commitment view carries `can_change` and `next_statuses`, and each commitment view also carries `can_make_firm`.
  Each person and institution view carries `can_change`.
  They come from the one permission rule and the one transition rule, so a client does not repeat them.
  `next_statuses` is empty when `can_change` is false and never lists `firm`.
  `can_make_firm` is true if `can_change` is true and the status is `conditional`.
- My Work lists the open actions and commitments of the caller in each event that the caller can read: an event role of any kind, or the organization role owner or admin.
  The open records of an event that the caller lost access to do not show.

## Consequences

- A conditional commitment cannot become firm by accident.
  The acceptance criterion (5) has a testable rule.
- Each accepted change of the four kinds has evidence that a reader can open.
- The current row is easy to query.
  The history needs the audit log and `record_evidence` together.
- Each new table joins the structured export and the data inventory in the same commit.

## Alternatives

- Approach B, an append-only table of versions for each kind: a reader sees every state, but each list query needs the latest row, and the schema doubles.
- Approach C, one generic records table with a JSON payload: it moves the type checks from the schema to the code and weakens the foreign keys to promisors and workstreams.
- Evidence in the audit log only: the log holds no personal data (ARCHITECTURE.md), so it cannot hold quotes.
- A free `condition_met` flag instead of the status `firm`: it hides the change from the status filter and from the audit trail.
