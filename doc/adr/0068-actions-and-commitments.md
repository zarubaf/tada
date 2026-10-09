# 0068. Actions and commitments

- Status: Proposed
- Date: 2026-10-09
- Amends: [0050](0050-proposals-and-review.md)

## Context

Slice 2a adds the work records that members and the AI PM follow: actions and commitments.
The glossary says that a conditional commitment stays conditional, and that no signature or approval is invented.
A commitment comes from a person or an institution (ADR 0069).
The audit log keeps the history of changes, but a reader of a record also needs the evidence of each accepted change.
ADR 0038 defines the prefixes `ACT` and `COM`. ADR 0052 defines the owner of a work record.

## Decision

Storage:

- Each kind has its own typed table with the current row and a record version.
  The audit log keeps the history.
- A new table `record_evidence` keeps the evidence of each accepted change.
  Each row holds the organization, the record, the record version that the change produced, the source version, the offsets, the quote and the page.
  The record is one foreign key for each kind, which can be empty, with a `CHECK` that exactly one is set.
  One row exists for each passage of an accepted proposal that created or changed an action, a commitment, a person or an institution.
- The record view shows the evidence of each version with the capture time of the source version.
- Direct commands carry no evidence in 2a.

Action (`ACT`, event scope):

- Fields: title (at most 200 characters), optional description (at most 4000), owner, optional workstream, optional due date (a calendar date) and status.
- The owner is a member of the event with the contributor or manager role.
- Statuses: `open`, `in-progress`, `blocked`, `done`, `canceled`.
- Transitions:
  - `open` and `in-progress` change into each other.
  - `open` and `in-progress` change to `blocked`. `blocked` changes to `open` or `in-progress`.
  - `open`, `in-progress` and `blocked` change to `done` or `canceled`.
  - `done` changes to `open` (reopen).
  - `canceled` is final.

Commitment (`COM`, event scope):

- Fields: text (at most 500 characters), promisor, owner, optional workstream, optional due date and optional condition text (at most 500).
- The promisor is exactly one person or one institution of the organization.
- The owner is the member of the event who follows the commitment up. The owner has the contributor or manager role.
- The initial status is `conditional` if the condition text is set, else `firm`.
- Transitions:
  - `conditional` changes to `firm`. This needs a reason (at most 500 characters). The condition text stays as history.
  - `conditional` and `firm` change to `fulfilled`, `broken` or `withdrawn`.
  - `fulfilled`, `broken` and `withdrawn` are final.
- A command cannot remove the condition text of a conditional commitment.
  The only way to `firm` is the explicit "make firm" command, or an accepted proposal of that change.
- The "make firm" command needs a reason. The reason is stored on the commitment. The audit log records who made it firm and when.
  The audit log holds no free text (ADR 0039).

Operations in proposals (`domain::proposals::Operation`):

- `CreateAction`, `CreateCommitment`, `ChangeActionStatus`, `ChangeActionDue` and `ChangeCommitmentStatus`.
  The create operations carry the UUID of the new record, as in ADR 0050. The change operations carry `expected_version`.
- A `CreateCommitment` names its promisor as a person or an institution.
- A change to `firm` through a proposal is the AI path for "condition met".
  The owner reviews it. The evidence is the passage, for example the signed order.
  The reason of the proposal becomes the reason of the firm commitment, so it has at most 500 characters.
- The review edit supports the text fields, the due date and the condition text of the create operations.
  An edit of the owner or the workstream is a field edit too. The same rules as for a direct command apply.
  An edit cannot remove the condition of a proposed commitment. An edit that adds a condition makes the commitment start `conditional`.
  The record keeps the passages of the proposal and the edited values as a source version of the kind `review`.
- A proposal that names a workstream conflicts if the workstream closes before the apply.
- The operations listed here cover the proposals of 2a.

Not in 2a:

- Proposals that change the owner of a record. The AI PM never reassigns owners.
- Proposals that create workstreams.

Concurrency and errors:

- A change carries `expected_version`. If it does not match, the command returns `record-version-conflict`. Of two changes with the same version, one wins.
- A forbidden transition returns `invalid-transition`.
- A text over its limit, or an unknown promisor, owner or workstream, returns `validation-failed` with a field code (ADR 0037).
  The field code for a record of another organization or event is `unknown-record`.
- Readable IDs are given when the record becomes accepted state (ADR 0038).

## Consequences

- A conditional commitment cannot become firm by accident. The acceptance criterion (5) has a testable rule.
- Each accepted change of the four kinds has evidence that a reader can open.
- The current row is easy to query. The history needs the audit log and `record_evidence` together.
- Each new table joins the structured export and the data inventory in the same commit.

## Alternatives

- Approach B, an append-only table of versions for each kind: a reader sees every state, but each list query needs the latest row, and the schema doubles.
- Approach C, one generic records table with a JSON payload: it moves the type checks from the schema to the code and weakens the foreign keys to promisors and workstreams.
- Evidence in the audit log only: the log holds no personal data (ARCHITECTURE.md), so it cannot hold quotes.
- A free `condition_met` flag instead of the status `firm`: it hides the change from the status filter and from the audit trail.
