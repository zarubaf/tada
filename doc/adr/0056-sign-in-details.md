# 0056. Sign-in: organization, invitations and rate limits

- Status: Accepted
- Date: 2026-10-07

## Context

Slice 1 brings the sign-in of ADR 0008. It replaces the development authenticator of ADR 0053.
The accepted ADRs already decide most of the sign-in:

- ADR 0008: invite-only sign-up, magic links, invitations, sessions, the cookie, the `Origin` check and the rate limits.
- ADR 0036: `tada bootstrap` creates an organization and the invitation of its first owner.
- ADR 0039: the `MemberCaller` and personal API tokens. A token belongs to one organization.
- ADR 0042: the worker sends the magic link and the invitation mail. Only the worker creates the token.
- ADR 0052: event roles. An event manager manages the event memberships.

Four questions remain open:

1. A user can be a member of more than one organization. A `MemberCaller` holds one organization. No ADR says how a request selects it.
2. ADR 0008 says that an owner gives a new invitation. No ADR says which organization roles can invite, and which roles an invitation can give.
3. ADR 0008 requires rate limits but does not say where the counters live. ADR 0025 requires that two `serve` processes work together, so counters in process memory are not enough. A counter key is an email address or an IP address, which is personal data (ADR 0035).
4. The magic link and the invitation link carry a token. The reverse proxy of the operator can log the path and the query of each request. The browser does not send the URL fragment to the server.

## Decision

We decide the following. The alternatives below list the options that we rejected.

Organization of a request:

- The `session` table gets an `organization_id` column. It is empty until the member chooses an organization.
- After the sign-in, a user with one organization membership gets this organization at once.
- A user with more than one organization membership chooses one in the web client. The session stores the choice.
- The member can change the organization of the session later. The session token stays the same.
- The authenticator reads the membership on each request. If the membership no longer exists, the session loses its organization.
- A request of a session without an organization gets the new problem code `organization-required` (ADR 0037). The web client then shows the choice.
- API paths do not change. They contain no organization ID (ADR 0044).

Invitations:

- An owner can invite with the organization role owner, admin or member.
- An admin can invite with the organization role admin or member.
- A member cannot invite.
- An invitation names an email address, an organization role and the display name of the person.
- If a user with this email address exists, the invitation adds a membership to this user. Otherwise it creates the user.
- One email address belongs to at most one user in an installation.
- The acceptance of an invitation starts a session in the organization of the invitation.
- Owners and admins can revoke a pending invitation and remove a membership. Nobody can remove the last owner of an organization, and the last owner cannot leave.
- A sign-in request for an address without a membership sends no mail. The response is the same as for other addresses (ADR 0008).

Rate limits:

- A PostgreSQL table holds the counters, so that all `serve` processes share them.
- The key of a counter is an HMAC-SHA-256 of the normalized email address or the IP address, with a secret from `TADA_RATE_LIMIT_KEY_FILE` (ADR 0036). A plain hash of an IP address is easy to reverse.
- The limits are 5 sign-in requests for each email address and 30 sign-in requests for each IP address in one hour. The limits are constants in the code.
- Each sign-in request also deletes the counters whose window ended. This needs no scheduled job, because schedules come only in Slice 2.
- The data inventory (ADR 0045) lists the table.

Links with tokens:

- A magic link and an invitation link carry the token in the URL fragment (`#token=...`), not in the path or the query.
- The web client reads the token from the fragment and shows the confirmation page (ADR 0008). Only the POST request sends the token to the server, in the body.

## Consequences

- One user can work in several organizations with one email address and one session.
- `MemberCaller`, the API paths and the `OrgScope` of ADR 0006 do not change.
- A removed member loses access to the organization with the next request.
- Proxy logs and server logs never contain a sign-in token.
- `serve` needs one more secret file.
- The web client must handle the problem code `organization-required`.

## Alternatives

Organization of a request:

- The organization in the path, for example `/api/v1/orgs/{slug}/events`: explicit, but it changes all paths of ADR 0044 and of the OpenAPI contract.
- An organization header on each request: the web client must keep state that the server already has.
- One organization for each user: simple, but a person who works for two clubs needs two email addresses.

Invitations:

- Only owners invite: the owner becomes a bottleneck in a large club.
- Each member invites: membership is then no longer a decision of the club.

Rate limits:

- Counters in process memory, for example with the `governor` crate: no database writes, but two `serve` processes count separately, and a restart resets the counters.
- Plain email addresses and IP addresses as keys: simpler, but the table then holds direct identifiers.
- A plain SHA-256 hash as the key: an attacker with the table can find each IPv4 address by brute force.

Links with tokens:

- The token in the query string: the reverse proxy can log it.
- The token in the path: the same problem, and ADR 0035 already warns about it.
