# 0062. Authenticators in `app`

- Status: Proposed
- Date: 2026-10-07

## Context

ADR 0008 says: "The `app` crate defines an `Authenticator` port. Only adapter crates implement it."
The session authenticator (`SessionAuthenticator`) lives in `app`.
It reads the session, the user and the membership through three ports, and it gives a `MemberCaller` with an `OrgScope`.
The constructor of `OrgScope` for a session is private to `app`, so that only a caller gives a scope (ADR 0006, ADR 0039).
An authenticator in `store-pg` would need a public scope constructor.
Then any crate could make a scope for any organization, which weakens tenant isolation.
A second authenticator for API tokens comes later (ADR 0040), so the rule must be clear before then.

## Decision

- An authenticator that only composes ports lives in `app`.
  Examples are `SessionAuthenticator` and the later API-token authenticator.
- Adapter crates implement the stores that an authenticator reads, not the authenticator.
- The `Authenticator` port stays in `app`, and the API uses only the port.
- No adapter decides authorization.
- This decision amends the line about adapter crates in ADR 0008. The other parts of ADR 0008 stay in force.

## Consequences

- The constructors of `OrgScope` stay private to `app`.
- A new authenticator that needs I/O adds a store port, and an adapter crate implements that port.
- The development authenticator in `store-pg` is an exception until it goes away with the session sign-in in production.

## Alternatives

- Authenticators in `store-pg`: they need a public `OrgScope` constructor, which any crate could then use.
- A separate `auth` crate: one more crate for two small types, and it still needs the private scope constructor of `app`.
