# Roadmap

This roadmap divides the work into slices.
Each slice ends with a demonstration, tests, updated documents and a clean commit series.
A slice starts only after the previous slice meets its acceptance criteria.
The numbers in parentheses identify the acceptance criteria; they do not change when criteria move between slices.
The roadmap shows only the open slices.
The Git history keeps the done slices.

## Slice 1: Preliminary event concept

Goal: the team enters the known facts of a large event, uploads source documents and gets a German preliminary concept and an enquiry draft with provenance.
tada sends no correspondence in this slice.
Real personal data enters tada only after ADR 0045 is accepted, also in this slice. Before that, Slice 1 runs with invented data and public facts.

Demonstration:

1. A member describes the event in free text to their own AI agent (Claude or Codex). The agent proposes facts, assumptions, unknowns and open questions through MCP ([ADR 0040](adr/0040-ai-intake-through-mcp.md)).
2. The member accepts or corrects the proposals in the Review Inbox.
3. A member uploads source documents and browses them in the web client.
4. A member asks the agent: "What are we planning and what remains unknown?" The agent answers from the read tools.
5. The agent writes a German concept and an enquiry draft from accepted facts and labeled assumptions, and proposes both as document drafts.
6. tada saves both drafts as document versions with fact and source-version provenance.
7. A member proposes a change of the date window through a Telegram command. The owner reviews it. The agent writes a new draft version, and tada shows the differences.
8. An operator restarts the services. All records and files stay available.

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

## Slice 2: Distributed planning and the AI PM

Goal: workstream leads own their work directly, and the AI PM follows up without the PM.

Scope: AI intake inside tada (web and Telegram) through the model adapter, actions and commitments, distributed review, durable scheduled checks, internal Telegram reminders, decisions, risks, requirements, templates, portfolio and resource views, and inbound email.
The Telegram work adds the webhook mode, the health endpoints of the `telegram` process role and the web page that confirms a Telegram link (ADR 0011).
The scheduled checks use the job queue of [ADR 0054](adr/0054-job-queue-implementation.md).

Acceptance:

- (2) Duplicate notifications produce one source version and do not duplicate accepted commitments.
- (3) The designated workstream lead can review a supplier proposal without the PM relaying the message.
- (5) A conditional promise stays conditional; no signature or approval is invented.
- (6) Disconnected or delayed sync is visible and cannot produce a falsely reassuring brief.
- (11) An AI cap leaves manual planning usable and provides per-event usage totals.
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
