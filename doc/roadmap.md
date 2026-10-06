# Roadmap

This roadmap divides the work into slices.
Each slice ends with a demonstration, tests, updated documents and a clean commit series.
A slice starts only after the previous slice meets its acceptance criteria.
The numbers in parentheses identify the acceptance criteria; they do not change when criteria move between slices.

## Slice 0: Foundation and walking skeleton

Goal: a repository and a runtime that later slices can build on without rework.

1. Foundation: tools, documentation checks, secret scanning, ADRs and CI. (Done.)
2. Walking skeleton:
   - Docker Compose with PostgreSQL, Garage and Mailpit.
   - The `tada` binary with the roles `serve` and `worker`, and the web client.
   - One domain command, `CreateEvent`, from the API to the web client, with a test.
   - The first migration, the OpenAPI snapshot check and the crate boundary check.
3. Spikes:
   - The job queue (ADR 0007).
   - Upload, download and versions in object storage (ADR 0009).
   - Telegram identity linking (ADR 0011).
4. A priced deployment plan for staging and production (ADRs 0015 and 0016).

## Slice 1: Preliminary event concept

Goal: the team enters the known facts of a large event, uploads source documents and gets a German preliminary concept and an enquiry draft with provenance.
tada sends no correspondence in this slice.

Demonstration:

1. A member enters the known facts in an event profile, with assumptions, unknowns and open questions.
2. A member uploads source documents and browses them in the web client.
3. A member asks: "What are we planning and what remains unknown?"
4. tada generates a German concept and an enquiry draft from accepted facts and labeled proposals.
5. tada saves both drafts as document versions with fact and source-version provenance.
6. A member proposes a change of the date window through Telegram. The owner reviews it. tada generates a new draft version and shows the differences.
7. An operator restarts the services. All records and files stay available.

Acceptance:

- Every asserted fact traces to an accepted field or an exact source version.
- An unknown value stays unknown.
- Members find files without the chat.
- A reviewed Telegram change shows in web queries.
- Nobody can overwrite an approved version.
- A schema upgrade keeps the concept, the sources, the relationships and the old versions.
- (1) Two organizations and several events have isolated access, including search, citations, background jobs and exports.
- (4) A changed accepted record causes an old proposal to conflict rather than overwrite it.
- (7) A document update preserves the evidence behind a prior accepted decision.
- (9) An AI answer identifies source versions and separates accepted state from unreviewed proposals.
- (10) Backup restoration and structured export are demonstrated.
- (11) An AI cap leaves manual planning usable and provides per-event usage totals.

## Slice 2: Distributed planning and the AI PM

Goal: workstream leads own their work directly, and the AI PM follows up without the PM.

Scope: actions and commitments, distributed review, durable scheduled checks, internal Telegram reminders, decisions, risks, requirements, templates, portfolio and resource views, and inbound email.

Acceptance:

- (2) Duplicate notifications produce one source version and do not duplicate accepted commitments.
- (3) The designated workstream lead can review a supplier proposal without the PM relaying the message.
- (5) A conditional promise stays conditional; no signature or approval is invented.
- (6) Disconnected or delayed sync is visible and cannot produce a falsely reassuring brief.
- (12) A simple club event can be organized without purchasing a separate task-management tool.
- Quiet hours and cooldowns survive restarts.
- A changed deadline or a completed action cancels obsolete reminders.

## Slice 3: Controlled outbound work

Goal: tada prepares and, under explicit rules, sends external communication.

Scope: explicit PM automation rules, draft supplier replies, optional calendar publication and participant forms.
Supplier communication, commitments and other consequential actions keep their own approval policies.

Acceptance:

- (8) A canceled job or retried outbound command cannot send duplicate mail.

## Later

Ticketing integration, richer volunteer scheduling, the aviation planning module, readiness snapshots and selected operations features.
Commercial packaging follows validation with other clubs and organizers.
