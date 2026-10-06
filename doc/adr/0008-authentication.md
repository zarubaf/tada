# 0008. Authentication separate from authorization

- Status: Proposed
- Date: 2026-10-06

## Context

Members sign in with their own email address through a magic link.
No Microsoft account is necessary.
The briefs propose Better Auth, which also has an organization plugin.
If Better Auth and the domain both store memberships, there are two authorities for one fact.

## Decision

- Better Auth handles authentication only: magic links, sessions and later passkeys.
- We do not use the Better Auth organization plugin.
- The `identity` code module owns users, external identities, organization memberships and event memberships.
- Domain commands check authorization on each call. UI buttons and session claims are not authorization.
- Sign-up is invite-only. An email domain never grants membership.
- Better Auth stays behind an `Authenticator` port. Only `identity/infra` imports it.
- We pin the Better Auth version and test the sign-in flow in CI.

## Consequences

- One authority for memberships and roles.
- We can replace the authentication library without a change to the domain.
- The Telegram link and the web session map to the same user UUID.

## Alternatives

- Better Auth with its organization plugin: duplicates our membership model.
- Our own session code: security risk with no benefit.
- An identity server such as Keycloak: one more service to operate.
