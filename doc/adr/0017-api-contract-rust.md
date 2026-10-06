# 0017. HTTP API contract from Rust types

- Status: Proposed
- Date: 2026-10-06
- Supersedes: [0004](0004-http-api-contract.md)

## Context

ADR 0004 defined the API contract with Hono and Zod in TypeScript.
ADR 0003 moves the backend to Rust.
The rules of ADR 0004 for versions and stability stay valid; only the tools change.

## Decision

- The `api` crate uses `axum` for HTTP and `utoipa` for the OpenAPI document.
- Rust types with `serde` and `utoipa` derives are the single definition of each request and response.
- The command `tada openapi` writes `contracts/openapi.json`. The file is committed.
- A CI check fails if the committed file differs from the generated file.
- All routes use the prefix `/api/v1`.
- In v1, we only add: new routes, new optional request fields and new response fields.
- CI compares `openapi.json` with `main` through `oasdiff` and fails on a breaking change.
- A breaking change needs a new ADR, a `!` commit and a new version prefix.
- The web client uses TypeScript types and a client that the build generates from `openapi.json`.
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
