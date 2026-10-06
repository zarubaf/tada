tada is a hosted workspace for planning multiple club events, with structured facts, browsable documents and AI participation across web and messaging. This document defines the product, its users and scope. Implementation details are in the architecture; the first deliverable and development instructions are in the PoC handoff.

## Product vision

**An affordable coordination system for organizations that run events repeatedly, with AI that works from evidence and helps the whole team participate.**

The project manager should spend time resolving exceptions and making decisions. Workstream leads should own their work directly. Volunteers and suppliers should be able to contribute through email, familiar documents and simple mobile forms.

tada maintains accepted event state and its evidence. Members participate through Telegram and a lightweight web interface using their own email addresses. Existing email, document and calendar tools remain useful but are optional integrations. A scheduled AI PM checks progress, follows up with owners and escalates exceptions. Its durable state resides in tada.

The initial market hypothesis is clubs and associations running several events each year, with temporary teams and limited software budgets. A broader commercial market remains a hypothesis to validate with other organizers. The product’s proposed distinction is accountable coordination across communications, evidence and readiness. It should not claim that competitors lack these capabilities without a deeper evaluation.

### Outcomes to optimize

- Fewer messages and status updates routed through the project manager.
- Clear ownership of every accepted action and commitment.
- Source-backed answers with visible freshness and uncertainty.
- Shared people and resources across events without leaking event-specific information.
- Reuse of event templates and lessons without copying stale approvals.
- A sustainable cost and maintenance burden for a volunteer organization.

## Research and the build boundary

Official product documentation was checked on 6 October 2026. This is a focused landscape review, not a complete procurement exercise.

| Existing capability          | Research finding                                                                                                                                                                                                                                                                                                                                     | Proposed tada boundary                                                                                                  |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Club administration          | ClubDesk covers membership, calendars, documents, email and finances. Its Swiss price page lists a limited free plan and paid plans. [ClubDesk](https://www.clubdesk.ch/de/preise)                                                                                                                                                                   | Keep any existing member and accounting system. Confirm API/export access before promising a connector.                 |
| Ticketing                    | pretix exposes a REST API; its pricing page distinguishes hosted and self-operated options. [API](https://docs.pretix.eu/dev/api/) · [Pricing](https://pretix.eu/about/en/pricing/)                                                                                                                                                                  | Integrate when public ticketing is needed. Do not build payments, refunds or gate scanning initially.                   |
| Resource operations          | Rentman provides equipment and crew capabilities and an API supporting reads and writes. [Rentman](https://rentman.io/integrations/api)                                                                                                                                                                                                              | Keep lightweight internal reservations; connect a specialist tool if the club or suppliers already use one.             |
| CRM and event administration | CiviCRM documents an extensible API. [CiviCRM](https://docs.civicrm.org/dev/en/latest/api/)                                                                                                                                                                                                                                                          | Evaluate reuse if membership/registration becomes a major need; do not introduce another platform merely to connect it. |
| Workflow automation          | n8n’s official guidance identifies commercial embedding cases requiring an Embed license. [n8n guidance](https://support.n8n.io/article/can-i-use-your-license-for-my-use-case)                                                                                                                                                                      | Optional club-side automation; do not make a sellable product depend on assumed free embedding rights.                  |
| Microsoft integration        | Graph supports change notifications and delta tracking. Permissions vary by resource and access model. [Notifications](https://learn.microsoft.com/en-us/graph/change-notifications-overview) · [Delta](https://learn.microsoft.com/en-us/graph/delta-query-overview) · [Permissions](https://learn.microsoft.com/en-us/graph/permissions-reference) | Optional later; not a release prerequisite. Validate mailbox, calendar and SharePoint scenarios individually.           |
| Future Google integration    | Gmail push uses Pub/Sub; broad mail-read scopes can require restricted-scope verification and server-side security assessment. [Push](https://developers.google.com/workspace/gmail/api/guides/push) · [Scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)                                                                       | Preserve a provider interface now; fund and validate Google support later.                                              |

**Build:** event state, ownership, commitments, decisions, risks, requirements, provenance, review workflows, portfolio conflicts and readiness.

**Reuse:** authentication libraries, email, calendar, documents, payments, ticketing, accounting, transcription services and mapping tools.

**Supply a small native fallback where otherwise nobody can participate:** tasks, simple requests/forms, assignments and resource bookings. This should be enough to run a small club event without purchasing a project-management suite.

No new paid platform is a prerequisite for the first slice. Open-source software still carries hosting, maintenance and license obligations.

## Users and everyday workflows

| User                             | Primary interaction                                                         |
| -------------------------------- | --------------------------------------------------------------------------- |
| Project manager                  | Portfolio view, exceptions, open decisions and readiness                    |
| Event/workstream lead            | Own inbox, actions, review requests and dependencies                        |
| Volunteer                        | Narrow mobile view, assignment acknowledgement, availability and completion |
| External supplier or participant | Scoped form or explicit email exchange; no access to the internal tada      |
| Club administrator               | People, resource sharing, connectors, permissions and retention             |

### Example supplier loop

1. A supplier writes to a designated event mailbox.
2. The connector captures the message once, including attachments and source identifiers.
3. AI proposes: “Generator delivery Friday 15:00, subject to signed order.” The condition is preserved.
4. Ground Ops receives the proposal directly and accepts or edits it.
5. tada stores the commitment, owner, condition, due date and exact evidence.
6. A draft follow-up or acknowledgement is prepared when needed. External sending requires an authorized person or an explicitly configured rule.
7. The PM sees an exception only if the commitment is late, unowned or affects a milestone.

Incoming text is evidence, not authority to operate tools. A supplier’s “please send the full budget” never grants access or permission.

### Example portfolio loop

The club’s summer fly-in and members’ barbecue request the same radios and volunteers. tada flags overlapping reservations and assignments. The event leads resolve them directly; the PM sees unresolved conflicts. Availability and permissions determine what each lead can see.

### Example meeting loop

A transcript or minutes file produces proposed actions and decisions. The relevant owners review them. Silence is not acceptance. Adopted decisions retain the approved wording, approver and evidence. Teams transcription availability and licensing must be verified; file upload is the initial fallback.

## Scope for the first usable version

### Core across all events

Organization, event series, event occurrence, membership/access, workstream, action, milestone, decision, risk, requirement, commitment, evidence, review proposal, participant, resource, reservation and audit history.

Each event enables only the modules it needs. A small barbecue might need actions, catering commitments, volunteers and equipment. An airshow adds requirements, risks and aviation operations. Recurring events use a versioned template; each occurrence has its own accepted state.

Templates copy checklists, roles and relative deadlines. They do not carry forward approvals, supplier acceptance, attendance or completed evidence as current facts. Prior evidence can be linked as historical reference.

### Aviation extension

Aircraft identity, event participation, documentation status, dimensions/handling needs and planned movements are typed domain records linked to the generic core. Initial aviation support is planning and document tracking. Live ATC, operational flight instructions, display approvals and automated go/no-go decisions are outside the first release.

Keep core relationships explicit. Do not disguise every domain object as a generic asset, and do not build a universal no-code schema designer.

### Initial interface

Portfolio, event overview, My Work, registers, Review Inbox and integration health. An evidence panel and Ask Event are available across views. Mobile participation matters more than a decorative dashboard.

## Proactive AI project manager

The AI PM is a persistent service in tada, not a conversation that must be kept open. A durable scheduler evaluates accepted project state; rules decide when contact is appropriate; AI prepares concise context and wording; authorized domain commands apply changes. Telegram delivers messages and receives responses. Every check, reminder, response and escalation is recorded.

### Suggested policy to configure

| Check                | Default proposal                                                            |
| -------------------- | --------------------------------------------------------------------------- |
| Due and blocked work | Every weekday at 08:30 Europe/Zurich                                        |
| Owner contact        | One private digest of relevant work, rather than a message per task         |
| Follow-up            | After two working days without a response; skip completed or snoozed tasks  |
| Escalation           | Workstream lead first; PM for unresolved critical items                     |
| PM brief             | Weekly and when a configured critical trigger occurs                        |
| Quiet hours          | 20:00–08:00 recipient local time; urgent exceptions require explicit policy |
| Meetings             | Prepare an agenda one day before a meeting stored in tada                   |

These are product defaults to discuss, not active schedules. Store timezone, working-day calendar, cadence, severity, recipients, cooldowns and escalation chain. Persist last/next run, last notified record version and response state. Coalesce reminders across events for each person. Never infer acceptance or completion from silence.

### Participation without Microsoft accounts

Invite an OK member using their existing email address. They can use a short-lived sign-in link for the web app and link Telegram through a one-time invitation. Bind the stable Telegram user ID to the approved member; do not trust display names, usernames or shared group membership as identity.

A member must start the bot before it can send private messages. The bot can also participate in an approved event group. With Telegram privacy mode enabled, use explicit commands, replies and buttons; do not assume the bot reads every group conversation. See [Telegram bot features](https://core.telegram.org/bots/features) and [Bots FAQ](https://core.telegram.org/bots/faq).

Do not send internal/private records to a group unless the configured audience permits it. Recheck membership and role at every action. Provide unlink/revoke and web/email alternatives.

### Example closed loop

“Anna, TASK-042 catering confirmation is due Friday. Is it on track?”

Buttons: On track, Blocked, Propose new date, Open task. “Blocked” records Anna’s authenticated report and asks for the blocker. A proposed date changes nothing until the applicable authority accepts it. A “Done” response records owner-reported completion; evidence verification remains separate where required. Free-text changes are extracted into proposals, with targeted confirmation when meaning is ambiguous.

The AI PM can send agreed routine internal reminders without asking the project manager each time. It cannot invent commitments, reassign accountable owners, approve authority requirements, spend money or issue aviation instructions.

### Delivery reliability

Use authenticated Telegram webhooks with a secret token, deduplicate incoming update IDs and store outbound intents before sending. Record sent, failed and unknown outcomes separately; a successful send does not prove a person read the message. Telegram sending lacks a general application idempotency key: when a timeout leaves the result unknown, do not blindly retry and promise exactly-once delivery. Surface ambiguous outcomes and suppress immediate duplicates through application policy. Validate button actor, permissions, task version and expiry. See [Telegram Bot API](https://core.telegram.org/bots/api).

A stopped worker resumes durable schedules without flooding users with missed reminders. Failed delivery triggers a visible exception or configured fallback. Keep an operator-facing heartbeat and a pause-all-messages control. The AI PM’s operation depends on a deployed worker and configured bot; writing this brief creates neither.

### Additional acceptance tests

An invited member works without a Microsoft account. A verified task owner receives a policy-authorized reminder and updates the task through Telegram. An unrelated group member cannot do so. Quiet hours and cooldowns survive restarts. Blocked bots and stale ingestion never look healthy. Ambiguous sends do not cause uncontrolled duplicate retries. A changing deadline or completed task cancels obsolete reminders. No automated routine message requires the PM to relay it.

## Cost and service expectations

The first club deployment targets **less than CHF 100/month in incremental cash operating costs**, excluding development and existing Microsoft 365 licenses. Event-day peaks, payment fees and future commercial compliance work require separate budgets.

The following is an allocation envelope, not vendor pricing:

| Cost category                         | Monthly target ceiling |
| ------------------------------------- | ---------------------- |
| Application and worker hosting        | CHF 25                 |
| Database, storage and backups         | CHF 25                 |
| Metered AI                            | CHF 25                 |
| Monitoring and miscellaneous services | CHF 10                 |
| Contingency                           | CHF 10                 |
| Total                                 | CHF 95                 |

A real quote may require combining hosting/database categories or changing deployment choices. Do not promise this budget for a public airshow’s live command system.

Control AI spend with source-version caching, incremental processing, inexpensive extraction models, larger models only for selected analyses, quotas and per-event usage reports. Hitting a cap pauses optional AI work; ordinary event records remain usable.

Track the full cost: licenses, transaction charges, support hours, upgrades and restore work. Someone must own operations. Self-hosting should be chosen only if it fits that responsibility.

Initial service scope is event planning. Provide backups with a tested restore, full exports and downloadable approved packs. Reliable offline event command, live incident dispatch and high-availability operational systems require a later design and budget.
