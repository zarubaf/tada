# 0044. API conventions

- Status: Accepted
- Date: 2026-10-06

## Context

The first list endpoints come with the walking skeleton.
ADR 0017 defines the contract and its stability rules.
ADR 0037 defines errors, ADR 0038 defines IDs, time and idempotent creates.
State changes go through domain commands with a record version (ADR 0006).
The same conventions must hold for the web client, the MCP server (ADR 0040) and later clients.

## Decision

Naming and format:

- JSON field names use `snake_case`. Enum values use `kebab-case`, as problem codes do, for example `accepted-with-edit`.
- Keys of the field catalog (ADR 0049), for example `date_window`, are data, not enum values. They use `snake_case`, because agents and exports use them as JSON keys.
- Paths use plural nouns and IDs: `/api/v1/events/{event_id}/actions`.
- Instants are RFC 3339 strings in UTC. Calendar dates are `YYYY-MM-DD` (ADR 0038).
- An absent optional field and `null` mean the same. Responses omit absent fields.
- An explicit unknown is a value with its own status, never `null` (see the fact model ADR).

Reads:

- `GET` requests never change state.
- A list response has this shape: `{"items": [...], "next_cursor": "..."}`. `next_cursor` is absent on the last page.
- The cursor is opaque. Clients never build or parse it. It encodes the sort key and the last ID.
- `limit` sets the page size. The default is 50 and the maximum is 200.
- Filters are query parameters named after fields, for example `?status=open&owner_id=...`. Each list operation documents its filters.
- `sort` takes one field name, with `-` for descending order, for example `?sort=-due_date`. Each list operation documents the allowed fields and its default sort.
- No total count by default. An operation that needs one documents it.

Writes:

- Each state change is a command: `POST /api/v1/<resource>/{id}/<command>`, for example `POST /api/v1/changesets/{changeset_id}/apply` (ADR 0050).
- A create is `POST` on the collection. The client can send the new UUID for a safe retry (ADR 0038).
- A command on an existing record carries `expected_version`. A mismatch gives `record-version-conflict` (ADR 0037).
- There is no generic `PATCH`. Each change has a named command, so that the audit log and the permissions know its meaning.
- A successful command returns the changed record with its new version.

Documentation:

- `utoipa` documents each operation, its filters, its sort fields and its problem codes (ADR 0037).

## Consequences

- All clients page, filter and sort in the same way.
- Commands make the audit log readable: "accepted proposal", not "patched record".
- Cursor pages stay stable when new records arrive.

## Alternatives

- Offset pagination: pages shift when records arrive, and large offsets are slow.
- Generic `PATCH` with JSON Merge Patch: short, but it hides the meaning of a change from the audit log and the permission checks.
- GraphQL: rejected in ADR 0017.
