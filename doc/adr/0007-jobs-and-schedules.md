# 0007. Durable jobs and schedules in PostgreSQL

- Status: Proposed
- Date: 2026-10-06

## Context

A domain command must commit the change, the audit event and any outbound job in one transaction.
The AI PM needs schedules that continue after a restart.
A restart must not flood users with old reminders.
The Rust job libraries are younger than their TypeScript equivalents.

## Decision

- The `app` crate defines a `JobQueue` port. Jobs live in the tada PostgreSQL database.
- A domain command adds jobs inside its own database transaction.
- Workers claim jobs with `SELECT ... FOR UPDATE SKIP LOCKED`, with a lease, retries and a backoff.
- Each job payload has a `version` field. A handler accepts all versions that can still be in the queue.
- Schedules are rows with a next run time and an explicit limit for missed runs.
- Outbound messages store an intent before the send, and record the result as sent, failed or unknown.
- A spike of at most two days in the walking skeleton tests the Rust `graphile_worker` crate and `apalis` (1.0 release candidate) with PostgreSQL.
  The hard criterion: a command can add a job inside its own `sqlx` transaction.
  We use a library only if it meets this criterion and the other points above.
  Otherwise, `store-pg` implements the queue itself.

## Consequences

- No message broker to operate.
- A rolled-back command never leaves an orphan job.
- An own queue is a few hundred lines that we must test and maintain.
- Job throughput is limited by PostgreSQL. This is sufficient for club events.

## Alternatives

- `underway`: close to our needs, but few users and no release since July 2025.
- Redis or a message broker: one more service and no shared transaction with the domain data.
