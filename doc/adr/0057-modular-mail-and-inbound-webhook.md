# 0057. Modular mail adapters and an inbound webhook

- Status: Accepted
- Date: 2026-10-07

## Context

ADR 0042 defines the `Mailer` port and an SMTP adapter for outbound mail.
ADR 0046 defines an inbound mail port and an IMAP adapter. It says that a provider webhook adapter can come later.
The product owner wants mail to be modular in both directions: the `app` crate defines the ports, and configuration selects the adapters.
IMAP and SMTP stay supported.
The first inbound adapter that the product owner will deploy receives mail through a webhook.
The mail provider of an operator is an operator detail (ADR 0033).

The architecture rules for integrations apply (see [ARCHITECTURE.md](../ARCHITECTURE.md), Integrations):

- A webhook only signals that something can have changed. A worker then fetches the authoritative data.
- Delivery is at least once, and processing is idempotent.
- A checkpoint moves only after durable capture.
- "No new mail" must never hide "the connector is disconnected".

We checked the inbound webhooks of four transactional mail providers on 2026-10-07:

| Provider | Request signature                                                                                   | Message in the request                                                                                     | Retries                                       |
| -------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| Resend   | Svix, which follows Standard Webhooks: HMAC-SHA-256 over the request ID, the timestamp and the body | Metadata only. An API call gives the message and a short-lived URL of the raw message. A list call exists. | Svix schedule: 8 attempts over about 27 hours |
| Mailgun  | HMAC-SHA-256 over a timestamp and a token. The signature does not cover the body.                   | Raw MIME in the field `body-mime`, if the URL ends in `mime`                                               | 6 retries over 8 hours                        |
| SendGrid | ECDSA over a timestamp and the body, if the operator enables it                                     | Raw MIME, if the operator enables the option. Messages up to 30 MB.                                        | Not stated on the pages that we checked       |
| Postmark | No signature. Basic authentication in the URL and an IP allow-list                                  | Parsed JSON. Raw MIME in `RawEmail`, if the operator enables the option.                                   | 10 attempts over about 10 hours               |

Sources: the provider documentation pages on inbound mail and on webhook security, and the [Standard Webhooks specification](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md).
Standard Webhooks recommends a timestamp tolerance against replays and the request ID as an idempotency key.
No checked provider sends the raw message with a Standard Webhooks signature in one request.

## Decision

Ports and selection:

- The `app` crate defines the ports. Adapters implement them in the `adapters` crate. The `tada` binary selects an adapter from the settings.
- Outbound: the `Mailer` port of ADR 0042 does not change. SMTP is the only outbound adapter now.
  `TADA_MAIL_OUTBOUND_ADAPTER` comes with the second outbound adapter, with the default `smtp`.
- Inbound: `TADA_MAIL_INBOUND_ADAPTER` selects `none` (the default), `imap` or `webhook`. One installation uses one inbound adapter at a time.
- The inbound and outbound adapters are independent. An operator can use different providers for the two directions.
- Each adapter has its own settings section (ADR 0036). A process loads only the sections of the selected adapters.
- Tests use in-memory adapters. Development uses Mailpit for outbound mail.

Inbound port:

- All inbound adapters deliver to one `app` command that captures one inbound delivery.
- An inbound delivery has the adapter, a delivery key, the envelope recipients and the raw message (RFC 5322).
  The envelope recipients matter, because a member can send a copy as Bcc.
- The delivery key is unique for each adapter:
  - IMAP: the mailbox, its `UIDVALIDITY` and the message UID.
  - Webhook: the message ID of the provider.
- An `inbound_deliveries` table holds one row for each delivery key, with a unique constraint. A second capture of the same key changes nothing.
- The table stores no message content: only the adapter, the delivery key, the state and the times.
- The capture stores the raw message as a source version and adds the parse job in one transaction (ADRs 0007 and 0046).
- The delivery is the key of the parse job. The deduplication of ADR 0046 (`Message-ID` and normalized body) then works across deliveries.

Webhook adapter format:

- The webhook adapter follows the rule "the webhook signals, the worker fetches".
- The request must carry a Standard Webhooks signature. The adapter accepts the header names `webhook-*` and `svix-*`, because Svix uses its own prefix.
- The request names one received message. A worker job then fetches the raw message through the provider API.
- A webhook dialect is the provider-specific part. It reads the message ID from the request body.
  It also fetches and lists raw messages through the provider API.
- `TADA_MAIL_INBOUND_WEBHOOK_DIALECT` selects the dialect.
- Of the checked providers, only Resend meets all three needs: a signature over the body, a fetch of the raw message and a list call. Its dialect is the first one.
- A dialect that receives the raw message in the request needs a new ADR.

Request flow:

1. A provider sends `POST /webhooks/mail/inbound` to `serve`. The URL comes from `TADA_PUBLIC_URL` (ADR 0025).
2. The adapter checks the body size, the timestamp and the signature. It does not touch the database before these checks pass.
3. The adapter ignores event types other than "mail received", and answers 204.
4. The adapter calls the capture command with the delivery key only. The command writes the delivery row and the fetch job in one transaction.
5. The adapter answers 2xx after the commit, also for a known delivery key.
6. The `worker` runs the fetch job. It gets a fresh download URL for each attempt, because the URL expires.
7. The worker streams the raw message to a staging key, checks the size, and completes the capture.

The route is not part of the versioned API (ADR 0017). It is not in `contracts/openapi.json`, because the provider defines its format.
The adapter module provides the route. The binary mounts it in `serve` only if the settings select the webhook adapter.

Signature and replay protection:

- The adapter computes HMAC-SHA-256 over `<webhook-id>.<webhook-timestamp>.<body>` on the raw body bytes, before any parsing.
- It compares each `v1` signature in constant time with the RustCrypto `hmac` crate. The lock file already contains `hmac` and `sha2` (ADR 0056).
- A timestamp more than 5 minutes from the process clock gives 401. Svix uses the same tolerance.
- A replay inside the tolerance finds a known delivery key and changes nothing.
- A request with a missing or wrong signature gets 401 with an empty body.

Secrets (ADR 0036):

- `TADA_MAIL_INBOUND_WEBHOOK_KEY_FILE` contains the signing key in the Standard Webhooks format (`whsec_` and base64). Only `serve` loads it.
- The file can contain two keys, one on each line. The adapter accepts a signature from either key, so that the operator can rotate the key without lost mail.
- `TADA_MAIL_INBOUND_API_KEY_FILE` contains the provider API credential for the fetch and the list. Only the `worker` loads it.
- The operator gives this credential the narrowest scope that the provider offers.

Limits on the public route:

- The request body limit is 64 KiB, because a signal request contains no message. A larger body gets 413.
- A concurrency limit of 32 requests for each `serve` process and a timeout of 10 seconds protect the process. Svix waits 15 seconds for an answer.
- tada keeps no counters for unauthenticated requests. A database write for each bad request would itself be a load vector.
  Network rate limits in front of `serve` are an operator detail.
- The fetch job reads the size from the provider before the download, and it stops a stream above `TADA_MAIL_INBOUND_MAX_BYTES`.
  A larger message gets the state "too large" and goes to triage with a note (ADR 0046).

Recovery after downtime:

- If `serve` is down, the provider retries. The Svix schedule covers about 27 hours.
- If the database is down, the adapter answers 503, and the provider retries.
- A reconciliation job lists the messages at the provider and adds a fetch job for each message ID without a delivery row.
  It runs when the worker starts and every hour. It reads back to its last checkpoint, with an overlap of one hour.
- A fetch job that fails retries with backoff (ADR 0054). The delivery row keeps the state "signaled" until the capture succeeds.
- A message that the provider deletes before tada recovers is lost. The integration health view makes this visible early.
- IMAP needs no reconciliation job: the mailbox keeps the messages, and the UID checkpoint moves only after the capture.

Observability (ADR 0035):

- Each webhook request writes one log line with `dialect`, `outcome` and, if known, the `delivery_id` of tada.
  The outcomes are `accepted`, `duplicate`, `ignored`, `bad_signature`, `stale`, `too_large` and `malformed`.
- Logs never contain addresses, the address token, subjects, headers, bodies, signatures, keys, the request body or the client IP address.
- The integration health view reads the delivery table: the last accepted delivery, the last reconciliation, deliveries without capture, and failed fetches.

Data protection (ADR 0045):

- The provider is a sub-processor of the operator. It keeps a copy of each message for its own retention period.
- The operator sets this retention and states it in the privacy notice.
- The raw message in tada is the evidence. tada does not use the parsed fields or the authentication results from the provider API.

ADR 0033 and provider names:

- The code names the dialects that tada supports, as it names S3 and Telegram. This is product scope.
- The deployment repository says which adapter and which dialect an operator selects, with the account, the domain and the keys.
- No document in this repository says which provider an operator uses.

Changes to ADR 0046:

- The inbound mail port is the capture command above. IMAP and the webhook are two adapters of it. The webhook is no longer "later".
- `TADA_INBOUND_MAX_BYTES` becomes `TADA_MAIL_INBOUND_MAX_BYTES`, so that all mail settings share one prefix. No code uses the old name.
- The `authserv-id` of the receiving mail server is a setting: `TADA_MAIL_INBOUND_AUTHSERV_ID`.
  With a webhook dialect, it is the provider's receiving server. If the raw message has no matching header, tada records no results.
- ADR 0046 rejects mail provider APIs because of the lock-in. Here the API use stays inside one dialect, behind the port, and IMAP stays as an alternative.
- All other rules of ADR 0046 stay.

## Consequences

- An operator changes the inbound or outbound adapter through settings, without a code change.
- The public route accepts only small, signed requests, and it does no database work for a bad request.
- One mail gives one delivery row and one source version, also after retries, replays and reconciliation.
- The first deployment needs no mailbox and no polling delay.
- A provider outage delays the fetch, but the delivery row and the health view show it.
- We maintain a small Standard Webhooks check, one dialect and the reconciliation job.
- `doc/settings.md` and the data inventory get the new settings and the `inbound_deliveries` table.

## Alternatives

- A vendor-neutral format only (Standard Webhooks and the raw message in the body): no checked provider sends it.
  An operator would need to run a relay service.
- A dialect with the raw message in the request first: it allows bodies up to 30 MB on a public route.
  Also, the signature of Mailgun does not cover the body.
- Postmark with basic authentication: a static password in the URL, which proxy logs can contain, and no replay protection.
- The fetch inside the webhook request: a slow provider API makes the request time out, and the provider then retries.
- The `svix` crate: it brings a full API client for one HMAC check.
- Rate limits on the route with the counter table of ADR 0056: each bad request then writes to the database.
- IMAP only: the product owner decided to deploy a webhook first, and polling adds a delay.
- Several inbound adapters at the same time: more settings and more cases, for no current need.
