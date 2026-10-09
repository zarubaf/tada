# 0066. The HTTP status of a problem code lives in `app`

- Status: Proposed
- Date: 2026-10-09
- Amends: [0037](0037-error-model.md)

## Context

[ADR 0037](0037-error-model.md) says: "The `api` crate maps each variant to an HTTP status with an exhaustive `match`."
Two adapters now answer over HTTP with problem details: `api` and `mcp` (ADR 0040).
The guard of `mcp` also answers with a problem body, for example for a missing token.
The crate boundaries of ADR 0002 do not let `mcp` use the `api` crate.
So `mcp` kept a second status map with a fallback to 500, and its problem body had no `type` URL.
A second map drifts from the first, and a fallback hides a new code.

## Decision

- `tada_app::problem::ProblemCode::http_status` gives the HTTP status of each code as a number, with an exhaustive `match`.
  A new code without a status does not compile.
- `tada_app::problem::ProblemCode::type_url` gives the `type` URL of each code in the public catalog.
- `api` and `mcp` use these two methods. No other crate maps a code to a status or builds a `type` URL.

This changes one sentence of ADR 0037: the exhaustive `match` is in `app`, not in `api`.
The rest of ADR 0037 does not change.

## Consequences

- The problem bodies of `/api/v1` and `/mcp` have the same `type`, `status` and `title` for the same code.
- `app` knows the HTTP status numbers, but no HTTP crate: the number is part of the catalog of codes (`doc/problems.md`).
- A new HTTP adapter uses the same methods and needs no status map of its own.

## Alternatives

- `mcp` uses the `api` crate: it breaks the crate boundaries of ADR 0002.
- The composition root gives `mcp` the status function of `api`: one more wiring point for a fixed table.
- A separate small crate for the catalog: one more crate for one table and one URL.
