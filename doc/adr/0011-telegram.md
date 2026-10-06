# 0011. Telegram channel adapter

- Status: Accepted
- Date: 2026-10-06

## Context

Telegram is the first messaging channel.
Telegram identity must link to a user through a single-use code, never through a display name.
Development laptops have no public HTTPS address.

## Decision

- The `telegram` crate uses `teloxide`. The binary runs it as its own process: `tada telegram`.
- `teloxide` had no release since July 2025. The crate is small, so we can replace it with direct Bot API calls.
- The update source is configuration: webhook with a secret token in production, long polling in development.
- The gateway stores each update ID and ignores duplicates.
- Each button action checks the user, the current membership, the role, the record version and the expiry.
- The gateway calls `app` commands only. It contains no domain rules.

## Consequences

- Local development needs no tunnel.
- A WhatsApp adapter later uses the same domain commands.

## Alternatives

- An agent runtime such as NanoClaw: a runtime we do not need for one channel.
