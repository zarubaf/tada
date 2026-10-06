# 0008. Authentication separate from authorization

- Status: Proposed
- Date: 2026-10-06

## Context

Members sign in with their own email address through a magic link.
No Microsoft account is necessary.
Rust has no maintained library equal to Better Auth that includes magic links.
Authentication code is security-critical; we must not invent cryptography.

## Decision

- The `identity` module owns users, external identities, organization memberships and event memberships.
- Domain commands check authorization on each call. UI buttons and session data are not authorization.
- Sign-up is invite-only. An email domain never grants membership.
- The `app` crate defines an `Authenticator` port. Only adapter crates implement it.
- Magic links:
  - The token has 256 random bits from the operating system's secure random source.
  - The database stores only a SHA-256 hash of the token.
  - A token expires after 15 minutes and works only once. The consume step deletes it in the same transaction.
  - Rate limits apply for each email address and each IP address.
- Sessions use `tower-sessions` with a PostgreSQL store:
  - The cookie is `HttpOnly`, `Secure` and `SameSite=Lax`.
  - The session ID changes at sign-in.
  - Sessions have an idle timeout and an absolute timeout.
  - Each state-changing request must have a matching `Origin` header.
- Passkeys come later with `webauthn-rs`.
- A test suite covers the OWASP ASVS authentication and session requirements that apply.
  A second person reviews all authentication changes.

## Consequences

- One authority for memberships and roles.
- We own a small amount of authentication code. It uses only maintained primitives.
- Revocation is simple: the server deletes the session rows of a user.

## Alternatives

- An identity server such as Ory Kratos or Keycloak: mature, but one more service to operate and to keep in sync with our memberships.
- A TypeScript sidecar with Better Auth: a second backend language.
- Own cryptography or own session tokens without a library: high risk with no benefit.
