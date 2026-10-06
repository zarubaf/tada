# 0007. Durable jobs and schedules in PostgreSQL

- Status: Proposed
- Date: 2026-10-06

## Context

A domain command must commit the change, the audit event and any outbound job in one transaction.
The AI PM needs schedules that continue after a restart.
A restart must not flood users with old reminders.

## Decision

- We use Graphile Worker. It stores jobs in the same PostgreSQL database.
- A domain command adds jobs inside its own transaction.
- Each job payload has a `version` field. A job handler accepts all versions that can still be in the queue.
- Cron schedules use Graphile Worker's cron feature, with an explicit backfill limit.
- Outbound messages store an intent before the send, and record the result as sent, failed or unknown.

## Consequences

- No message broker to operate.
- A rolled-back command never leaves an orphan job.
- Job throughput is limited by PostgreSQL. This is sufficient for club events.

## Alternatives

- pg-boss: also good; Graphile Worker has a simpler transactional insert and built-in cron.
- Redis with BullMQ: one more service and no shared transaction with the domain data.
