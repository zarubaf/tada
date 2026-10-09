# Roadmap

This roadmap divides the work into slices.
Each slice ends with a demonstration, tests, updated documents and a clean commit series.
A slice starts only after the previous slice meets its acceptance criteria.
The numbers in parentheses identify the acceptance criteria; they do not change when criteria move between slices.
The roadmap shows only the open slices.
The Git history keeps the done slices.

## Slice 2: Distributed planning and the AI PM

Goal: workstream leads own their work directly, and the AI PM follows up without the PM.

tada runs with invented data and public facts only until two gates close.
The first gate is the external review of the authentication code ([ADR 0008](adr/0008-authentication.md)).
The second gate is the person features of [ADR 0045](adr/0045-data-protection.md) in this slice.

Scope: AI intake inside tada (web and Telegram) through the model adapter, actions and commitments, distributed review, durable scheduled checks, internal Telegram reminders, decisions, risks, requirements, templates, portfolio and resource views, and inbound email.
The Telegram work adds the webhook mode and the health endpoints of the `telegram` process role (ADR 0011).
The person features of ADR 0045 come in this slice: the export of one person, correction, legal redaction and retention jobs.
The sweep of orphan objects in the object storage comes in this slice too ([ADR 0009](adr/0009-object-storage.md), [ADR 0045](adr/0045-data-protection.md)).
An orphan is an object that no document version refers to, for example after a crash or an unknown commit outcome.
The sweep also removes the parts of multipart uploads that a canceled request left incomplete.
A lifecycle rule of the bucket in the deployment repository can do the second part (ADR 0033).
The scheduled checks use the job queue of [ADR 0054](adr/0054-job-queue-implementation.md).
Inbound email uses the modular adapters and the webhook of [ADR 0057](adr/0057-modular-mail-and-inbound-webhook.md) (proposed).
The evidence of a proposal gets the capture time of its source version, as the design asks ([components](design/components.md)).
The Review Inbox shows the time of the changeset until then.

Acceptance:

- (2) Duplicate notifications produce one source version and do not duplicate accepted commitments.
- (3) The designated workstream lead can review a supplier proposal without the PM relaying the message.
- (5) A conditional promise stays conditional; no signature or approval is invented.
- (6) Disconnected or delayed sync is visible and cannot produce a falsely reassuring brief.
- (11) An AI cap leaves manual planning usable and provides per-event usage totals.
- (12) A simple club event can be organized without purchasing a separate task-management tool.
- Quiet hours and cooldowns survive restarts.
- A changed deadline or a completed action cancels obsolete reminders.
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
