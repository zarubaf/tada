# 0060. Unicode normalization of source texts

- Status: Proposed
- Date: 2026-10-07

## Context

ADR 0050 defines a passage as a character range over the normalized text of a source version, plus the exact quote.
The same visible text can come in different Unicode forms.
For example, „ö“ can be one character or an „o“ with a combining diaeresis.
Telegram, browsers, mail clients and AI clients do not send the same form.
Without one normal form, the character offsets of a passage and the hash of a source version depend on the client.
The Rust standard library has no Unicode normalization.

## Decision

- tada stores the text of a source version in Unicode Normalization Form C (NFC), with `\n` line ends.
- `tada_domain::sources::SourceText::normalize` is the one place that normalizes a source text.
  It converts `\r\n` and `\r` to `\n`, then applies NFC.
- The hash of a source version is the SHA-256 of the normalized text.
- Passages count characters (Unicode scalar values) of the normalized text.
- The domain crate uses the crate `unicode-normalization` for NFC.
  It is a pure Rust crate without I/O.
  `Cargo.lock` already contains it through `url`.

## Consequences

- The same text from two clients gets the same hash and the same passage offsets.
- A client that computes passage offsets must use the normalized text that tada returns, not its own input.
- Passage offsets count Unicode scalar values, not bytes and not UTF-16 code units.
  JavaScript strings and many MCP clients count UTF-16 code units, so these clients convert their offsets before they send a passage.
  For example, an emoji outside the Basic Multilingual Plane is one scalar value but two UTF-16 code units.
- The domain crate has one more dependency.

## Alternatives

- No normalization: the offsets and the hash depend on the client.
- `icu_normalizer`: also in `Cargo.lock`, but its API is larger than the one function that tada needs.
- Normalization Form KC (NFKC): it changes the visible text, for example ligatures and full-width characters, so a quote would not match the original.
