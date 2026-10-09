# Glossary

This glossary gives each tada term exactly one meaning.
Docs, code identifiers, API names and UI keys use these terms.
If a term is missing, add it here in the same commit that uses it.

The "Avoid" column lists words that have a different meaning or no fixed meaning.
"(open)" marks a term that still needs a decision.

## Organization and people

| Term                      | Meaning                                                                                                                                              | Avoid                                    |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| organization              | The tenant: a club or association. It owns all its data. Data never crosses an organization boundary.                                                | club (in code), tenant (in UI)           |
| organization slug         | The unique key of an organization, for example `testwil`: 2 to 32 lowercase letters, digits and hyphens (ADR 0036).                                  | short name                               |
| privacy notice            | The text that tells members and invitees how the organization handles personal data. Without an own text, the template applies (ADR 0045).           | privacy policy                           |
| user                      | A person with a stable internal UUID. Email and Telegram are linked credentials, not identities.                                                     | account                                  |
| member                    | A user with an organization membership and one organization role: owner, admin or member.                                                            |                                          |
| external identity         | A credential linked to a user, for example a Telegram user ID or an email address.                                                                   |                                          |
| link code                 | A single-use code that a member sends to the bot to link a Telegram account. The member confirms the link in the web client (ADR 0011).              |                                          |
| invitation                | A single-use link that makes a person a member of an organization with one organization role (ADR 0008, ADR 0056).                                   | invite (as a noun)                       |
| invitee                   | The person whom an invitation names. The invitee accepts the invitation and becomes a member (ADR 0056).                                             |                                          |
| magic link                | A single-use sign-in link that tada sends to the email address of a member. It expires after 15 minutes (ADR 0008).                                  | login link                               |
| session                   | The signed-in state of one browser, stored in the `session` table and named by a cookie (ADR 0008).                                                  | login                                    |
| organizing committee (OK) | The Organisationskomitee of one event: the people who plan it. Write "OK" only after you define it in a document.                                    | committee                                |
| project manager (PM)      | The person who coordinates an event and makes the final decisions.                                                                                   |                                          |
| event manager             | The event role with all rights in one event: review, apply, field definitions, memberships and document approval (ADR 0052).                         | admin (that is an organization role)     |
| event contributor         | The event role that reads the event, creates proposals and work records, and changes its own work records.                                           | editor                                   |
| event viewer              | The event role that only reads the event.                                                                                                            | guest                                    |
| event membership          | The record that gives one member one event role in one event (ADR 0052).                                                                             | participation (that gives no access)     |
| workstream                | One area of work in an event, for example catering or ground operations. The German UI calls it „Arbeitsbereich“.                                    | team, department                         |
| workstream lead           | The member who owns a workstream and reviews the proposals routed to it (ADR 0067). The German UI calls it „Arbeitsbereichsleitung“.                 |                                          |
| volunteer                 | A person who receives assignments for an event.                                                                                                      | helper                                   |
| supplier                  | An external party that provides goods or services. Suppliers have no access to internal records.                                                     | vendor                                   |
| person                    | A record of a human being in an organization. A person can exist without a user account. The German UI calls it „Person“.                            | contact                                  |
| institution               | A record of an organization outside the tenant, for example an authority, a company or another club (kinds: „Behörde“, „Firma“, „Verein“, „Andere“). | organization (that is the tenant), party |
| participation             | A person or an institution in an event, with a participation role and a status: invited, interested, confirmed or declined. Not access.              | membership                               |
| participation role        | The involvement of a person or an institution in an event, for example organizing committee member, sponsor or authority. Gives no access.           | role (alone)                             |

## Events

| Term             | Meaning                                                                                                                            | Avoid                   |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| event series     | A recurring kind of event, for example the annual fly-in.                                                                          |                         |
| event            | One occurrence of an event, with its own dates and accepted state.                                                                 | occurrence (in UI)      |
| template         | A versioned set of checklists, roles and relative deadlines for an event series. A template never copies approvals or evidence.    |                         |
| module           | A domain feature that an event enables, for example aviation. Do not confuse with a code module.                                   | plugin                  |
| event profile    | The typed fields of an event and all its facts, including assumptions and unknowns.                                                |                         |
| fact             | A value for one field of one event, with a status (accepted, assumption or unknown) and evidence (ADR 0049).                       |                         |
| fact version     | One immutable state of a fact: accepted with a value, assumption with a value, or unknown, with its evidence.                      | revision                |
| assumption       | The fact status for a value that the team uses for planning but did not confirm.                                                   |                         |
| unknown          | The fact status for a value that nobody knows yet. The fact has no value, and tada never fills it in.                              | empty, missing          |
| field definition | A record that defines a fact field: key, label, value type, description and module. Only `choice` fields can allow several values. | attribute, custom field |
| field catalog    | All field definitions that an event can use: the shipped modules and the event's own fields.                                       | schema (alone)          |
| value type       | One of the fixed kinds of fact value: text, boolean, quantity, money, date, date window, choice or reference.                      | data type (alone)       |
| open question    | A question that the team must answer. It has an owner.                                                                             |                         |

## Work

| Term        | Meaning                                                                                                                                                                                                                                                                     | Avoid            |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- |
| action      | A piece of work with one owner, a status and a due date. Statuses: open, in progress, blocked, done, canceled („offen“, „in Arbeit“, „blockiert“, „erledigt“, „abgebrochen“). The German UI calls it „Aufgabe“.                                                             | todo             |
| milestone   | A date by which a set of actions must be complete.                                                                                                                                                                                                                          | deadline         |
| commitment  | A promise from a person or an institution, with its conditions. A conditional commitment stays conditional. Statuses: conditional, firm, fulfilled, broken, withdrawn („bedingt“, „verbindlich“, „erfüllt“, „gebrochen“, „zurückgezogen“). The German UI calls it „Zusage“. | agreement        |
| promisor    | The person or the institution that makes a commitment. A commitment has exactly one (ADR 0068).                                                                                                                                                                             | supplier (alone) |
| make firm   | The command that changes a conditional commitment to firm. It needs a reason, which tada stores on the commitment (ADR 0068).                                                                                                                                               | confirm          |
| My Work     | The start page after sign-in: the open actions and commitments of the member in each event that the member can read, and the number of proposals to review. The German UI calls it „Meine Arbeit“.                                                                          | dashboard        |
| decision    | An approved statement with its exact wording, approver and evidence.                                                                                                                                                                                                        |                  |
| risk        | A possible problem with a likelihood, an impact and an owner.                                                                                                                                                                                                               | issue            |
| requirement | A condition that the event must meet, for example from an authority.                                                                                                                                                                                                        |                  |
| resource    | A shared item, for example radios or a generator.                                                                                                                                                                                                                           | asset            |
| reservation | A booking of a resource for a period.                                                                                                                                                                                                                                       |                  |
| assignment  | A volunteer allocated to an action or a shift.                                                                                                                                                                                                                              |                  |

## Evidence and review

| Term                | Meaning                                                                                                                                                                                                                                     | Avoid                    |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| accepted state      | The records that a member with the correct authority accepted. Only domain commands change it.                                                                                                                                              | truth, master data       |
| record version      | A number that increases with each change of a record. Commands use it for optimistic concurrency.                                                                                                                                           | revision                 |
| source item         | An incoming item: the member text of an intake, a review edit, an uploaded document, a mail message or a Telegram message.                                                                                                                  |                          |
| source version      | One immutable version of a source item, with its hash and capture time.                                                                                                                                                                     |                          |
| evidence link       | A link from a record to an exact location in a source version.                                                                                                                                                                              | citation (in code)       |
| passage             | A range of characters in the normalized text of a source version, with its exact quote and, for a PDF, its page (ADR 0050).                                                                                                                 |                          |
| excerpt             | A passage with at most 100 characters of the text before and after it, so that a reviewer sees the passage in its context.                                                                                                                  | snippet (of evidence)    |
| record evidence     | The passages of an accepted proposal that created or changed an action, a commitment, a person or an institution, with the record version that the change produced (ADR 0068). A reader sees them as far as the reader can read the source. |                          |
| provenance          | The set of evidence links and fact versions behind a record or a generated draft.                                                                                                                                                           |                          |
| proposal            | A suggested change to accepted state, with its source, its author and the target record version. A proposal is never accepted state.                                                                                                        | suggestion               |
| review              | The act in which the owner accepts, edits or rejects a proposal. Silence is never acceptance.                                                                                                                                               | approval (of a proposal) |
| operation           | The one typed change of a proposal, for example "set a fact" or "add a field definition" (ADR 0050).                                                                                                                                        | patch                    |
| review result       | One append-only record of a review of a proposal: accepted, accepted with edit, rejected, conflict or withdrawn (ADR 0050).                                                                                                                 | status (of the record)   |
| conflict            | The state of a proposal when its target record changed after the proposal was created.                                                                                                                                                      |                          |
| changeset           | The proposals of one intake, reviewed together. A proposal can depend on another proposal of its changeset.                                                                                                                                 | batch                    |
| apply               | The command that accepts selected proposals of a changeset and their dependencies, all or nothing (ADR 0050).                                                                                                                               | merge (of proposals)     |
| stale               | The mark of an open proposal that is older than 14 days. A stale proposal does not change.                                                                                                                                                  | expired                  |
| overdue proposal    | An open proposal that is older than 3 days. The event managers see it in their inbox even if another member reviews it (ADR 0067).                                                                                                          | stale                    |
| reviewer            | A member who can accept or reject a proposal by the routing: the owner of the record, the lead of the workstream or an event manager (ADR 0067). A member without a current contributor or manager role is no reviewer.                     | approver                 |
| Review Inbox        | The list of changesets with open proposals that the review routing gives a member, and the overdue proposals for event managers.                                                                                                            | approval queue           |
| provenance manifest | The list of fact versions and source passages that one document version uses, extracted from its `tada:` links.                                                                                                                             | citations list           |
| legal redaction     | The audited replacement of personal data with a tombstone, the only exception to immutability (ADR 0045).                                                                                                                                   | deletion (for evidence)  |

## Documents

| Term             | Meaning                                                                                                                                                    | Avoid          |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| document         | A file with a stable tada ID. The ID does not change with its name or folder.                                                                              | file (in code) |
| document version | One immutable upload or generated draft of a document.                                                                                                     |                |
| draft            | A document version in Markdown that tada adds when a member accepts a draft proposal. It has a status.                                                     |                |
| approved version | A draft version with the status `approved`. Nobody can overwrite it.                                                                                       | final          |
| storage quota    | The largest total size of the uploaded files of one organization (ADR 0043).                                                                               |                |
| export           | All data of one organization as files (ADR 0059): JSON Lines, CSV, the original files and a manifest with hashes. An operator makes it with `tada export`. | dump, backup   |
| backup           | A copy of the database and the object storage of one installation. The operator makes and restores it outside tada (ADR 0033).                             |                |

## AI and automation

| Term       | Meaning                                                                                                                                   | Avoid        |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| AI PM      | The scheduled service that checks progress, sends reminders and escalates exceptions. It is not a person and has no authority of its own. | bot, agent   |
| bot        | The Telegram bot: one channel of tada.                                                                                                    |              |
| reminder   | A message from the AI PM to an owner about an action.                                                                                     | notification |
| escalation | A message to a workstream lead or the PM about an unresolved item.                                                                        |              |
| policy     | A configured rule that permits an automatic action.                                                                                       |              |

## Architecture

| Term             | Meaning                                                                                                                                           | Avoid                                            |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| domain command   | The only way to change accepted state. Web, Telegram, the worker and AI tools all use domain commands.                                            | mutation, action (in code)                       |
| query            | A read operation that applies permissions.                                                                                                        |                                                  |
| port             | An interface that the application defines and an adapter implements.                                                                              |                                                  |
| adapter          | The implementation of a port for one technology, for example S3 or Telegram.                                                                      | driver                                           |
| connector        | An adapter that brings data from an external system into tada.                                                                                    | integration (in code)                            |
| code module      | A Rust module for one bounded context inside the `domain` and `app` crates.                                                                       |                                                  |
| worker           | The process role that runs jobs and schedules.                                                                                                    |                                                  |
| job              | A durable unit of background work with a versioned payload.                                                                                       | task (in code)                                   |
| actor            | The record of who did a change, for the audit log: a member, a service identity, or AI with its principal. Code never checks permissions with it. | user (in code)                                   |
| audit subject    | The member whom an audit event is about, for example the member whose event role changed (ADR 0061).                                              | target, affected user                            |
| service identity | A named identity of tada that acts without a member: `job-runner`, `ai-pm`, `telegram-gateway`, `bootstrap` or `exporter`.                        | system user, worker                              |
| caller           | The typed value that a command takes for authorization: member, service identity or AI.                                                           | actor (for permissions)                          |
| principal        | The member or service identity for which AI acts. Never AI itself.                                                                                |                                                  |
| process role     | The kind of process that the `tada` binary runs: `serve`, `worker` or `telegram`.                                                                 | role (alone; it also means an organization role) |
| channel          | The way a call reaches tada: web, Telegram, job, API token or command line.                                                                       |                                                  |
| event-local ID   | The short ID of a record inside its event, for example `ACT-042`.                                                                                 | number, key                                      |
| problem code     | The stable code of an error response, for example `record-version-conflict`.                                                                      | error message                                    |
| MCP server       | The `/mcp` endpoint of tada. AI clients of members use it to read data and create proposals.                                                      | AI API                                           |
| API token        | A personal token with the prefix `tada_pat_`, bound to one member and one organization, with the scope `read` or `propose`.                       | API key                                          |
| token notice     | The text that a member confirms before the member creates an API token (ADR 0045). Each change of the text gets a new notice version.             |                                                  |
| feature flag     | A switch of one organization, stored as a row, that an owner changes with an audit event, for example `mcp-tokens` (ADR 0036).                    | setting, toggle                                  |
| outbound intent  | A stored record of a message that tada will send, written in the same transaction as the change that causes it.                                   | queue entry                                      |
| inbound delivery | One received mail message from one inbound adapter, with a delivery key that is unique for the adapter (ADR 0057).                                |                                                  |
| webhook dialect  | The provider-specific part of the inbound webhook adapter: it reads the message ID and fetches the raw message (ADR 0057).                        |                                                  |
| cursor           | An opaque value that a list response returns to get the next page. Clients never build it.                                                        | offset, page number                              |

## Design

| Term               | Meaning                                                                                                    | Avoid                         |
| ------------------ | ---------------------------------------------------------------------------------------------------------- | ----------------------------- |
| design token       | A named value for color, type, spacing, radius, elevation, motion or z-index. Components use only tokens.  | variable, style constant      |
| state of knowledge | The status of a value: accepted, proposed, assumption, unknown or conflict. Each value in the UI shows it. | confidence, certainty         |
| density            | The spacing setting of the UI: compact or comfortable.                                                     | zoom, size mode               |
| app shell          | The frame around all pages: navigation, top bar and the evidence panel area.                               | layout, chrome                |
| evidence panel     | The panel that shows the provenance of the selected value.                                                 | sources view, citations panel |
| sheet              | A panel that slides in from the right or the bottom over the page.                                         | drawer, side panel            |
| dialog             | A modal window that blocks the page until the member answers.                                              | popup, modal (in UI text)     |
| command menu       | The search and action menu that opens with `Ctrl+K`.                                                       | command palette               |
| component gallery  | The development-only route `/_gallery` that shows all tokens and components.                               | style guide, storybook        |
