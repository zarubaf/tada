# 0004. HTTP API contract and stability

- Status: Accepted
- Date: 2026-10-06

## Context

The web client, the Telegram adapter, AI tools and possible future clients depend on the API.
API stability is a project goal.
A promise of stability is not enough; a check must enforce it.

## Decision

- The HTTP server uses Hono with `@hono/zod-openapi`.
- Zod schemas in `packages/contracts` are the single definition of each request and response.
- The build generates `packages/contracts/openapi.json` from these schemas. The file is committed.
- All routes use the prefix `/api/v1`.
- In v1, we only add: new routes, new optional request fields and new response fields.
- CI compares `openapi.json` with `main` through `oasdiff` and fails on a breaking change.
- A breaking change needs a new ADR, a `!` commit and a new version prefix.
- The web client uses a client that the build generates from `openapi.json`.
- HTTP handlers only translate HTTP to domain commands and queries. They contain no domain rules.

## Consequences

- The OpenAPI file shows each contract change in review.
- A client never depends on a field that is not in the contract.
- Error responses use one format (RFC 9457 problem details) for all routes.

## Alternatives

- A hand-written OpenAPI file: two definitions of each shape that can disagree.
- tRPC: no language-neutral contract for Telegram, AI tools or external clients.
- GraphQL: more complexity than the use cases need.
