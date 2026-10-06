# 0025. Platform contract between the app and its runtime

- Status: Accepted
- Date: 2026-10-06

## Context

The first runtime is one VM with Docker Compose (ADR 0015).
Later, tada can run on Kubernetes, Google Cloud Run or a managed platform.
If the app code knows about Compose, Caddy, Cloudflare or host paths, each move needs code changes.
A small, written contract lets each runtime run the same image without changes.

## Decision

The `tada` image follows this contract on every runtime:

Artifact:

- One OCI image for all roles. The command selects the role: `tada serve`, `tada worker`, `tada telegram`, `tada migrate`.
- The image runs as a non-root user and needs no write access to its file system, except `/tmp`.

Configuration:

- Settings come from environment variables with the prefix `TADA_`.
- A secret comes from a file. The variable `TADA_<NAME>_FILE` gives the path. The app never reads a secret from a plain variable.
- The app checks all settings at startup and stops with a clear error if one is missing or invalid.
- PostgreSQL comes from `TADA_DATABASE_URL`. Object storage comes from an S3 endpoint, a bucket name and credentials.
- `TADA_PUBLIC_URL` gives the external base URL. The app builds magic links and webhook URLs only from it.
- `TADA_TRUSTED_PROXIES` lists the network ranges whose `X-Forwarded-For` header the app trusts (ADR 0008).

Processes:

- Processes are stateless. All state is in PostgreSQL and object storage.
- `serve` listens on the port in `TADA_PORT`. It does not handle TLS.
- `serve` and `telegram` answer `GET /healthz` (the process runs) and `GET /readyz` (the database and the object storage respond).
- `worker` writes a heartbeat to the database. An operator check reads it.
- On `SIGTERM`, a process stops accepting new work, completes or releases its current work, and stops within 25 seconds.
- `tada migrate` runs once and stops. It is the only role that changes the schema (ADR 0006).

Logs and metrics:

- Logs go to standard output as JSON lines, one event per line, without personal data.
- The app writes no log files.

Forbidden in app code:

- Host paths, container names, Compose service names and Docker socket access.
- Code that depends on a specific proxy, CDN or cloud provider.
- Assumptions about the number of replicas. Two `serve` processes must work together.

## Consequences

- Compose, Kubernetes and Cloud Run all start the same image with different manifests only.
- The deployment files in `deploy/` hold all knowledge about the runtime.
- Each new setting needs an entry in the settings reference and a startup check.

## Alternatives

- A configuration file in the image: each environment needs its own image.
- Secrets in environment variables: they leak through `docker inspect`, crash reports and child processes (ADR 0016).
- Platform-specific code, for example Cloud Run metadata calls: a lock-in that the product owner wants to avoid.
