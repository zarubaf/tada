# 0035. Observability without personal data

- Status: Proposed
- Date: 2026-10-06

## Context

The platform contract (ADR 0025) sends logs to standard output as JSON lines without personal data.
An operator must find the cause of a failure from the logs alone.
"No personal data" needs a mechanism, not only a rule, because a single `{:?}` can leak an email address.
Alerts and dashboards belong to each operator's deployment repository (ADR 0033).
The audit log is a domain record (see [ARCHITECTURE.md](../ARCHITECTURE.md)). It is not a log stream.

## Decision

Logs:

- The app uses `tracing` with the JSON formatter of `tracing-subscriber`.
- Each line has these fields: `timestamp` (UTC, RFC 3339), `level`, `target`, `message`, `role` and, where known, `request_id`, `job_id`, `organization_id` and `event_id`.
- `TADA_LOG` sets the level filter. The default is `info`.

Personal data:

- IDs (UUIDs) can go into logs. Names, email addresses, Telegram names, message text, document content and tokens never go into logs.
- Types that hold personal data or secrets, for example `Email`, `DisplayName` and `SecretString`, implement `Debug` with a redacted value. They do not implement `Display` for logs.
- A test fails if a log line from the integration tests contains an email address or a fixture name.

Correlation:

- `serve` takes `X-Request-Id` from a trusted proxy (`TADA_TRUSTED_PROXIES`) or generates a UUIDv7.
- The response returns the request ID. Error responses contain it (ADR 0037).
- A job stores the request ID of the command that created it, so that one ID connects the request, the job and the send.

Health and metrics:

- `/healthz` and `/readyz` work as ADR 0025 describes.
- The `worker` heartbeat row contains the time of the last loop, the queue depth and the age of the oldest due job.
- A metrics endpoint in Prometheus text format listens on `TADA_METRICS_PORT`, on a separate port from the API. It contains:
  - HTTP requests by route, status class and duration,
  - jobs by type and result, and the queue age,
  - outbound sends by channel and result (sent, failed, unknown),
  - model calls and tokens by event,
  - the time of the last successful sync of each connector.
- Metric labels never contain personal data or user IDs.

Not now:

- No distributed tracing export. The spans use `tracing`, so an OpenTelemetry exporter can come later through configuration.
- No error tracking service. The logs and the request ID are enough at club scale.

## Consequences

- The operator's deployment decides where logs and metrics go, and which alerts exist.
- A developer cannot log an email address by accident through `Debug`.
- One request ID connects an error message in the UI with the log lines of all roles.

## Alternatives

- Free-text logs: hard to search, and no fields for correlation.
- OpenTelemetry from the start: more dependencies and a collector to operate, with no current need.
- Sentry or a similar service: one more processor of personal data under revDSG.
