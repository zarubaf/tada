# 0070. An AI security review replaces the external review

- Status: Accepted
- Date: 2026-10-09
- Amends: [0008](0008-authentication.md), [0011](0011-telegram.md), [0035](0035-observability.md), [0036](0036-configuration.md), [0039](0039-actors-and-identities.md), [0056](0056-sign-in-details.md), [0065](0065-worker-deletes-ended-rate-limit-counters.md)

## Context

[ADR 0008](0008-authentication.md) requires that a person outside the core team reviews the sign-in code before real event data goes live.
The roadmap names this review as the first of two gates for real data.
The club has no such person at hand, and the product owner wants to go live soon.

The product owner decided on 2026-10-09 that an adversarial AI security review replaces the human external review.
The condition is that the team addresses all findings of the review.
The model of the review was Claude Opus.
This ADR records the scope, the result, the decisions that closed the findings and the risks that remain.

## Decision

We replace the external human review of ADR 0008 with an adversarial AI security review.
The gate "review of the authentication code" closes when the fixes of this ADR are on `main`.
This ADR is the record of the review.
The review report itself stays private.

### Scope

The review read the code of `main` at commit 31fc6a0.
Paths are relative to the repository root.

| Area                           | Files                                                                                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sessions                       | `crates/app/src/session.rs`, `crates/store-pg/src/session.rs`, `crates/store-pg/migrations/0008_session.sql`                                                                    |
| Authenticator port and callers | `crates/app/src/auth.rs`, `crates/app/src/caller.rs`                                                                                                                            |
| Magic links                    | `crates/app/src/sign_in.rs`, `crates/store-pg/src/sign_in.rs`, `crates/api/src/sign_in.rs`, `crates/store-pg/migrations/0006_outbound_and_sign_in_tokens.sql`                   |
| Invitations                    | `crates/app/src/members.rs`, `crates/store-pg/src/members.rs`, `crates/store-pg/migrations/0007_invitation_and_email_checks.sql`                                                |
| Mail intents and the worker    | `crates/app/src/outbound.rs`, `crates/store-pg/src/outbound.rs`, `crates/adapters/src/mail.rs`                                                                                  |
| Token generation and hashing   | `crates/store-pg/src/token.rs`                                                                                                                                                  |
| API tokens                     | `crates/app/src/tokens/mod.rs`, `crates/store-pg/src/tokens.rs`, `crates/api/src/tokens.rs`, `crates/store-pg/migrations/0013_api_token.sql`                                    |
| MCP guard and tools            | `crates/mcp/src/guard.rs`, `crates/mcp/src/lib.rs`, `crates/mcp/src/tools.rs`, `crates/mcp/src/propose.rs`, `crates/mcp/src/errors.rs`                                          |
| Rate limits                    | `crates/app/src/rate_limit.rs`, `crates/store-pg/src/rate_limit.rs`, `crates/store-pg/migrations/0009_rate_limit.sql`                                                           |
| `Origin` check                 | `crates/api/src/origin.rs`, `crates/app/src/public_url.rs`                                                                                                                      |
| Client IP address              | `crates/api/src/client_ip.rs`                                                                                                                                                   |
| Cookies and request extractors | `crates/api/src/extract.rs`                                                                                                                                                     |
| Telegram link                  | `crates/telegram/src/command.rs`, `crates/api/src/telegram.rs`, `crates/app/src/telegram.rs`, `crates/store-pg/src/telegram.rs`, `crates/store-pg/migrations/0004_telegram.sql` |
| Log redaction                  | `crates/tada/src/logging.rs`, `crates/api/src/request_id.rs`, `crates/tada/tests/support/logs.rs`                                                                               |
| Wiring                         | `crates/tada/src/serve.rs`, the router in `crates/api/src/lib.rs`, `scripts/check_no_dev_auth.py`                                                                               |

The review also read the ADRs 0008, 0039, 0040, 0053, 0056, 0062 and 0065 and [asvs-coverage.md](../asvs-coverage.md).
The review did not cover the passkeys (they do not exist yet), OAuth for MCP clients, the deployment, the event roles inside an event, or the web client except the sign-in pages.

The review tried these attack classes:

- Magic links and invitations: token entropy, storage, comparison, single use under concurrency, expiry, binding, enumeration, open redirect, host-header poisoning, token transport.
- Sessions: cookie flags, fixation, timeouts, role change and removal, CSRF through the `Origin` check.
- API tokens and MCP: format, storage, scope, the AI boundary of the caller types, `Origin` on `/mcp`, test-only constructors in release builds.
- Organization isolation: every query in scope filters by the organization, and the database enforces it.
- Telegram linking: code strength, single claim, confirmation, one account for each user.
- Rate limits and client IP address: bypass through `X-Forwarded-For`, IPv6, lockout of a member.
- Error handling and logs: secrets in `Debug` output, in log lines and in problem responses.
- The development authenticator: no authenticator remains besides the session and the token.
- Dependencies: the versions in `Cargo.lock`.

### Result

The method was code reading only.
The reviewer was read-only, started no server and sent no network request.
`cargo audit` was not installed, so the dependency versions were read but not matched against an advisory database.
The check `mise run check:deps` covers this.

The review found no way into an account, into another organization or past the AI-caller boundary for an attacker without a mailbox, a session or a token.
The token generation, the hashing, the single use and the scope types held.
The weak points were availability, the recovery after a session theft and some lifecycle gaps.

| Severity  | Count |
| --------- | ----- |
| Critical  | 0     |
| High      | 0     |
| Medium    | 3     |
| Low       | 6     |
| Info      | 6     |
| Suspicion | 1     |

Every finding is closed by a fix or by an accepted risk.
A commit subject names the fix.

| ID  | Title                                                                       | Severity  | How it was closed                                                                                                                                                                                                                       |
| --- | --------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1  | Targeted sign-in lockout through the per-address rate limit                 | Medium    | Fix: `fix(sign-in): stop sign-in lockout through the address limit` and `fix(sign-in): lengthen the mail cooldown to 10 minutes`.                                                                                                       |
| M2  | The per-IP limit counts each IPv6 address on its own                        | Medium    | Fix: `fix(rate-limit): count IPv6 clients by their /64 prefix`.                                                                                                                                                                         |
| M3  | A stolen session has no remedy, and removal plus re-invitation restores it  | Medium    | Fix: `fix(members): end all sessions of a removed member`, `fix(tokens): require a recent sign-in for API tokens and Telegram links` and `fix(sign-in): let members sign out everywhere and managers end sessions`.                     |
| L1  | A removed admin keeps pending invitations as a way back in                  | Low       | Fix: `fix(members): revoke the pending invitations of a removed member`.                                                                                                                                                                |
| L2  | An invitation for an existing user is a 7-day credential for the account    | Low       | Fix: `fix(sign-in): start no session from an invitation of a known member`. The check holds at acceptance only (see the residual risks).                                                                                                |
| L3  | Several magic links of one user stay valid, also after sign-in and sign-out | Low       | Fix: `fix(sign-in): end the other magic links at sign-in and sign-out` and `fix(sign-in): lock the user before two magic links can deadlock`.                                                                                           |
| L4  | The Telegram confirmation protects one direction only, and no unlink exists | Low       | Fix: `fix(telegram): let the Telegram account consent and let both sides unlink`, `fix(telegram): end the codes before the link in a removal and an unlink` and `fix(telegram): show the masked address of the account in the consent`. |
| L5  | An admin can revoke a pending owner invitation                              | Low       | Fix: `fix(members): let only owners revoke an owner invitation` and `fix(members): let only owners replace an owner invitation`. The second commit closes the route through a new invitation of the same address.                       |
| L6  | An empty `TADA_TRUSTED_PROXIES` turns the per-IP limit into a global limit  | Low       | Fix: `fix(api): warn when a proxy is not in TADA_TRUSTED_PROXIES`. A warning, not a required setting.                                                                                                                                   |
| S1  | Spoofed request IDs in the audit log                                        | Suspicion | Fix: `fix(api): always make the request ID on the server` and `fix(api): say that the proxy request ID is not trusted`.                                                                                                                 |
| I1  | A non-text `X-Forwarded-For` value moves a client into the proxy bucket     | Info      | Fix: `fix(api): read X-Forwarded-For entries one by one`.                                                                                                                                                                               |
| I2  | Enumeration by response time                                                | Info      | Accepted risk. The time difference is small, and the known gap ASVS 6.3.8 stays. See the cooldown timing in the residual risks.                                                                                                         |
| I3  | The rate-limit key has no minimum length                                    | Info      | Fix: `fix(settings): require 32 bytes for the rate-limit key`.                                                                                                                                                                          |
| I4  | An admin learns if an invitation ID belongs to another organization         | Info      | Accepted risk. The IDs are UUIDv7 and not guessable. The leak is one bit about a UUID that the attacker already knows. Idempotent retries need the global primary key.                                                                  |
| I5  | Login CSRF by link: the confirmation page names no account                  | Info      | Fix: `fix(sign-in): name the masked address on the magic-link page`.                                                                                                                                                                    |
| I6  | No global cap on new rate-limit counters                                    | Info      | Declined. A global cap is a lever to stop the sign-in of the whole installation. A volume flood belongs to the reverse proxy.                                                                                                           |

The review also confirmed one known gap of the coverage file: ASVS 7.2.4, session fixation.
The fix is `fix(sign-in): end the session that a sign-in replaces`.

### Decisions of the fixes

Rate limits (M1, M2, I3):

- The client limit counts by client network: an IPv4 address, or the /64 prefix of an IPv6 address.
  A /64 is the smallest prefix that a host normally gets.
  A /56 would put whole households into one bucket.
- The limit of 30 sign-in requests for each client network in one hour refuses the request.
- The limit for an email address is a mail cooldown, not a request limit.
  tada sends at most one magic-link mail for each address in each fixed window of 10 minutes.
  That is at most 6 mails for each address in one hour.
  The mail limit never refuses a request and never changes the answer.
- A request that the client limit refuses does not use the mail of the cooldown.
- The cooldown works because a new magic link does not end the older links, and a link lives 15 minutes.
  When the cooldown stops a mail, the member finds a link in the inbox that is valid for 5 more minutes or longer.
  The sign-in page tells the member to use the link in the newest mail.
- The key of the rate-limit counters must have 32 bytes or more.
  `serve` stops with exit code 2 for a shorter key.
- We reject an hourly address limit that answers `202` when exceeded.
  The member then gets neither a new mail nor an error for the rest of the hour, and the last link expires after 15 minutes.
  We also reject a limit by address and client network with a global cap for each address.
  The global cap is a lockout lever again.

Client IP address and request ID (L6, S1, I1):

- tada reads `X-Forwarded-For` entry by entry.
  An entry that is not text or not an address stops the walk where it is.
- `serve` writes one warning for each process when a peer outside `TADA_TRUSTED_PROXIES` sends `X-Forwarded-For`.
  A deployment without a proxy is valid, so the setting stays optional.
- `serve` always makes the request ID as a UUIDv7.
  It writes the `X-Request-Id` of a trusted proxy to the log line "request completed" as `proxy_request_id`.
  Some proxies, for example Caddy, pass a client header on, so a trusted proxy does not mean a trusted value.
  Do not use `proxy_request_id` to join the actions of members.

Sessions and tokens (M3, ASVS 7.2.4, L3):

- A sign-in ends the session that the request sends, in the same transaction, only when a new session starts.
- The removal of a membership, including leaving, ends all sessions of the user in every organization.
  It also ends the open magic links, the Telegram link and the member's link codes of that organization.
  tada cannot tell a stolen session from the member's own session, and a session of another organization could choose the first organization again after a new invitation.
- A member signs out everywhere in the settings page „Konto“ (`POST /api/v1/session/sign-out-everywhere`).
  This works for the last owner too.
- An owner or an admin ends all sessions of a member without a removal (`POST /api/v1/members/{user_id}/sessions/end`).
  A caller can end the sessions of members up to the own role, so only an owner ends the sessions of an owner.
  The audit action is `organization_membership.end_sessions`.
- The creation of an API token and the confirmation of a Telegram link need a recent sign-in.
  A sign-in is recent for 15 minutes, counted from the creation of the session.
  Both create access that outlives the session, so a stolen older session cannot make them.
  The revocation of a token and the unlink need no recent sign-in, because they only take access away.
  The problem code is `recent-sign-in-required` (HTTP 403).
- A sign-in and a sign-out delete all magic links of the user.
  A redemption first locks the user row, so two links redeemed at once cannot deadlock.

Invitations (L1, L2, L5, I5):

- The acceptance of an invitation starts a session only if the user has no membership in another organization.
  Otherwise it adds the membership, answers `202` with no cookie, and the member signs in with a magic link.
  The session of an acceptance never reaches more than the organization of the invitation.
- The removal of a membership revokes the pending invitations that the member created, with an audit event for each.
  A later command that lowers a role must revoke them in the same way.
- Only an owner revokes or replaces a pending owner invitation.
  An admin manages invitations up to the role admin.
- The confirmation page of a magic link shows the masked address of the account, for example `a…@example.org`.
  A preview (`POST /api/v1/sign-in/magic-link/preview`) does not use the token.

Telegram linking (L4):

- Linking has three steps.
  The member asks for a code in the web client and sends it to the bot.
  The bot names the tada account (display name, masked email address and organization), and the Telegram account accepts with `/bestaetigen` or rejects with `/trennen`.
  Then the web session shows the Telegram name and ID, and the member confirms.
- Both sides consent after they see the other side.
  A deep link makes the client of the victim send `/start` by itself, so only a step after the bot names the account stops a link to the attacker.
- The member removes a link in the web client.
  The Telegram account removes it with `/trennen`.
  The removal of a membership ends it.

### Amendments of older ADRs

This ADR changes the following rules.
The other parts of these ADRs stay in force.

ADR 0008:

- This replaces the rule "Before the first real event data goes live, a person outside the core team reviews the authentication code" of ADR 0008.
  The AI security review of this ADR takes its place.
- This replaces the consequence "We must find the external reviewer before go-live" of ADR 0008.
- This replaces the rule "Rate limits apply for each email address and each client IP address" of ADR 0008.
  The client limit applies for each client network and refuses the request.
  The address limit is a mail cooldown that stops the mail only.
- This adds to the section "Sessions": a sign-in ends the session token that the request sends.
  The creation of an API token or the confirmation of a Telegram link needs a session younger than 15 minutes.
  The removal of a membership implements "Revocation deletes the session rows of a user".
- This adds to the section "Magic links": a sign-in and a sign-out delete all magic links of the user.

ADR 0056:

- This replaces the rule "The acceptance of an invitation starts a session in the organization of the invitation" of ADR 0056.
  The acceptance starts a session only for a user without a membership in another organization.
- This replaces the rule "Owners and admins can revoke a pending invitation" of ADR 0056 with a role ceiling.
  An owner and an admin revoke or replace an invitation up to the own role.
- This replaces the rule "If the membership no longer exists, the session loses its organization" and the consequence "A removed member loses access to the organization with the next request" of ADR 0056.
  A removal ends all sessions of the member at once, in each organization, and ends the Telegram link.
- This replaces the rule "The key of a counter is an HMAC-SHA-256 of the normalized email address or the IP address" of ADR 0056.
  The key is an HMAC-SHA-256 of the normalized email address or the client network.
- This replaces the rule "The limits are 5 sign-in requests for each email address and 30 sign-in requests for each IP address in one hour" of ADR 0056.
  The limits are 30 sign-in requests for each client network in one hour and one magic-link mail for each address in each 10 minutes.

ADR 0011:

- This replaces the steps 3 to 5 of "Linking" of ADR 0011.
  The bot names the tada account, and the Telegram account accepts or rejects.
  The web session then shows the Telegram account, and the member confirms with a recent sign-in.
  Only after both consents does tada bind the Telegram user ID to the user.
- This adds: a member and a Telegram account can both remove a link.
- This extends the consequence "A phishing link alone cannot bind an account" of ADR 0011 to both directions.

ADR 0035:

- This replaces the rule "`serve` accepts `X-Request-Id` from a trusted proxy only if the value is a UUID" of ADR 0035.
  `serve` always generates a UUIDv7.
  The log field `proxy_request_id` holds the UUID `X-Request-Id` of a trusted proxy.

ADR 0036:

- This adds a rule: `TADA_RATE_LIMIT_KEY_FILE` holds at least 32 bytes.
- This adds a rule: `serve` writes a warning when a peer outside `TADA_TRUSTED_PROXIES` sends `X-Forwarded-For`.

ADR 0039:

- This adds to the API tokens: the creation of a token needs a session younger than 15 minutes.

ADR 0065:

- ADR 0065 is still Proposed. This amends its proposed text, and the owner's acceptance of ADR 0065 includes this amendment.
- This replaces the meaning of "rate-limit window" in ADR 0065 with the hour of the client limit.
  The cleanup deletes all counters whose window started before the previous hour, also the counters of the 10-minute mail cooldown.
  The retention of two hours stays.

### Residual risks

The team accepts these risks for now.

- The check of L2 holds at the time of the acceptance.
  A session from the link of a user without another membership reaches each organization that the user joins later, for the life of the session.
- The hint of I5 shows the first character and the domain of the address.
  A person with the same initial and domain does not see a difference.
- API tokens that a stolen session made in another organization survive a removal in this one.
  The member revokes them in the other organization.
- "Sign out everywhere" and the end of sessions by a manager keep the API tokens and the Telegram link.
  A session stolen within 15 minutes of a sign-in can make both.
  The member revokes the tokens and removes the Telegram link on their pages.
- `end_sessions` does not take the user lock.
  A redemption of a magic link that runs at the same time can keep its new session, or one of the two requests can fail with a database error.
  The removal has the same pattern.
- An invitation that a removed admin sends at the same moment as the removal can commit after the revocation of the pending invitations.
- The check of the admin role runs outside the lock of that transaction.
  An admin can end the sessions of a member that an owner promotes at the same moment.
  The effect is a sign-out only.
- A request in a mail cooldown is one statement faster than a request outside it.
  The difference shows that someone asked for the address in the last 10 minutes.
  The window allows few samples, and this adds to the known gap ASVS 6.3.8.
- An attacker with a /48 prefix has 65,536 client buckets.
  This does not raise the mail bound for an address, but it grows the counter table.
- Clients behind NAT64 share one /64 bucket, so the limit of 30 for each hour applies to all of them together.
- The warning for a missing `TADA_TRUSTED_PROXIES` needs an `X-Forwarded-For` header.
  A proxy that sends no such header gives no warning, and the warning comes once for each process.
  The deployment checklist must name the setting and the 32-byte minimum of the key.

The next change of the sign-in code needs a new security review of the changed files.
The product owner picks the reviewer.
This applies to the files of the scope table above.

## Consequences

- The first gate of the roadmap closes without a human outsider.
  The second gate, the person features of ADR 0045, stays open.
- The team owns the residual risks above.
  An AI review has no accountability, and it can miss classes of errors that a human expert finds.
  The method (code reading, no running server, no dependency advisory check) limits what it finds.
- A removal now signs a member out in every organization.
  Members of several clubs notice this.
- Telegram linking has three steps.
  Pending claims from before the migration need `/bestaetigen`, and confirmed links stay.
- A request for an address in its mail cooldown gets `202` and no mail.
  The OpenAPI contract does not change, and one new problem code exists.
- `serve` stops with a rate-limit key shorter than 32 bytes.
  The operator checks production keys before the deploy.
- The review brief of the sign-in code is gone.
  This ADR holds its scope, and [asvs-coverage.md](../asvs-coverage.md) holds the open items.

## Alternatives

- A human external review, as ADR 0008 requires: independent and accountable.
  We rejected it because no reviewer is at hand, and the owner wants to go live soon.
- No review: fastest, but the sign-in code protects all data of the clubs, and the review found three Medium findings that tests had not found.
