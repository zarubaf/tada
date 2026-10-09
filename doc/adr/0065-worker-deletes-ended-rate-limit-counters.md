# 0065. The worker deletes ended rate-limit counters

- Status: Proposed
- Date: 2026-10-09

## Context

[ADR 0056](0056-sign-in-details.md) keeps the sign-in rate-limit counters in PostgreSQL.
Each sign-in request deletes the counters whose window ended.
ADR 0056 says that this needs no scheduled job.

The data inventory and the privacy notice template tell members that tada keeps a counter for two hours at most.
A counter is an HMAC of an email address or an IP address, so it is personal data (ADR 0045).
If no sign-in request comes after a counter, the counter stays for any time, and backups copy it.
The promise of two hours is then false.

## Decision

- The first loop of the worker in each rate-limit window deletes the counters whose window ended more than one window ago.
  The set of counters that the cleanup can delete changes only when a new window starts.
- The worker uses the same cleanup as a sign-in request.
  It keeps the previous window and skips locked rows, as ADR 0056 requires.
- Each sign-in request continues to delete ended counters.
- A failed cleanup writes a warning to the log.
  The worker records only a successful cleanup, so the next loop tries again.

The rest of ADR 0056 does not change.

## Consequences

- No counter stays longer than two hours while a worker runs.
- The data inventory and the privacy notice template state the true retention.
- The worker runs one more small `DELETE` in each window.
- If no worker runs, the cleanup again depends on the next sign-in request.

## Alternatives

- Change the texts to "until the next sign-in after two hours": simple, but tada then keeps a counter for any time.
- A separate scheduled job in the job queue: more code for the same result, and schedules come in Slice 2.
