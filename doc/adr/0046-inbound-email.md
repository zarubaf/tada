# 0046. Inbound email

- Status: Proposed
- Date: 2026-10-06

## Context

Suppliers and members send information by email.
In Slice 2, members forward or copy messages to an address for each event (see [ARCHITECTURE.md](../ARCHITECTURE.md), Integrations).
The address, the mail provider and spam filtering are operator details (ADR 0033).
An email is evidence, never authority: a sender address can be forged.

## Decision

Receiving:

- Each event has an inbound address. The address contains a random token, for example `<event-key>-<token>@<operator domain>`. The token makes guessing hard.
- The `app` crate defines an inbound mail port. The first adapter polls an IMAP mailbox of the operator. A provider webhook adapter can come later.
- The adapter stores the raw message (RFC 5322) as a source version before it parses anything. A checkpoint moves only after this.

Deduplication and evidence:

- Deduplication uses the `Message-ID` and a hash of the raw message, inside the organization.
- tada keeps the thread relationship from `In-Reply-To` and `References`.
- Each attachment becomes a document version, linked to the message. The upload policy (ADR 0043) applies.
- tada records the sender address and the authentication results (SPF, DKIM, DMARC) from the operator's mail server as evidence. These results never grant rights.

Attribution:

- The address token gives the event.
- A message to an unknown or revoked token goes to the triage queue of the organization. AI can suggest an event; a member decides (see ARCHITECTURE.md).

Limits:

- `TADA_INBOUND_MAX_BYTES` sets the size limit for one message. Larger messages go to triage with a note.
- The operator's mail server filters spam. tada does not filter spam itself.

Not now:

- Outbound replies from the inbound address. Replies stay in the members' own mail clients.

## Consequences

- A forwarded message appears once, also when two members forward it.
- An event address can be revoked and replaced when spam arrives.
- No Microsoft or Google integration is necessary.

## Alternatives

- A fixed address without a token: easy to guess and to spam.
- Parse before storing: a parser bug would lose the original evidence.
- Mail provider APIs from the start: a lock-in to one provider.
