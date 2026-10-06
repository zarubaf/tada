# 0002. Monorepo with one core and enforced boundaries

- Status: Accepted
- Date: 2026-10-06

## Context

The product owner wants one repository for all parts of tada.
Some parts, for example the Telegram gateway and the AI PM, can later run as separate services.
The domain rules must exist in one place only; web, Telegram, the AI PM and scheduled jobs must all use the same domain commands.
The team is small and the operations budget is less than CHF 100 per month.

## Decision

We use one Git repository with one Cargo workspace for the backend (ADR 0003):

```text
crates/
  domain/        types, rules and state machines; no I/O dependencies
  app/           domain commands, queries and ports (traits)
  store-pg/      PostgreSQL repositories, migrations, sessions and the job queue
  adapters/      object storage, mail and model provider adapters
  api/           HTTP handlers and the OpenAPI document
  telegram/      Telegram gateway
  tada/          the binary: serve, worker, telegram and migrate commands
apps/web/        web client in TypeScript (ADR 0005)
contracts/       the generated openapi.json (ADR 0017)
scripts/         Python scripts for checks (ADR 0034)
```

Cargo enforces the direction of dependencies, because a crate can only use the crates in its `Cargo.toml`:

1. `domain` depends on no other tada crate and on no I/O crate.
2. `app` depends on no tada crate except `domain`, and on no I/O crate.
3. Each adapter crate depends on `app` and implements its ports.
4. `api`, `telegram` and the worker call `app` commands and queries. They contain no domain rules.
5. Only the `tada` binary depends on the adapter crates. It is the composition root.
6. `apps/web` uses only the client that the build generates from `contracts/openapi.json`.

`cargo-deny` checks the rules that Cargo cannot see. Its `[bans]` section allows I/O crates such as `sqlx`, `aws-sdk-s3` and `reqwest` only as dependencies of the adapter crates.

Bounded contexts, for example `identity`, `events`, `documents`, `provenance` and `assistant`, are Rust modules inside `domain` and `app`.
Each module exports a small public interface; the rest is `pub(crate)` or private.

The binary starts one role for each process: `tada serve`, `tada worker` and `tada telegram`.
All roles use the same image and the same `app` crate.
Only these roles connect to the tada database.

A component becomes a separate network service only for a recorded reason: a different language, a separate secret, failure isolation or independent scaling.
Each split needs an ADR, and the new service then talks to the core only through the versioned API.

## Consequences

- Domain rules exist once, in `domain` and `app`.
- The compiler enforces most boundaries; `cargo-deny` covers the rest.
- A role can move to its own service later, because it already depends only on `app`.
- More crates mean more `Cargo.toml` files. We split a crate only when a boundary needs it. For example, `adapters` splits only when one adapter needs its own boundary.

## Alternatives

- One crate with modules: Rust visibility alone does not stop an adapter import in domain code.
- Microservices from the start: high operations cost, and no team needs independent deployment.
- A repository for each service: contract changes need coordinated releases.
