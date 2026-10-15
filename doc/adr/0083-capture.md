# 0083. Capture

- Status: Proposed
- Date: 2026-10-15
- Amends: [0011](0011-telegram.md), [0067](0067-workstreams-and-review-routing.md), [0077](0077-agent-runner.md)

## Context

tada is a tracker that should fill itself from unstructured content.
Today text reaches tada only as the source text of a changeset.
Telegram handles only `/vorschlag` and link codes.
The agent runner has one task, the daily digest.
A member who leaves a meeting with notes must type each record by hand.

A spike on 2026-10-13 tested a capture run with invented dumps (minutes, mail, chat messages) and a context of 110 and 300 actions.
Results:

- A Sonnet-class model made a usable proposal for 23 of 24 dumps.
  A Haiku-class model made one for 21 of 24.
- No answer guessed an owner, and no run followed a hostile instruction in the text.
- An agent with read tools missed records, because exact-word search did not find them.
  The same model with the whole event as context found the right target for all 24 dumps.
- A capture costs about USD 0.04 to 0.10 at 300 actions.
  Prompt caching lowers the cost.

## Decision

### Flow

1. A member sends text to the intake core with the member, the event, the channel and the text.
2. The core checks the AI cap, stores the text as a source and writes a `capture` row.
3. A worker job runs the capture task.
4. The task ends in one of four results: a changeset, nothing to record, a question for the member, or a failure.
5. The member sees one summary with the actions „Alle übernehmen“, „Details“ and „Verwerfen“.

The first channels are the private Telegram chat (text and forwarded messages) and a text box in the web client.
Groups stay ignored.
Photos, files, voice and email follow later as small adapters.
The text of a capture has at most 20,000 characters.

### Event

A capture belongs to the current event of the member.
A member with one event has no choice.
A member with several events switches with a button or a command.
If the text clearly names another event, the task asks before it proposes anything.
The sender comes from the channel and never from the text.

### Capture task

- The task gets the whole current event as a compact, cached context.
  The context holds the open and recent records with ID, title, owner, status, due date and version.
- The task has no read tools.
  Its only output is a changeset proposal.
  Hostile text can therefore only produce proposals that a person sees.
- The model class is Sonnet.
  Haiku is not good enough.
- Server-side retrieval is a later step.
  We add it when an event exceeds a few hundred open records.
- The task does not guess a person, an action or a date.
  It leaves the field empty or asks one short question.
  The answer of the member is a new capture, and the previous capture of the last 15 minutes is its context.
- A name that matches two persons is a question.

### Operations in the first slice

- create an action, with the owner from „ich“ or marked „needs owner“
- change the status, the due date and the owner of an action
- create a commitment, with a condition and a linked follow-up action
- update a commitment (text, date, condition)
- create an open question

Decisions, risks and milestones follow.
A member confirms them one at a time.

### Review

- The review card draws each proposal from its operation and never from the description of the model.
  It shows the current values next to the new values, the quote, the sender and a badge for forwarded text.
- An action or commitment without an owner cannot be accepted until the reviewer picks a person.
- The author of a changeset may accept a proposal if the author may make that change by a direct command.
  The check reuses the permission checks of the direct commands.
  This amends ADR 0067.
- One tap covers:
  - new actions owned by the sender
  - actions that need an owner
  - new open questions
  - due-date changes on an action of the sender
- The member confirms these changes one at a time:
  - status changes on actions of other members
  - anything that forwarded content causes
  - decisions and commitments
  - changes of a person or a date on an existing record
- A test matrix proves that a contributor cannot reach a manager-only change through a capture.

### Interfaces

- `POST /api/v1/events/{event_id}/captures` returns 202 and the capture ID.
- `GET /api/v1/captures/{id}` returns the state and the result.
- `POST /api/v1/captures/{id}/accept` takes all proposals that the member may take.
- `POST /api/v1/captures/{id}/discard` discards the capture.
- Plain text in a private Telegram chat becomes a capture.
  This amends ADR 0011.
- The capture task is a second task of the agent runner.
  This amends ADR 0077.

### Limits

The AI cap of the organization applies.
Turns, cost and time of a run are settings.
When the cap is reached, the bot says so and the forms still work.

## Consequences

- A member can dump notes and accept structured records with one tap.
- The agent runner gets its first task with an untrusted input.
  The missing write tool and the review rule limit the risk.
- A capture costs money for each run.
  The cap and the context size limit the cost.
- A large event can exceed the context.
  We add retrieval then.
- The product needs an evaluation set of at least 20 invented dumps.
  It runs against recorded replies in CI.
- We measure acceptance, edit and reject rates, wrong targets, misses and cost for each operation kind.

## Alternatives

- **An agent with read tools.** Exact-word search missed records.
- **A fixed extraction without the event.** It cannot find the record for a status change and makes duplicates.
- **A cheaper model class.** It made more unusable proposals.
- **Direct writes without review.** A hostile text could then change data without a person.
