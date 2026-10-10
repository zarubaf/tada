# 0071. Event closing, retention and housekeeping

- Status: Proposed
- Date: 2026-10-10

## Context

[ADR 0045](0045-data-protection.md) requires that each organization sets retention periods for sources and for closed events, and that a job deletes data after the period.
tada has no closed state for an event yet.
Without it, the job cannot tell a finished event from an event in work.
Technical rows, such as old sign-in links and sessions, hold personal data too (an email address, a user agent).
They need a period as well.
A club can have an accounting duty (OR Art. 958f, 10 years) for invoices and contracts, which sources can contain.
A default period would delete such data without the club's decision.

## Decision

Event closing:

- An event has a status: `active` or `closed`. It also has `closed_at` and `redacted_at`.
- An event manager, an organization owner or an organization admin closes an event.
- An organization owner or admin reopens it. A manager cannot reopen.
- An event with `redacted_at` set cannot be reopened. The command returns `invalid-transition`.
- A closed event is read-only for every role. This covers direct commands, proposals into the event, the apply of its open proposals, uploads and drafts.
- Each of these refuses with the problem code `event-closed` (409).
- The check lives in one place. `access::event_access` returns read-only rights for a closed event, and each command that writes calls `event_writable` once.
- The audit log records `event.close` and `event.reopen`.
- The web client shows the banner „Abgeschlossen am …“ and hides the write controls of a closed event.

Retention settings:

- An organization has two periods in months: `retention_closed_event_months` and `retention_source_months`.
- Each period is empty (off) or a whole number from 1 to 120.
- Retention is off until an owner sets a period. tada has no default.
- Only an owner changes the periods. An owner or an admin reads them. The audit log records `organization.set_retention`.
- The audit detail of this event holds the two periods as numbers or null, and no other content.
  A migration widens the `CHECK` constraint of the detail for these two keys.
- The settings page names the accounting duty, so that the owner sets a period with this duty in mind.

Retention job:

- The worker runs the job once in 24 hours, in the pattern of the sweep in [ADR 0065](0065-worker-deletes-ended-rate-limit-counters.md).
- For each organization with a period, it redacts each closed event whose `closed_at` is older than the period and whose `redacted_at` is empty.
  The redaction keeps the structure of the event: IDs, statuses, dates and the audit log.
  [ADR 0072](0072-legal-redaction-and-deletion-journal.md) defines the redaction and the content that it removes.
- It redacts each source item that belongs to no event and is older than the source period.
  A source of an event follows the period of the event.
- Each event and each source item is one transaction. The job is idempotent and continues after a crash.
- The job never touches an active event or a source of an active event.
  A source of an active event belongs to the event, or the evidence of the event cites it: an evidence link, proposal evidence or record evidence.
  A cited source becomes due when the event closes.
- Persons and institutions have no automatic retention. They are the contact directory of the club, and an owner redacts them on request.

Housekeeping:

- The worker deletes technical rows once in 24 hours. Each rule is one SQL statement.
- 30 days after the end: expired or used sign-in links, ended sessions and open Telegram claims.
- 90 days after the end: finished invitations, sent or failed mail intents and failed jobs.
- A user account is global across organizations, so no organization owner can erase it.
  The job anonymizes an account 90 days after its last organization membership ended.
  The display name becomes the tombstone, and the email identity, the Telegram identity, the sessions, the sign-in links and the API tokens are deleted.
- The data inventory states each period.

## Consequences

- A club decides its periods. A new installation deletes nothing until an owner sets them.
- A closed event cannot change, so a club that finds an error must reopen it first.
- Every write path must call `event_writable`. A test covers each entry.
- The periods are constants in code. A change needs a new commit, not a setting.
- The member list of a redacted event stays, because IDs and roles carry no content.

## Alternatives

- Default periods: rejected, because a default can delete data under an accounting duty without the club's decision.
- Full deletion of events: rejected, because [ADR 0045](0045-data-protection.md) keeps tombstones so that links and manifests resolve.
- Settings for the housekeeping periods: rejected, because the periods carry no club policy and each setting is a cost to document and test.
- Retention for persons and institutions: rejected, because no event or close date gives the age of a contact.
