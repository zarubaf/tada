# Roadmap

This roadmap divides the work into slices.
Each slice ends with a demonstration, tests, updated documents and a clean commit series.
A slice starts only after the previous slice meets its acceptance criteria.
The numbers in parentheses identify the acceptance criteria; they do not change when criteria move between slices.
The roadmap shows only the open slices.
The Git history keeps the done slices.

## Slice 1: Sign-in and the preliminary event concept

Goal: invited members sign in and enter the known facts of a large event.
They upload source documents and get a German preliminary concept and an enquiry draft with provenance.
tada sends no correspondence to suppliers or the public in this slice.
It sends only invitations and magic links.

Slice 1 runs with invented data and public facts only.
Real personal data needs the external review of the authentication code (ADR 0008) and the person features of ADR 0045 in Slice 2.

Scope:

- Sign-in with magic links, invitations and sessions ([ADR 0008](adr/0008-authentication.md)), and the first owner through `tada bootstrap` ([ADR 0036](adr/0036-configuration.md)).
- Organization memberships, event memberships and event roles ([ADR 0052](adr/0052-event-roles-and-ownership.md)).
- Transactional email through the worker ([ADR 0042](adr/0042-transactional-email.md)).
- The fact model and the field catalog ([ADR 0049](adr/0049-entities-and-fact-model.md)), proposals and the Review Inbox ([ADR 0050](adr/0050-proposals-and-review.md)).
- API tokens and the MCP server ([ADR 0039](adr/0039-actors-and-identities.md), [ADR 0040](adr/0040-ai-intake-through-mcp.md)).
- Source documents with versions ([ADR 0009](adr/0009-object-storage.md), [ADR 0043](adr/0043-upload-policy.md)), and document drafts with provenance ([ADR 0051](adr/0051-document-drafts-and-provenance.md)).
- The Telegram link confirmation in the web client and one Telegram command that creates a proposal ([ADR 0011](adr/0011-telegram.md)).
- The privacy notice, the API token notice and the data inventory ([ADR 0045](adr/0045-data-protection.md)).

Out of scope:

- Passkeys (ADR 0008), OAuth for MCP clients (ADR 0040) and API tokens with a `write` scope (ADR 0039).
- Self sign-up.
  Sign-up stays invite-only (ADR 0008).
- AI intake inside tada and the model adapter (Slice 2).
- Workstreams, review routing to owners, and scheduled checks (Slice 2).
- The Telegram webhook mode and the health endpoints of the `telegram` process role (Slice 2).
- The person features of ADR 0045: the export of one person, correction, legal redaction and retention jobs (Slice 2).
- Mail to suppliers or the public (Slice 3), and the processing of bounces (ADR 0042).

Work items, in this order:

1. Identity and mail:
   - Users, email identities, organization memberships and event memberships in the `identity` code module and in `store-pg`.
   - The data inventory of ADR 0045 starts with these tables.
   - The Telegram tables get a foreign key to the user.
   - The `Mailer` port, the SMTP adapter, outbound intents and the send job of the worker (ADR 0042).
2. Invitations and sign-in in the API:
   - `tada bootstrap` creates the organization and the owner invitation (ADR 0036).
   - Owners and admins invite members, revoke invitations and remove memberships ([ADR 0056](adr/0056-sign-in-details.md)).
   - Magic links, sessions, sign-out, the `Origin` check and the rate limits (ADR 0008, ADR 0056).
   - The new secret `TADA_RATE_LIMIT_KEY_FILE` of `serve` (ADR 0056).
     `mise run gen` adds it to [doc/settings.md](settings.md), and `scripts/dev_secrets.py` generates it for development.
   - The session authenticator replaces the development authenticator and the rejecting authenticator (ADR 0053).
3. Sign-in in the web client:
   - The sign-in page, the confirmation pages for magic links and invitations, the choice of the organization and sign-out.
   - The member list and the invitation form for owners and admins.
   - The event memberships page, where an event manager gives event roles (ADR 0052).
   - The Telegram link confirmation (ADR 0011).
     The gateway keeps long polling in this slice.
4. Facts and review: the field catalog, the event profile, facts, fact versions, changesets, conflicts and the Review Inbox (ADR 0049, ADR 0050).
   The event managers review all proposals (ADR 0052).
5. API tokens and MCP: the token notice, and token creation and revocation (ADR 0039, ADR 0045).
   The `/mcp` endpoint gives the read and proposal tools (ADR 0040).
6. Documents:
   - Upload with type detection, document versions and the document list in the web client ([ADR 0043](adr/0043-upload-policy.md), [ADR 0055](adr/0055-office-format-detection.md)).
   - Document drafts with a provenance manifest, approval, and the differences between two versions (ADR 0051).
7. Telegram command: a linked member proposes a change of a fact (ADR 0011).
8. Gates before the demonstration:
   - Isolation tests with two organizations, the structured export, and a backup restoration in the operator's deployment repository (ADR 0033).
   - The privacy notice in the web client, from the template of ADR 0045.
   - The external review of the authentication code (ADR 0008).

Demonstration:

1. An operator runs `tada bootstrap` for a new organization.
   The owner accepts the invitation mail and invites a second member.
   The member signs in with a magic link, and the owner makes the member an event manager of a new event.
2. The event manager describes the event in free text to their own AI agent (Claude or Codex).
   The agent proposes facts, assumptions, unknowns and open questions through MCP ([ADR 0040](adr/0040-ai-intake-through-mcp.md)).
3. The event manager accepts or corrects the proposals in the Review Inbox.
4. A member uploads source documents and browses them in the web client.
5. A member asks the agent: "What are we planning and what remains unknown?"
   The agent answers from the read tools.
6. The agent writes a German concept and an enquiry draft from accepted facts and labeled assumptions, and proposes both as document drafts.
7. tada saves both drafts as document versions with fact and source-version provenance.
8. A member links Telegram and proposes a change of the date window through a Telegram command.
   An event manager reviews it.
   The agent writes a new draft version, and tada shows the differences.
9. An operator restarts the services.
   All records, files and sessions stay available.

Acceptance:

- A person without an invitation cannot sign in.
  The sign-in form gives the same answer for a known and an unknown address.
- A magic link expires after 15 minutes and works once.
  A GET request on the link does not sign in.
- An invitation expires after 7 days and works once.
- The database contains no token in plain text: no magic link, invitation, session, link code or API token.
- A session after its idle timeout (14 days) or its absolute timeout (90 days) gets `unauthenticated`.
- After sign-out, the old session cookie gets `unauthenticated`.
- A state-changing request with a wrong `Origin` header changes nothing.
- The sixth sign-in request for one address in one hour gets `rate-limited`, also when two `serve` processes run.
- A release build contains no development authenticator.
- An event viewer cannot create proposals or a `propose` token, also through MCP.
- A removed member loses access with the next request.
- No log line contains an email address, a token or an IP address (ADR 0035).
- The tests cover the applicable requirements of OWASP ASVS 5.0 on authentication and sessions.
- The team closes the findings of the external reviewer (ADR 0008).
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
The Telegram work adds the webhook mode and the health endpoints of the `telegram` process role (ADR 0011).
The person features of ADR 0045 come in this slice: the export of one person, correction, legal redaction and retention jobs.
The scheduled checks use the job queue of [ADR 0054](adr/0054-job-queue-implementation.md).
Inbound email uses the modular adapters and the webhook of [ADR 0057](adr/0057-modular-mail-and-inbound-webhook.md) (proposed).

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
