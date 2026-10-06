# 0042. Transactional email

- Status: Proposed
- Date: 2026-10-06

## Context

Magic links and invitations (ADR 0008) need email in Slice 1.
Later, reminders and digests can use email as a fallback channel.
The provider, the sending domain and the DNS records (SPF, DKIM, DMARC) are operator details (ADR 0033).
Outbound messages go through a stored intent and a job (ADR 0007).
`lettre` 0.11.23 (August 2026) is the maintained SMTP client for Rust.

## Decision

Port and adapter:

- The `app` crate defines a `Mailer` port. It sends one message with a recipient, a subject, a plain text part and an HTML part.
- The first adapter uses SMTP with `lettre`, with TLS required. Nearly every mail provider accepts SMTP, so the operator can choose freely.
- A provider HTTP API adapter can come later through a new ADR, if an operator needs it.
- Development uses Mailpit. Tests use an in-memory adapter.

Sending:

- No command sends mail directly. A command stores an outbound intent, and the `worker` sends it (ADR 0007).
- An intent or a job payload never contains a token. For a magic link or an invitation, the intent names the user and the purpose; the worker creates the token when it sends, and stores only its hash (ADR 0008).
- The worker records the result as sent, failed or unknown. A timeout gives "unknown"; the worker does not retry it blindly.
- Each message has a `Message-ID` from tada, so that a later bounce can be matched.

Content:

- Subjects and texts come from Fluent messages (ADR 0005) in the recipient's locale.
- Each message has a plain text part and an HTML part. The HTML is simple and works without images or remote content.
- Links use only `TADA_PUBLIC_URL` (ADR 0025). The app never builds a link from request headers.
- Messages contain no record content beyond what the recipient may see. A reminder names the action and links to it.

Requirements for the operator (in the deployment repository):

- A sending domain with SPF, DKIM and DMARC.
- SMTP credentials as a secret file (ADR 0036).

Not now:

- Automatic processing of bounces and complaints. The worker logs SMTP rejections, and the integration health view shows the failure rate. Processing comes with ADR 0046 or later.

## Consequences

- An operator can use any SMTP provider without code changes.
- A rolled-back command never sends mail.
- A member gets German mail texts from the same Fluent files as the UI.

## Alternatives

- A provider API from the start: better bounce data, but a lock-in to one provider.
- Direct sending from a command: a mail could go out for a change that then rolls back.
- HTML-only mail: worse for screen readers and plain text clients.
