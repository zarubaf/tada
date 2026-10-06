# 0011. Telegram channel adapter

- Status: Proposed
- Date: 2026-10-06

## Context

Telegram is the first messaging channel.
Telegram identity must link to a user through a single-use code, never through a display name.
An attacker can send a victim a link that contains the attacker's code. If the victim opens it, the victim's Telegram account links to the attacker's tada user.
`teloxide` had no release since July 2025.
Development laptops have no public HTTPS address.

## Decision

Library and process:

- The `telegram` crate uses `frankenstein`, a thin typed client for the Bot API, with no bot framework.
- The binary runs the gateway as its own process: `tada telegram`.
- The update source is configuration: webhook in production, long polling in development.
- The gateway calls `app` commands only. It contains no domain rules.

Webhook:

- The webhook checks the secret token header with a constant-time comparison.
- The gateway stores each update ID and ignores duplicates.

Linking:

1. A member signs in on the web and asks for a link code.
2. tada stores the hash of the code. The code expires after 10 minutes and works once.
3. The member sends the code to the bot.
4. The web session shows the Telegram name and ID that sent the code. The member confirms the link there.
5. Only after the confirmation does tada bind the Telegram user ID to the user.

Actions:

- Each button action checks the user, the current membership, the role, the record version and the expiry.

Groups:

- Only an event manager can bind a Telegram group to an event.
- The bot posts in a group only content whose audience includes all group members.
- Group membership never grants rights in tada.

## Consequences

- Local development needs no tunnel.
- A phishing link alone cannot bind an account, because the victim's web session must confirm it.
- We write the update handling that a bot framework would give, but this code is small.

## Alternatives

- `teloxide`: a full framework, with no release since July 2025.
- Direct Bot API calls with `reqwest`: possible, but we would write the types ourselves.
- An agent runtime such as NanoClaw: a runtime we do not need for one channel.
