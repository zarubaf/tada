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
Each sub-slice has its own design, plan, reviews and merge.
The owner wants tada usable for a first real club event as soon as possible, so the sub-slices come in this order.

tada runs with invented data and public facts only until two gates close.
The first gate is the external review of the authentication code ([ADR 0008](adr/0008-authentication.md)).
The second gate is the person features of [ADR 0045](adr/0045-data-protection.md), which sub-slice 2b delivers.

### Slice 2a: Work records and distributed review

Scope: workstreams, actions, commitments, persons, institutions, review routing to owners and workstream leads, and the "My Work" page.
The decisions are in [ADR 0067](adr/0067-workstreams-and-review-routing.md), [ADR 0068](adr/0068-actions-and-commitments.md) and [ADR 0069](adr/0069-persons-and-institutions.md).

Acceptance:

- (3) The designated workstream lead can review a supplier proposal without the PM relaying the message.
- (5) A conditional promise stays conditional; no signature or approval is invented.
- (12) A simple club event can be organized without purchasing a separate task-management tool. Sub-slice 2a delivers a part of this criterion.

### Slice 2b: Person features

Scope: the person features of ADR 0045: the export of one person, correction, legal redaction and retention jobs.
The sweep of orphan objects in the object storage comes here too ([ADR 0009](adr/0009-object-storage.md), [ADR 0045](adr/0045-data-protection.md)).
An orphan is an object that no document version refers to, for example after a crash or an unknown commit outcome.
The sweep also removes the parts of multipart uploads that a canceled request left incomplete.
A lifecycle rule of the bucket in the deployment repository can do the second part (ADR 0033).

Acceptance: the data-protection gate of ADR 0045 closes.

### Slice 2c: Schedules and reminders

Scope: durable scheduled checks, AI PM checks, internal Telegram reminders, the webhook mode and the health endpoints of the `telegram` process role (ADR 0011).
The scheduled checks use the job queue of [ADR 0054](adr/0054-job-queue-implementation.md).

Acceptance:

- Quiet hours and cooldowns survive restarts.
- A changed deadline or a completed action cancels obsolete reminders.

### Slice 2d: AI intake

Scope: AI intake inside tada (web and Telegram) through the model adapter, the AI cap and the usage totals of each event.

Acceptance:

- (11) An AI cap leaves manual planning usable and provides per-event usage totals.

### Slice 2e: Inbound email

Scope: inbound email with the modular adapters and the webhook of [ADR 0057](adr/0057-modular-mail-and-inbound-webhook.md), deduplication and the health of the sync.

Acceptance:

- (2) Duplicate notifications produce one source version and do not duplicate accepted commitments.
- (6) Disconnected or delayed sync is visible and cannot produce a falsely reassuring brief.

### Slice 2f: Further records and views

Scope: decisions, risks, requirements, milestones, participations, templates, the portfolio view and resource views.
The evidence of a proposal gets the capture time of its source version, as the design asks ([components](design/components.md)).
The Review Inbox shows the time of the changeset until then.

Acceptance:

- (12) A simple club event can be organized without purchasing a separate task-management tool. Sub-slice 2f completes this criterion.

### Owner-only items of Slice 2

These items stay with the product owner and have no sub-slice.

- The team closes the findings of the external reviewer (ADR 0008).
- (10) A backup restoration in the operator's deployment repository is demonstrated (ADR 0033, [release gates](release-gates.md)).

## Slice 3: Controlled outbound work

Goal: tada prepares and, under explicit rules, sends external communication.

Scope: explicit PM automation rules, draft supplier replies, optional calendar publication and participant forms.
Supplier communication, commitments and other consequential actions keep their own approval policies.

Acceptance:

- (8) A canceled job or retried outbound command cannot send duplicate mail.

## Later

Ticketing integration, richer volunteer scheduling, the aviation planning module, readiness snapshots and selected operations features.
Passkeys (ADR 0008), OAuth for MCP clients (ADR 0040), API tokens with a `write` scope (ADR 0039) and the processing of bounces (ADR 0042).
Commercial packaging follows validation with other clubs and organizers.
