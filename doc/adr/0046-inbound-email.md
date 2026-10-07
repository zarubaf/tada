# 0046. Inbound email

- Status: Accepted
- Date: 2026-10-06
- Amended by: [0057](0057-modular-mail-and-inbound-webhook.md) (proposed): the webhook adapter, the inbound delivery and the setting names.

## Context

Suppliers and members send information by email.
In Slice 2, members forward or copy messages to an address for each event (see [ARCHITECTURE.md](../ARCHITECTURE.md), Integrations).
The address, the mail provider and spam filtering are operator details (ADR 0033).
An email is evidence, never authority: a sender address can be forged.
A forwarded message gets a new `Message-ID` and new headers from each member who forwards it.
`Authentication-Results` headers can be forged by a sender, and after a forward they describe the forwarding member, not the original sender.
`async-imap` 0.12.0 is the maintained async IMAP client; the `imap` crate 3.0 is in alpha since 2023. `mail-parser` parses MIME messages.

## Decision

Receiving:

- Each event has an inbound address. The address contains a random token, for example `<event-key>-<token>@<operator domain>`. The token makes guessing hard.
- The `app` crate defines an inbound mail port. The first adapter polls an IMAP mailbox of the operator with `async-imap` and parses messages with `mail-parser`. A provider webhook adapter can come later.
- The adapter stores the raw message (RFC 5322) as a source version before it parses anything. A checkpoint moves only after this.

Deduplication and evidence:

- Deduplication works inside the organization on the original message:
  - If a message contains a forwarded message as an attachment (`message/rfc822`), tada uses the `Message-ID` of the embedded message.
  - Otherwise tada uses the `Message-ID` of the message itself.
  - As a second key, tada uses a hash of the normalized body: without quoted forward headers, signatures and whitespace differences.
- tada keeps the thread relationship from `In-Reply-To` and `References`.
- Each attachment becomes a document version, linked to the message. The upload policy (ADR 0043) applies.
- tada reads only the `Authentication-Results` header whose `authserv-id` is the operator's own mail server. tada ignores all other such headers.
- These results describe the last hop. After a forward, they say who forwarded the message, not who wrote it.
- tada labels each sender who is not a member as unverified. A sender label and authentication results never grant rights.

Attribution:

- The address token gives the event.
- A message to an unknown or revoked token goes to the triage queue of the organization. AI can suggest an event; a member decides (see ARCHITECTURE.md).

Limits:

- `TADA_INBOUND_MAX_BYTES` sets the size limit for one message. Larger messages go to triage with a note.
- The operator's mail server filters spam. tada does not filter spam itself.

Not now:

- Outbound replies from the inbound address. Replies stay in the members' own mail clients.

## Consequences

- A message that two members forward appears once, if its embedded `Message-ID` or its normalized body matches. A forward that changes the body can still create a second source version; the review then shows both.
- An event address can be revoked and replaced when spam arrives.
- No Microsoft or Google integration is necessary.

## Alternatives

- A fixed address without a token: easy to guess and to spam.
- Parse before storing: a parser bug would lose the original evidence.
- Mail provider APIs from the start: a lock-in to one provider.
