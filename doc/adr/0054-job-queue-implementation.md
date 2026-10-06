# 0054. Job queue in store-pg

- Status: Proposed
- Date: 2026-10-06

## Context

ADR 0007 asks for a spike of the Rust `graphile_worker` crate and of `apalis` (1.0 release candidate) with PostgreSQL.
The hard criterion: a command can add a job inside its own `sqlx` transaction.
ADR 0006 adds three rules: only `tada migrate` changes the schema, we write and review each migration file, and the previous image still starts after an expand migration.
ADR 0038 adds a rule for schedules: they store a local time and a time zone.

The spike ran on 2026-10-06 with `sqlx` 0.9 and PostgreSQL 18.6:

| Point                              | `graphile_worker` 0.13.6                                                                     | `apalis-postgres` 1.0.0-rc.9                                                              |
| ---------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `sqlx` version                     | 0.9                                                                                          | 0.9                                                                                       |
| Job inside the caller transaction  | Yes: `WorkerUtils::with_executor(&mut tx)`. A rollback removes the job, a commit keeps it.   | No: the insert function is in a private module. Only `PostgresStorage` with its own pool. |
| Claims, retries, backoff, recovery | Yes: `SKIP LOCKED`, exponential backoff, a worker heartbeat and a sweep of stale workers     | Not tested, because the hard criterion failed                                             |
| Schema changes                     | `WorkerOptions::init()` always applies the library migrations when a worker starts           | Not tested                                                                                |
| Older binary after a newer schema  | The worker stops if the schema has a breaking revision that the binary does not know         | Not tested                                                                                |
| Schedules                          | Cron schedules, without the local time and time zone of ADR 0038                             | Not tested                                                                                |
| Size                               | 268 crates in its dependency tree, including `chrono`; 0.x releases, the last one on the day | 1.0 release candidate                                                                     |

## Decision

- `store-pg` implements the job queue of ADR 0007 itself, behind the `JobQueue` port of the `app` crate.
- The queue tables are in the reviewed migrations of `store-pg` (ADR 0006).
- A command adds a job with the transaction of the command.
- Workers claim jobs with `SELECT ... FOR UPDATE SKIP LOCKED` and a lease. The database clock gives due times and leases (ADR 0038).
- Schedules are own rows with a local time, a time zone and a limit for missed runs (ADRs 0007 and 0038).
- We test the queue with the PostgreSQL containers of the other `store-pg` tests: a rolled-back command leaves no job, two workers never run the same job, and an expired lease returns the job to the queue.

## Consequences

- No library changes the schema outside `tada migrate`, and a deploy can still roll back by image after an expand migration.
- We own a few hundred lines of queue code and its tests (ADR 0007 expected this).
- The dependency tree stays smaller, and no second time library comes in.

## Alternatives

- `graphile_worker`: it passes the hard criterion and has good recovery. But it migrates its own schema at worker startup, which contradicts ADR 0006, and its schedules do not follow ADR 0038. A fork or a wrapper that skips `init()` would depend on internal behavior.
- `apalis`: it fails the hard criterion in 1.0.0-rc.9.
