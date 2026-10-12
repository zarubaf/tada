# Roadmap

This roadmap divides the work into slices.
Each slice ends with a demonstration, tests, updated documents and a clean commit series.
A slice starts only after the previous slice meets its acceptance criteria.
The numbers in parentheses identify the acceptance criteria; they do not change when criteria move between slices.
The roadmap shows only the open slices.
The Git history keeps the done slices.

## Slice 2: Distributed planning and the AI PM

Goal: workstream leads own their work directly, and the AI PM follows up without the PM.

Slice 2 splits into the sub-slices 2a to 2f.
The open sub-slices are 2d to 2f.
Each sub-slice has its own design, plan, reviews and merge.
The owner wants tada usable for a first real club event as soon as possible.
So 2f comes first, then the first real event, then 2d and 2e.

tada runs with invented data and public facts only until the external review of the sign-in is done ([Sign-in before real data](#sign-in-before-real-data)).
The review of the authentication code is closed.
[ADR 0070](adr/0070-ai-security-review-replaces-the-external-review.md) records the AI security review and its fixes.
The person features of [ADR 0045](adr/0045-data-protection.md) are closed.
The export of one person, the correction with dispute notes, the legal redaction with its journal, the retention jobs, the housekeeping and the orphan sweep deliver them.

### Slice 2f: Further records and views

Scope: decisions, risks, requirements, milestones, participations, templates, the portfolio view and resource views.
The evidence of a proposal gets the capture time of its source version, as the design asks ([components](design/components.md)).
The Review Inbox shows the time of the changeset until then.

Planning views keep the record types fixed and make fields and views configurable:

- The record types and their states stay fixed, because the review routing, the reminders and the retention depend on them.
- An event manager adds custom fields to actions and commitments in the web client. They use the field definitions of [ADR 0049](adr/0049-entities-and-fact-model.md).
- An event manager saves views of an event: a table, a board and a timeline. A view keeps its filter, grouping and sort order.
- An action gets an optional start date.
  The timeline shows actions as bars, grouped by workstream, with the milestones and the date of the event.
- The resource view shows the open actions of each member for each week.
- Not in scope: dependencies between actions, a critical path, automatic rescheduling and time tracking.

A new ADR records the views and the custom fields of work records before the work starts.

Acceptance:

- (12) A simple club event can be organized without purchasing a separate task-management tool. Sub-slice 2f completes this criterion.

### Slice 2d: AI intake

Scope: AI intake inside tada (web and Telegram), the usage totals of each event and the AI cap per event.
The intake uses the model adapter, the agent runner and the AI cap of the organization that Slice 2c delivered ([ADR 0047](adr/0047-model-adapter.md), [ADR 0077](adr/0077-agent-runner.md)).

Acceptance:

- (11) An AI cap leaves manual planning usable and provides per-event usage totals.

### Slice 2e: Inbound email

Scope: inbound email with the modular adapters and the webhook of [ADR 0057](adr/0057-modular-mail-and-inbound-webhook.md), deduplication and the health of the sync.

Acceptance:

- (2) Duplicate notifications produce one source version and do not duplicate accepted commitments.
- (6) Disconnected or delayed sync is visible and cannot produce a falsely reassuring brief.

### Follow-ups of Slice 2c

Slice 2c delivered schedules, the AI PM digest, the model adapter and the agent runner, the Telegram webhook mode and the health endpoints.
The first digest runs for members who opt in.
These items are later work:

- Escalation of exceptions.
- The PM brief.
- Meeting agendas.
- Propose a new date and free-text replies to a reminder.
- A holiday calendar for working days.
- Event caps for AI cost.
- A digest projection tool without person data.
- Moving the four daily sweeps onto schedule rows.

### Owner-only items of Slice 2

These items stay with the product owner and have no sub-slice.

- The findings of the security review of the authentication code are closed (ADR 0070).
- (10) A backup restoration in the operator's deployment repository is demonstrated (ADR 0033, [release gates](release-gates.md)).

## Slice 3: Controlled outbound work

Goal: tada prepares and, under explicit rules, sends external communication.

Scope: explicit PM automation rules, draft supplier replies, optional calendar publication and participant forms.
Supplier communication, commitments and other consequential actions keep their own approval policies.

Acceptance:

- (8) A canceled job or retried outbound command cannot send duplicate mail.

## Sign-in before real data

An external review of the whole sign-in ([#46](https://github.com/zarubaf/tada/issues/46)) comes before the first real data.
It runs at the same time as Slice 2f.

These items come later:

- OAuth for MCP clients through a mature OAuth server ([#47](https://github.com/zarubaf/tada/issues/47)).
- Passkeys ([#45](https://github.com/zarubaf/tada/issues/45)).
- Single sign-on with Google and Microsoft ([#48](https://github.com/zarubaf/tada/issues/48)).

Each new sign-in method uses a general model of sign-in methods, so that it adds one method without a redesign.

## Later

Ticketing integration, richer volunteer scheduling, the aviation planning module, readiness snapshots and selected operations features.
API tokens with a `write` scope (ADR 0039) and the processing of bounces (ADR 0042).
Commercial packaging follows validation with other clubs and organizers.
