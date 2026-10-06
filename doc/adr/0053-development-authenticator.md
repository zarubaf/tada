# 0053. Development authenticator for the walking skeleton

- Status: Accepted
- Date: 2026-10-06

## Context

The walking skeleton has an API command (`CreateEvent`) and a page that lists events.
Each `app` command takes a typed caller (ADR 0039), so the API must identify a member.
Sign-in with magic links and sessions (ADR 0008) is not in Slice 0.
An API that accepts calls without a caller would contradict ADR 0039 and leave an open API in the code.

## Decision

- The `app` crate defines the `Authenticator` port (ADR 0008). The `api` crate asks it for the caller of each request.
- `store-pg` has a `DevAuthenticator`. It exists only in debug builds (`cfg(debug_assertions)`), so a release build cannot contain it.
- In a debug build, `tada serve` creates the development organization (slug `dev`) if it does not exist.
  The `DevAuthenticator` then gives each request the same member: an owner of this organization, with fixed UUIDs.
- In a release build, `tada serve` uses an authenticator that rejects each request with `unauthenticated`.
  The image therefore serves no organization data until sign-in exists.
- The sign-in of ADR 0008 replaces both in Slice 1. The `app` and `domain` crates do not change for this.
- The development member has no row in the database. Records refer to it only through its UUID.

## Consequences

- The skeleton exercises the real path from the HTTP request through the caller type to the database.
- Anyone who can reach a debug build has owner rights in the development organization. Debug builds are for local development and tests only.
- A release image cannot show the web client with data before Slice 1.

## Alternatives

- Full sign-in in the skeleton: correct from the start, but it makes the skeleton several times larger.
- No caller check in the skeleton: smallest, but it contradicts ADR 0039.
- A development authenticator behind a setting: an operator could switch it on in production.
