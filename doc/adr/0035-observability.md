# 0035. Observability without direct identifiers

- Status: Proposed
- Date: 2026-10-06

## Context

The platform contract (ADR 0025) sends logs to standard output as JSON lines.
An operator must find the cause of a failure from the logs alone.
Under revDSG, a user UUID is personal data, because the operator can link it to a person.
Logs therefore cannot be free of personal data; they can be free of direct identifiers, and their retention can be limited.
A rule alone is not enough, because a single `{:?}` can leak an email address.
Alerts and dashboards belong to each operator's deployment repository (ADR 0033).
The audit log is a domain record (see [ARCHITECTURE.md](../ARCHITECTURE.md)). It is not a log stream.

## Decision

Format:

- The app uses `tracing`. `json-subscriber` writes one JSON object per line, with span fields flattened to the top level.
  If `json-subscriber` fails the walking skeleton check, a custom `FormatEvent` of `tracing-subscriber` does the same.
- Each line has these fields: `timestamp` (UTC, RFC 3339), `level`, `target`, `message`, `process_role` and, where known, `request_id`, `job_id`, `organization_id` and `event_id`.
  A root span of each process sets `process_role`.
- `TADA_LOG` sets the level filter. The default is `info`.

Identifiers:

- Logs can contain record UUIDs, including user UUIDs.
- Logs never contain direct identifiers or content: names, email addresses, Telegram user IDs and names, IP addresses, message text, document content, tokens and secrets.
- Types that hold such values, for example `Email`, `DisplayName`, `TelegramUserId` and `SecretString`, implement `Debug` with a redacted value. They do not implement `Display` for logs.
- HTTP logs contain the matched route template, for example `/api/v1/events/{event_id}`, never the raw path or the query. A path can contain a token (ADR 0008).
- A test fails if a log line from the integration tests contains an email address, a fixture name or a token.
- The platform contract asks each operator for a log retention limit (ADR 0025).

Correlation:

- `serve` accepts `X-Request-Id` from a trusted proxy (`TADA_TRUSTED_PROXIES`) only if the value is a UUID. Otherwise it generates a UUIDv7.
- The response returns the request ID. Error responses contain it (ADR 0037).
- A job stores the request ID of the command that created it, so that one ID connects the request, the job and the send.

Health:

- `/healthz` and `/readyz` work as ADR 0025 describes.
- The `worker` heartbeat row contains the time of the last loop, the queue depth and the age of the oldest due job.
- Queue state, send results, model usage and connector sync times are already database records (ADRs 0007 and 0010). The product shows them; no separate metrics system repeats them.

Not now:

- No metrics endpoint. We add one with an ADR when an operator needs it.
- No distributed tracing export. The spans use `tracing`, so an OpenTelemetry exporter can come later.
- No error tracking service. The logs and the request ID are enough at club scale.

## Consequences

- A developer cannot log an email address or a token by accident through `Debug` or a request path.
- One request ID connects an error message in the UI with the log lines of all process roles.
- The operator's deployment decides where logs go, how long they stay and which alerts exist.

## Alternatives

- The plain JSON formatter of `tracing-subscriber`: it nests span fields, so `request_id` is not a top-level field.
- A Prometheus endpoint now: it repeats data that the database already holds, and no operator needs it yet.
- OpenTelemetry from the start: more dependencies and a collector to operate.
- Sentry or a similar service: one more processor of personal data under revDSG.
