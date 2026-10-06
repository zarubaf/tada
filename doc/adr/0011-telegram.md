# 0011. Telegram channel adapter

- Status: Proposed
- Date: 2026-10-06

## Context

Telegram is the first messaging channel.
Telegram identity must link to a user through a single-use code, never through a display name.
Development laptops have no public HTTPS address.

## Decision

- We use the grammY library in a `channels/telegram` adapter.
- The update source is configuration: webhook with a secret token in production, long polling in development.
- The adapter stores each update ID and ignores duplicates.
- Each button action checks the user, the current membership, the role, the record version and the expiry.
- The adapter calls domain commands only. It contains no domain rules.

## Consequences

- Local development needs no tunnel.
- A WhatsApp adapter later uses the same domain commands.

## Alternatives

- Direct Bot API calls: more code for update parsing and retries.
- An agent runtime such as NanoClaw: a runtime we do not need for one channel.
