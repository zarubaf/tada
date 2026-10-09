# 0008. Authentication separate from authorization

- Status: Accepted
- Date: 2026-10-06
- Amended by: [0070](0070-ai-security-review-replaces-the-external-review.md)

## Context

Members sign in with their own email address through a magic link.
No Microsoft account is necessary.
Rust has no maintained library equal to Better Auth that includes magic links.
The published `tower-sessions-sqlx-store` (0.15.0) needs `sqlx` 0.8 and has no user column, so it cannot revoke the sessions of one user.
Authentication code is security-critical; we must not invent cryptography.

## Decision

Authorization:

- The `identity` module owns users, external identities, organization memberships and event memberships.
- Domain commands check authorization on each call. UI buttons and session data are not authorization.
- Sign-up is invite-only. An email domain never grants membership.
- The `app` crate defines an `Authenticator` port. Only adapter crates implement it.

Tokens (magic links, invitations and sessions):

- Each token has 256 random bits from the operating system's secure random source.
- The database stores only a SHA-256 hash of each token.
- Token comparisons use the hash lookup, never a string comparison of secrets.

Magic links:

1. The member enters an email address. The response is the same for known and unknown addresses.
2. tada sends a link that expires after 15 minutes.
3. A GET request on the link shows a confirmation page only. It does not use the token, because mail scanners open links.
4. The member clicks "Sign in". A POST request uses the token. The same transaction deletes it.
5. All authentication pages send `Referrer-Policy: no-referrer`.

Invitations:

- An invitation link expires after 7 days and works once.
- A member who loses access to the email address asks an owner for a new invitation. tada records this in the audit log.

Sessions (own implementation in `store-pg`, no session library):

- The `session` table has the token hash, the `user_id`, the creation time, the last-use time and the user agent.
- The cookie is `HttpOnly`, `Secure` and `SameSite=Lax`, and has the `__Host-` prefix.
- A new session starts at each sign-in.
- Sessions have an idle timeout of 14 days and an absolute timeout of 90 days.
- Revocation deletes the session rows of a user.
- Each state-changing request must have a matching `Origin` header.

Rate limits:

- Rate limits apply for each email address and each client IP address.
- The server reads the client IP address from `X-Forwarded-For` only when the request comes from the configured reverse proxy.

Passkeys come later with `webauthn-rs`.

Assurance:

- Tests cover the applicable requirements of OWASP ASVS 5.0, chapters on authentication and session management.
- Before the first real event data goes live, a person outside the core team reviews the authentication code.

## Consequences

- One authority for memberships and roles.
- We own about as much code for sessions as for magic links. It uses only maintained primitives.
- We must find the external reviewer before go-live.

## Alternatives

- `tower-sessions` with its SQL store: incompatible with `sqlx` 0.9, and no user column.
- An identity server such as Ory Kratos or Keycloak: mature, but one more service to operate and to keep in sync with our memberships.
- A TypeScript sidecar with Better Auth: a second backend language.
- Own cryptography: high risk with no benefit.
