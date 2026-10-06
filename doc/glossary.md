# Glossary

This glossary gives each tada term exactly one meaning.
Docs, code identifiers, API names and UI keys use these terms.
If a term is missing, add it here in the same commit that uses it.

The "Avoid" column lists words that have a different meaning or no fixed meaning.
"(open)" marks a term that still needs a decision.

## Organization and people

| Term                      | Meaning                                                                                                           | Avoid                          |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| organization              | The tenant: a club or association. It owns all its data. Data never crosses an organization boundary.             | club (in code), tenant (in UI) |
| user                      | A person with a stable internal UUID. Email and Telegram are linked credentials, not identities.                  | account                        |
| member                    | A user with an organization membership and one organization role: owner, admin or member.                         |                                |
| external identity         | A credential linked to a user, for example a Telegram user ID or an email address.                                |                                |
| organizing committee (OK) | The Organisationskomitee of one event: the people who plan it. Write "OK" only after you define it in a document. | committee                      |
| project manager (PM)      | The person who coordinates an event and makes the final decisions.                                                |                                |
| workstream                | One area of work in an event, for example catering or ground operations.                                          | team, department               |
| workstream lead           | The member who owns a workstream and reviews its proposals.                                                       |                                |
| volunteer                 | A person who receives assignments for an event.                                                                   | helper                         |
| supplier                  | An external party that provides goods or services. Suppliers have no access to internal records.                  | vendor                         |

## Events

| Term          | Meaning                                                                                                                         | Avoid              |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| event series  | A recurring kind of event, for example the annual fly-in.                                                                       |                    |
| event         | One occurrence of an event, with its own dates and accepted state.                                                              | occurrence (in UI) |
| template      | A versioned set of checklists, roles and relative deadlines for an event series. A template never copies approvals or evidence. |                    |
| module        | A domain feature that an event enables, for example aviation. Do not confuse with a code module.                                | plugin             |
| event profile | The facts, assumptions, unknowns and open questions of an event.                                                                |                    |
| fact          | A value that a member accepted, with its evidence.                                                                              |                    |
| assumption    | A value that the team uses for planning but did not confirm.                                                                    |                    |
| unknown       | A value that nobody knows yet. tada shows it as unknown and never fills it in.                                                  |                    |
| open question | A question that the team must answer. It has an owner.                                                                          |                    |

## Work

| Term        | Meaning                                                                                                 | Avoid     |
| ----------- | ------------------------------------------------------------------------------------------------------- | --------- |
| action      | A piece of work with one owner, a status and a due date. (open: the UI can call it "task")              | todo      |
| milestone   | A date by which a set of actions must be complete.                                                      | deadline  |
| commitment  | A promise from a person or a supplier, with its conditions. A conditional commitment stays conditional. | agreement |
| decision    | An approved statement with its exact wording, approver and evidence.                                    |           |
| risk        | A possible problem with a likelihood, an impact and an owner.                                           | issue     |
| requirement | A condition that the event must meet, for example from an authority.                                    |           |
| resource    | A shared item, for example radios or a generator.                                                       | asset     |
| reservation | A booking of a resource for a period.                                                                   |           |
| assignment  | A volunteer allocated to an action or a shift.                                                          |           |

## Evidence and review

| Term           | Meaning                                                                                                                              | Avoid                    |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------ |
| accepted state | The records that a member with the correct authority accepted. Only domain commands change it.                                       | truth, master data       |
| record version | A number that increases with each change of a record. Commands use it for optimistic concurrency.                                    | revision                 |
| source item    | An incoming item: a mail message, a document or a Telegram message.                                                                  |                          |
| source version | One immutable version of a source item, with its hash and capture time.                                                              |                          |
| evidence link  | A link from a record to an exact location in a source version.                                                                       | citation (in code)       |
| provenance     | The set of evidence links and fact versions behind a record or a generated draft.                                                    |                          |
| proposal       | A suggested change to accepted state, with its source, its author and the target record version. A proposal is never accepted state. | suggestion               |
| review         | The act in which the owner accepts, edits or rejects a proposal. Silence is never acceptance.                                        | approval (of a proposal) |
| conflict       | The state of a proposal when its target record changed after the proposal was created.                                               |                          |

## Documents

| Term             | Meaning                                                                       | Avoid          |
| ---------------- | ----------------------------------------------------------------------------- | -------------- |
| document         | A file with a stable tada ID. The ID does not change with its name or folder. | file (in code) |
| document version | One immutable upload or generated draft of a document.                        |                |
| draft            | A document version that nobody approved.                                      |                |
| approved version | A document version that a member approved. Nobody can overwrite it.           | final          |

## AI and automation

| Term       | Meaning                                                                                                                                   | Avoid        |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| AI PM      | The scheduled service that checks progress, sends reminders and escalates exceptions. It is not a person and has no authority of its own. | bot, agent   |
| bot        | The Telegram bot: one channel of tada.                                                                                                    |              |
| reminder   | A message from the AI PM to an owner about an action.                                                                                     | notification |
| escalation | A message to a workstream lead or the PM about an unresolved item.                                                                        |              |
| policy     | A configured rule that permits an automatic action.                                                                                       |              |

## Architecture

| Term           | Meaning                                                                                                | Avoid                      |
| -------------- | ------------------------------------------------------------------------------------------------------ | -------------------------- |
| domain command | The only way to change accepted state. Web, Telegram, the worker and AI tools all use domain commands. | mutation, action (in code) |
| query          | A read operation that applies permissions.                                                             |                            |
| port           | An interface that the application defines and an adapter implements.                                   |                            |
| adapter        | The implementation of a port for one technology, for example S3 or Telegram.                           | driver                     |
| connector      | An adapter that brings data from an external system into tada.                                         | integration (in code)      |
| code module    | A Rust module for one bounded context inside the `domain` and `app` crates.                            |                            |
| worker         | The background process that runs jobs and schedules.                                                   |                            |
| job            | A durable unit of background work with a versioned payload.                                            | task (in code)             |
