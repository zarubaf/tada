# 0037. Error model

- Status: Proposed
- Date: 2026-10-06

## Context

ADR 0017 makes RFC 9457 problem details the format for all error responses.
The web client and the Telegram gateway must show errors in German, through Fluent (ADR 0005).
Clients must react to an error by a stable code, not by parsing text.
An error must not show that a record of another organization exists.

## Decision

Format:

- Each error response has the media type `application/problem+json`.
- `type` is a URN with a stable code: `urn:tada:problem:<code>`, for example `urn:tada:problem:record-version-conflict`. A URN needs no domain name.
- `code` repeats the code as a separate field, so that clients need not parse the URN.
- `title` is a short, stable English text for developers. `detail` is an English text for developers about this case.
- `status` repeats the HTTP status. `instance` is the request path. `request_id` connects the error to the logs (ADR 0035).
- A validation error has an `errors` list. Each entry has a JSON pointer, a code and optional parameters, for example `{"pointer": "/date_window/end", "code": "before-start"}`.

Codes in the contract:

- The OpenAPI document lists the codes of each operation.
- The list of codes is open: a client must handle an unknown code as a general error of its status class. A new code is therefore not a breaking change (ADR 0017).
- A code never changes its meaning. A code that is no longer used stays reserved.

Status mapping:

| Status | Use                                                                                    |
| ------ | -------------------------------------------------------------------------------------- |
| 400    | The request is not valid JSON or does not match the schema                             |
| 401    | No valid session or token                                                              |
| 403    | The caller can see the record but lacks the permission for this action                 |
| 404    | The record does not exist, or it belongs to a scope the caller cannot see              |
| 409    | `record-version-conflict`, or a state transition that the current state does not allow |
| 422    | The values are well-formed but break a domain rule; the `errors` list names them       |
| 429    | Rate limit; the response has `Retry-After`                                             |
| 503    | A dependency is unavailable; the client can retry                                      |
| 500    | `internal`; the response contains no other detail                                      |

Code structure:

- The `app` crate returns typed errors: one `enum` per command group, with `thiserror`.
- The `api` crate maps each variant to a status and a code with an exhaustive `match`. A new variant without a mapping does not compile.
- An unexpected error becomes `internal`. The log contains the cause with the request ID; the response does not.

Display:

- The web client and the Telegram gateway show the Fluent message `problem-<code>`, with the parameters of the `errors` list. They never show `detail` to members.
- If no message exists for a code, they show the general message of the status class and the request ID.

## Consequences

- German error texts live in Fluent files only, not in the backend.
- A member can give the request ID to an operator, who finds the log lines.
- 404 for records outside the caller's scope prevents a check whether a record exists.

## Alternatives

- `type` as an HTTPS URL: needs a stable public domain for the documentation.
- `If-Match` and `412 Precondition Failed` for version conflicts: correct HTTP, but the commands already carry the record version in the body.
- Localized `detail` texts from the backend: the backend would need the locale of each member and duplicate the Fluent files.
