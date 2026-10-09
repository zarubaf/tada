# 0017. HTTP API contract from Rust types

- Status: Accepted
- Date: 2026-10-06
- Supersedes: [0004](0004-http-api-contract.md)
- See also: [0064](0064-one-proposal-input-for-http-and-mcp.md) (proposed): the proposal input types of `app` as request schemas.

## Context

ADR 0004 defined the API contract with Hono and Zod in TypeScript.
ADR 0003 moves the backend to Rust.
The rules of ADR 0004 for versions and stability stay valid; only the tools change.

## Decision

- The `api` crate uses `axum` for HTTP, and `utoipa` with `utoipa-axum` for the OpenAPI document. `utoipa-axum` registers routes and their documentation together, so they cannot drift.
- `Cargo.toml` pins `utoipa` and `utoipa-axum`; `mise.toml` pins `oasdiff`.
- Request and response types (DTOs) live only in the `api` crate. They are the single definition of the contract.
- The `api` crate maps DTOs to and from domain types. The `domain` crate never derives `ToSchema`, so a domain refactor cannot change the contract by accident.
- The command `tada openapi` writes `contracts/openapi.json`. The file is committed.
- A CI check fails if the committed file differs from the generated file.
- All routes use the prefix `/api/v1`.
- In v1, we only add: new routes, new optional request fields and new response fields.
- CI compares `openapi.json` with `main` through `oasdiff` and fails on a breaking change.
- A breaking change needs a new ADR, a `!` commit and a new version prefix.
- A new value in a response enum is a breaking change, unless the contract marks the enum as open and clients handle unknown values.
- The web client uses types from `openapi-typescript` and calls the API with `openapi-fetch`.
- A spike in the walking skeleton checks that `oasdiff` and `openapi-typescript` handle the real `utoipa` output.
- HTTP handlers only translate HTTP to `app` commands and queries. They contain no domain rules.
- Error responses use one format for all routes: RFC 9457 problem details.

## Consequences

- The OpenAPI file shows each contract change in review.
- A client never depends on a field that is not in the contract.
- An external service in any language can generate a client from the same file.

## Alternatives

- A hand-written OpenAPI file: two definitions of each shape that can disagree.
- `aide`: similar to `utoipa`, but with fewer users.
- GraphQL or gRPC: more complexity than the use cases need, and harder to use from a browser.
