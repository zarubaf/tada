# 0076. Schedules and reminders

- Status: Proposed
- Date: 2026-10-11

## Context

The AI PM sends each verified member who opted in one private Telegram digest of due, overdue and blocked work ([ADR 0011](0011-telegram.md), [ADR 0068](0068-actions-and-commitments.md)).
[ADR 0054](0054-job-queue-implementation.md) decides that schedules are own rows with a local time, a time zone and a limit for missed runs.
No such table exists yet.
The four daily sweeps run on in-memory timers.
The acceptance criteria are:

- Quiet hours and cooldowns survive a restart.
- A changed deadline or a completed action cancels obsolete reminders.
- A stopped worker does not send a flood of missed reminders.
- An ambiguous send is never retried without a check ([ADR 0011](0011-telegram.md)).
- Only the owner of a record can change it with a button.

## Decision

Schedules:

- The table `schedule` holds the job kind to start, a local time, an IANA time zone, the weekdays, `next_run_at`, `last_run_at` and `missed_run_limit`.
  The pair organization and kind is unique.
  The organization is optional, for later system schedules.
- `next_run_at` follows [ADR 0038](0038-ids-and-time.md): a gap of a DST change moves the run forward, and a fold runs it one time.
- A step of the worker loop claims due rows with `FOR UPDATE SKIP LOCKED`.
  In one transaction it adds the job, writes `next_run_at` and writes `last_run_at`.
- The database clock gives the current time.
- The step starts at most `missed_run_limit` jobs for one row.
  Older missed runs only move `last_run_at`.
  The `pm-check` schedule has the limit 1.
- The command that creates or changes the policy writes the `pm-check` row in its own transaction.
- The four existing sweeps stay on in-memory timers.
  A follow-up moves them to schedule rows.

Policy:

- One policy for each organization, edited by an organization owner or admin with `expected_version`.
- Defaults:
  - The check runs at 08:30 Europe/Zurich, Monday to Friday.
  - Quiet hours are 20:00 to 08:00 in the local time of the recipient.
  - The cooldown is 20 hours for each person.
    The range is 1 to 168 hours.
  - The follow-up comes after 2 working days.
    The range is 1 to 10.
  - The look-ahead is 2 working days.
    The range is 0 to 10.
  - The policy is not paused, and AI wording is off.
- Working days are Monday to Friday.
  There is no holiday calendar.
- A pause stops new checks and cancels pending sends at send time.
- A member opts in and sets a time zone on the own account.
  Both belong to the user, not to the membership.
  A user in two organizations gets two digests.
- An eligible recipient is a current member who opted in, has a confirmed Telegram link that is not marked blocked, in an organization whose policy is not paused.

Selection:

- Pure functions in the `domain` crate select the items.
  They have table tests.
- Items are actions in `open`, `in-progress` or `blocked` and commitments in `conditional` or `firm`, with the person as owner.
  An item is due if it is overdue, due inside the look-ahead, or an action that is `blocked`.
- The table `reminder_state` holds the version and the time of the last notice and the response of each person to each record.
- An item is included if it is new or its version is not the notified version.
- An item is included again as a follow-up if the version is the same, no response came and the follow-up period passed.
- An item with a response for its current version is skipped, unless it became overdue after the response.
- A person with a digest inside the cooldown gets no new digest.
- The tables `reminder_state` and `digest_item` hold no free text.
  The housekeeping of [ADR 0071](0071-event-closing-and-retention.md) deletes them with their intent after 90 days.

Digest intent and send:

- The `pm-check` job stores the intent, its items and a send job in one transaction.
  The purpose of the intent is `telegram-digest`.
  The send time is the end of the quiet hours of the recipient when the check falls inside them.
- A digest has at most 10 items.
  The rest is "and N more" with a link to My Work.
- Each item has an opaque reference of 16 random bytes.
  The reason is `overdue`, `due-soon`, `blocked` or `follow-up`.
- The send job locks the intent and stops if it is not `pending`.
- It reads each item again and removes it if its version changed, the record is closed, the owner changed or the recipient is no longer eligible.
  If no item stays, the intent becomes `canceled`.
- The intent goes from `pending` to `unknown` before the Bot API call, as the mail send does ([ADR 0011](0011-telegram.md)).
  A crash between the call and `finish` cannot send the digest twice.
- After a send, the result updates `reminder_state` for each item.
  An `unknown` result updates it too and is not retried.
  A 403 sets `blocked_at` on the Telegram link.
  Another failure changes no state.
- The text is plain text without a parse mode.
  A title cannot become a link or markup.
- The worker sends through a `Messenger` port that the `telegram` crate implements.
  The `telegram` role only receives updates.

Buttons:

- An action has the buttons On track, Blocked, Done and Open.
  A commitment has On track, Fulfilled and Open.
  Open is a URL button to the record.
- A button carries an answer code and the base64 reference of the item, at most 64 bytes.
- A button works for 7 days after the send.
- A press changes nothing unless all checks pass:
  - The Telegram user has a confirmed link.
  - The user is a current member.
  - The user may still change the record.
  - The record version equals the version of the item.
  - The send is less than 7 days old.
- On a failed check the bot answers "outdated, open the record".
  It always answers the callback query.
- A press writes the response.
  Blocked, Done and Fulfilled also run the existing command, with the capability of the member, in the same transaction.
- Only private chats are served.
  Group updates stay ignored.

Telegram webhook and health:

- `TADA_TELEGRAM_MODE` is `polling` or `webhook`.
  The default is `polling`.
- In webhook mode, the role calls `setWebhook` at start with a secret token and the allowed updates `message` and `callback_query`.
  The URL comes from `TADA_PUBLIC_URL`.
- `POST /telegram/webhook` compares `X-Telegram-Bot-Api-Secret-Token` with `subtle` in constant time, limits the body size and keeps the update dedupe.
  The secret comes from `TADA_TELEGRAM_WEBHOOK_SECRET_FILE`.
  It is never logged.
- Both modes start an axum listener on `TADA_TELEGRAM_LISTEN` (default `0.0.0.0:8081`) with `/healthz` and `/readyz`.
- `/readyz` checks the database, the object storage and the last successful Bot API call.
  It fails if no call succeeded for 5 minutes.
- The role stops within 25 seconds on SIGTERM.
- The `serve` and `telegram` roles write a heartbeat with their role, version and commit only.
  Only the `worker` row holds the queue depth ([ADR 0035](0035-observability.md)).
  An operator check of the worker filters `role = 'worker'`.

Dependencies:

- The new direct dependency `subtle` compares the webhook secret in constant time.

## Consequences

- Schedules and reminder state are in the database, so a restart loses nothing.
- Two workers never start the same run.
- A stopped worker sends at most one check for each schedule after it returns.
- The check at plan time and the check at send time make a changed deadline or a done action cancel the reminder without hooks in the work commands.
- A holiday gets a reminder until a calendar exists.
- A recipient in another time zone than the policy gets the send at the own 08:00 local time.
- Escalation, the PM brief, meeting agendas, free-text replies and new dates are later work.

## Alternatives

- One reminder job for each record: the work commands would have to create and cancel jobs, and this couples them to the reminders.
- A cron library: the library cannot keep the local time and the time zone of [ADR 0038](0038-ids-and-time.md) and [ADR 0054](0054-job-queue-implementation.md).
- Check only at plan time: a deadline or status change between the plan and the send would still send an obsolete reminder.
