# 0007. Durable jobs and schedules in PostgreSQL

- Status: Proposed
- Date: 2026-10-06

## Context

A domain command must commit the change, the audit event and any outbound job in one transaction.
The AI PM needs schedules that continue after a restart.
A restart must not flood users with old reminders.
The Rust job libraries are less mature than Graphile Worker for TypeScript.

## Decision

- The `app` crate defines a `JobQueue` port. Jobs live in the tada PostgreSQL database.
- A domain command adds jobs inside its own database transaction.
- Workers claim jobs with `SELECT ... FOR UPDATE SKIP LOCKED`, with a lease, retries and a backoff.
- Each job payload has a `version` field. A handler accepts all versions that can still be in the queue.
- Schedules are rows with a next run time and an explicit limit for missed runs.
- Outbound messages store an intent before the send, and record the result as sent, failed or unknown.
- A spike in the walking skeleton tests `apalis` with its PostgreSQL backend.
  We use `apalis` only if it can add a job inside the caller's transaction and supports the other points above.
  Otherwise, `store-pg` implements the queue itself.

## Consequences

- No message broker to operate.
- A rolled-back command never leaves an orphan job.
- An own queue is a few hundred lines that we must test and maintain.
- Job throughput is limited by PostgreSQL. This is sufficient for club events.

## Alternatives

- `underway`: the closest match, but too few users and no recent release.
- Redis or a message broker: one more service and no shared transaction with the domain data.
