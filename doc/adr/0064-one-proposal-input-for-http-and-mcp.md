# 0064. One proposal input for HTTP and MCP

- Status: Proposed
- Date: 2026-10-09

## Context

ADR 0017 says: "Request and response types (DTOs) live only in the `api` crate. They are the single definition of the contract."
ADR 0040 says that the MCP tool schemas come from the `app` input types through `schemars`.
The proposal input has many operations, each with a typed value, evidence and an expected version (ADR 0050).
The HTTP API takes the same proposals: the request of `CreateChangeset` holds a list of proposals.
The apply request of the Review Inbox holds edited values in the same shape (`FactStateInput`).
A copy of these types in `api` would be a second definition of the same shape.
The two copies could disagree, and an agent that uses MCP and HTTP would then need two shapes.

## Decision

- The HTTP API and MCP use one shape for the proposal input: the input types of `app::proposals::input`.
- Only these types are an exception to the DTO rule of ADR 0017:
  - `NewProposal` and the types that it contains, in the request of `CreateChangeset`;
  - `FactStateInput`, in the edits of the request of `ApplyChangeset`.
- The request types stay in `api`, for example `CreateChangesetRequest` and `ApplyChangesetRequest`. They hold the `app` input types as fields.
- `api` makes the OpenAPI schema of these requests from the JSON Schema of `schemars` (`crates/api/src/json_schema.rs`).
- All other request and response types stay DTOs in `api`, as ADR 0017 says.
- The `domain` crate still derives no schema.

## Consequences

- A change of an `app` input type changes the HTTP contract, also if no file of `api` changes.
- Two checks guard the contract:
  - `check:generated` fails if the committed `contracts/openapi.json` differs from the generated one, so each contract change shows in review;
  - `check:contract` (`oasdiff`) fails on a breaking change against `main`.
- A reviewer of a change in `app::proposals::input` checks the diff of `contracts/openapi.json`.
- A new exception needs a new ADR.

## Alternatives

- A DTO copy of the proposal input in `api`: two definitions of the same shape that can disagree.
- MCP tool schemas from the `api` DTOs: `mcp` would then depend on `api`, and ADR 0040 takes the schemas from `app`.
