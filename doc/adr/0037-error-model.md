# 0037. Error model

- Status: Proposed
- Date: 2026-10-06

## Context

ADR 0017 makes RFC 9457 problem details the format for all error responses.
The web client and the Telegram gateway must show errors in German, through Fluent (ADR 0005).
The Telegram gateway calls the `app` crate directly (ADR 0011), not the HTTP API.
Clients must react to an error by a stable code, not by parsing text.
An error must not show that a record of another organization exists, and it must not repeat input that can contain tokens.

## Decision

Problem codes:

- Each error has a stable problem code, for example `record-version-conflict`.
- The `app` crate owns the codes. Each error type has a method `code()` that returns the code. The `api` crate and the Telegram gateway use the same codes.
- [doc/problems.md](../problems.md) lists each code with its meaning, its HTTP status and its parameters. Once the `api` crate exists, the build generates this file, and CI fails if it differs.
- A code never changes its meaning. A code that is no longer used stays reserved.

Format:

- Each error response has the media type `application/problem+json`.
- `type` is the URL of the code in the public catalog: `https://github.com/zarubaf/tada/blob/main/doc/problems.md#<code>`.
- `code` repeats the code as a separate field, so that clients need not parse the URL.
- `title` is a short, stable English text for developers. `detail` is an English text about this case. `detail` never repeats input values.
- `status` repeats the HTTP status.
- `instance` is `urn:uuid:<request_id>`. It is never the request path, because a path can contain a token.
- `request_id` repeats the ID for clients that do not parse `instance`.
- A validation error has an `errors` list. Each entry has a JSON pointer, a code and optional parameters, for example `{"pointer": "/date_window/end", "code": "before-start"}`.

Codes in the contract:

- The OpenAPI document lists the possible codes of each operation in the extension `x-tada-problem-codes`. A CI check compares it with the codes that the handlers can return.
- The list of codes is open: a client must handle an unknown code as a general error of its status class. A new code is therefore not a breaking change (ADR 0017).

Status mapping:

| Status | Use                                                                                                            |
| ------ | -------------------------------------------------------------------------------------------------------------- |
| 400    | The request body is not valid JSON or does not match the schema                                                |
| 401    | No valid session or token                                                                                      |
| 403    | The caller can see the record but lacks the permission for this action                                         |
| 404    | The record does not exist, or it is in a scope the caller cannot see; both cases give the same code and detail |
| 409    | `record-version-conflict`, or a state transition that the current state does not allow                         |
| 413    | The request body is larger than the limit                                                                      |
| 415    | The media type is not supported                                                                                |
| 422    | The values are well-formed but break a domain rule; the `errors` list names them                               |
| 429    | Rate limit; the response has `Retry-After`                                                                     |
| 503    | A dependency is unavailable; the client can retry                                                              |
| 500    | `internal`; the response contains no other detail                                                              |

Code structure:

- The `app` crate returns typed errors: one `enum` for each command group, with `thiserror`, and the `code()` method.
- The `api` crate maps each variant to an HTTP status with an exhaustive `match`. A new variant without a status does not compile.
- The `api` crate uses its own request extractors, so that the rejections of `axum` also give problem details with the statuses above.
- An unexpected error becomes `internal`. The log contains the cause with the request ID; the response does not.

Display:

- The web client and the Telegram gateway show the Fluent message `problem-<code>`, with the parameters of the `errors` list. They never show `detail` to members.
- If no message exists for a code, they show the general message of the status class and the request ID.

## Consequences

- German error texts live in Fluent files only, not in the backend.
- A member can give the request ID to an operator, who finds the log lines.
- The `type` URL depends on the location of the public repository. If the repository moves, GitHub redirects the old URL.

## Alternatives

- A `urn:tada:` URN: `tada` is not a registered URN namespace, and RFC 9457 prefers a URI that can be opened.
- A `type` URL on each operator's domain: the same code would have a different `type` in each installation.
- The code mapping in the `api` crate only: the Telegram gateway would need a second mapping.
- `If-Match` and `412 Precondition Failed` for version conflicts: correct HTTP, but the commands already carry the record version in the body.
- Localized `detail` texts from the backend: the backend would need the locale of each member and duplicate the Fluent files.
