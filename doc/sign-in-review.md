# Review brief for the sign-in code

[ADR 0008](adr/0008-authentication.md) requires that a person outside the core team reviews the authentication code before real event data goes live.
This file is the brief for that person.
It names the scope, the decisions, the threats and the known gaps.
The review itself is a human task.

## What tada does

tada has no passwords.
A member signs in with a one-time link that tada sends to the email address of the member.
An invitation creates the first sign-in.
A session cookie identifies the member after the sign-in.
A member can also create personal API tokens for AI clients that use MCP.
A member can link a Telegram account with a one-time code.

## Scope

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
| Wiring                         | `crates/tada/src/serve.rs`                                                                                                                                                      |

The tests are in `crates/tada/tests` and in the `tests` modules of the files above.
[asvs-coverage.md](asvs-coverage.md) maps each ASVS 5.0 requirement of chapters V6 and V7 to a test.

## Decisions

| ADR                                       | Status   | Topic                                                                                                     |
| ----------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------- |
| [0008](adr/0008-authentication.md)        | Accepted | Authentication separate from authorization: magic links, sessions, `Origin` check, rate limits, assurance |
| [0011](adr/0011-telegram.md)              | Accepted | Telegram link with a single-use code and a confirmation in the web session                                |
| [0036](adr/0036-configuration.md)         | Accepted | Configuration and secret files, for example the key of the rate limits                                    |
| [0039](adr/0039-actors-and-identities.md) | Accepted | Callers, actors and personal API tokens                                                                   |
| [0040](adr/0040-ai-intake-through-mcp.md) | Accepted | MCP server with bearer tokens and proposal tools only                                                     |
| [0042](adr/0042-transactional-email.md)   | Accepted | Mail through stored intents and the worker                                                                |
| [0045](adr/0045-data-protection.md)       | Accepted | Personal data, retention and redaction                                                                    |
| [0056](adr/0056-sign-in-details.md)       | Accepted | Organization of a session, invitations, rate limit counters, tokens in the URL fragment                   |
| [0062](adr/0062-authenticators-in-app.md) | Proposed | Authenticators that only compose ports live in `app`                                                      |

[ADR 0053](adr/0053-development-authenticator.md) is superseded.
The check `scripts/check_no_dev_auth.py` shows that no development authenticator remains.

## Threat model

| Threat                                                   | What tada does                                                                                                                                                                                                                                                                            | Evidence                                                                                                                                                                                                                                                                                              | Open                                                                                                             |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Mail scanners open the link and use the token.           | A GET request on the link shows a confirmation page and does not use the token. Only the POST request uses it. The token is in the URL fragment, so the scanner does not send it to the server.                                                                                           | `sign_in.rs::a_get_request_on_the_link_does_not_sign_in`, `invitations.rs::a_preview_does_not_use_the_token`                                                                                                                                                                                          | None known.                                                                                                      |
| A link leaks through proxy logs or the `Referer` header. | The token is in the fragment, so no log of the proxy holds it. Each response forbids the referrer. The logs of tada hold no token.                                                                                                                                                        | `sign_in.rs::each_response_forbids_the_referrer`, `invitations.rs::each_invitation_response_forbids_the_referrer`, `support::logs::assert_clean`                                                                                                                                                      | None known.                                                                                                      |
| CSRF against a state-changing request.                   | The cookie is `SameSite=Lax`. Each state change needs a matching `Origin` header.                                                                                                                                                                                                         | `events.rs::a_state_change_from_another_origin_changes_nothing`, `crates/api/src/contract.rs::each_state_change_lists_the_origin_check`                                                                                                                                                               | None known.                                                                                                      |
| Session fixation.                                        | Sign-in creates a new session token with 256 random bits. The cookie has the `__Host-` prefix.                                                                                                                                                                                            | `sign_in.rs::a_member_signs_in_with_the_magic_link_in_the_organization`                                                                                                                                                                                                                               | ASVS 7.2.4: sign-in does not end a session token that the request already sends.                                 |
| Enumeration of members by the sign-in request.           | The response does not depend on the address, and only a member gets mail.                                                                                                                                                                                                                 | `sign_in.rs::a_sign_in_request_gets_the_same_answer_for_each_address_and_only_a_member_gets_mail`                                                                                                                                                                                                     | ASVS 6.3.8: no test checks the response time.                                                                    |
| Brute force and mail flooding across processes.          | A table in PostgreSQL holds the counters, so all `serve` processes share them. The limits are 30 requests for each client network (an IPv4 address or an IPv6 /64) in one hour, and one mail for each address in each 5 minutes. The limit of an address stops the mail, not the request. | `sign_in.rs::one_address_gets_at_most_one_mail_in_each_cooldown_also_with_two_processes`, `sign_in.rs::requests_of_another_client_do_not_lock_a_member_out`, `sign_in.rs::the_31st_request_from_one_ip_address_is_rate_limited`, `sign_in.rs::the_31st_request_from_one_ipv6_network_is_rate_limited` | ASVS 6.6.3: the limits apply to the requests for a link, not to the redemption of a token. A token has 256 bits. |
| Token theft from the database.                           | The database holds only the SHA-256 hash of each token. The counters hold HMAC values, not addresses.                                                                                                                                                                                     | `crates/store-pg/src/tokens.rs::tokens_start_with_the_prefix_and_the_database_holds_only_the_hash`, `crates/store-pg/src/rate_limit.rs::the_counters_hold_no_address`                                                                                                                                 | ASVS 6.5.3 and 6.5.4: the source of the random bytes and the length are checked by reading the code only.        |
| A removed member keeps access.                           | The authenticator reads the membership on each request, so the session loses its organization with the next request. tada deletes the API tokens of the member.                                                                                                                           | `members.rs::a_removed_member_loses_the_organization_with_the_next_request`, `crates/store-pg/src/members.rs::a_revocation_deletes_the_tokens_and_works_once`                                                                                                                                         | ASVS 7.4.5: no command ends the sessions of another member. The session rows stay.                               |
| An AI client reaches more than its scope.                | A token gives an AI caller for one member in one organization. The scope is `read` or `propose`. No tool accepts, rejects or deletes. A token does not open the HTTP API.                                                                                                                 | `tokens.rs::a_bearer_token_does_not_open_the_http_api`, `mcp.rs::only_a_propose_token_of_a_member_who_can_propose_in_the_event_proposes`, `mcp.rs::search_never_returns_a_source_of_another_organization_or_of_an_event_without_a_role`                                                               | None known.                                                                                                      |
| Phishing of the Telegram link.                           | An attacker can send a victim a link with the code of the attacker. The web session shows the Telegram name and ID that sent the code, and the member confirms the link there.                                                                                                            | `telegram.rs::the_member_confirms_the_link_in_the_web_client`                                                                                                                                                                                                                                         | A member can confirm without care. The text of the confirmation page is for the reviewer to judge.               |

## Known gaps and deviations

[asvs-coverage.md](asvs-coverage.md) lists the same items under "Open items".

| ID    | Status    | Description                                                                                                |
| ----- | --------- | ---------------------------------------------------------------------------------------------------------- |
| 7.2.4 | Gap       | Sign-in does not end a session token that the request already sends.                                       |
| 7.4.5 | Gap       | No command ends the sessions of another member.                                                            |
| 7.5.2 | Gap       | tada does not show the list of sessions.                                                                   |
| 6.3.3 | Deviation | Email is the single factor by design. Passkeys come later.                                                 |
| 6.5.5 | Deviation | The magic link lives 15 minutes, and ASVS asks for 10 minutes. The ADR gives no reason for the difference. |

## How to run the tests

Install the tools with `mise install`.
Start the database as [contributing.md](contributing.md) describes.
Then run:

```text
mise run check
```

This command runs the format checks, the linters, the secret scan, the Rust tests and the contract checks.
The tests that use PostgreSQL need the database of the development stack.
To list the tests of one file, run `cargo nextest list` and filter by the file name.

## Out of scope

- Passkeys. They come later with `webauthn-rs`.
- OAuth for MCP clients. [ADR 0040](adr/0040-ai-intake-through-mcp.md) says that OAuth needs a new ADR.
- The deployment: the reverse proxy, TLS, DNS and the mail provider. [ADR 0033](adr/0033-deployment-outside-this-repository.md) puts them outside this repository.
- The authorization rules for roles inside an event, except where they limit a token.
- The web client, except the sign-in pages and the handling of the URL fragment.

## Where findings go

Report a finding as a GitHub issue of this repository with the label `sign-in-review`.
Give the file, the line and the ASVS requirement if one applies.
This repository has no security policy yet.
Do not post an exploitable vulnerability in a public issue.
Ask the product owner for a private channel first.
The team closes the findings before real data goes live.
