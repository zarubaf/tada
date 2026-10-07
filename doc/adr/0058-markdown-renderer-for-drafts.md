# 0058. Markdown parser and renderer for drafts

- Status: Proposed
- Date: 2026-10-07

## Context

ADR 0051 defines the draft format, the `tada:` scheme, the provenance manifest and the draft lint.
It leaves the choice of the renderer to the walking skeleton.
The server must read each draft for three jobs: the provenance manifest, the lint and, later, the export.
The web client must show a draft to a reader with the rules of ADR 0051:

- Raw HTML shows as text.
- Only the link schemes `https`, `mailto` and `tada` are allowed.
- A fact link shows the formatted value of the cited fact version, „Annahme“, „unbekannt“ or „entfernt“.

The comparison of two versions needs a text difference by line (ADR 0051).
A new dependency needs an ADR (see [contributing](../contributing.md)).

## Decision

Server:

- The `app` crate parses drafts with `pulldown-cmark` 0.13.4, without its default features.
  Tables are on.
  All other extensions are off.
- The server never makes HTML from a draft.
  A raw HTML block or inline HTML is an event of the parser that the server keeps as text.
  It never gives a link or a manifest entry, and the lint gives a `RawHtml` warning for it.
- The manifest extraction checks the destination of each link and each image:
  - A destination needs the scheme `https`, `mailto` or `tada`.
    The extraction rejects other schemes and relative destinations.
  - The extraction rejects an image with a `tada:` destination, because a `tada:` target is not an image.
  - A fact link must have empty text, and its URI must be `tada:fact/<uuid>?v=<n>` with `n` of 1 or more.
  - A source link URI must be `tada:source/<uuid>#<start>-<end>` with `start` less than `end`.
- The server normalizes the line endings of a draft to LF before it parses the draft.
- The export (ADR 0051) uses the same parser.
- The text difference by line uses `similar`.

Web client:

- The web client renders drafts with `react-markdown` and `remark-gfm`.
- It sets `skipHtml`, so that raw HTML never becomes elements.
- Its `urlTransform` keeps only `https`, `mailto` and `tada` destinations and removes all others.
- A custom link component resolves each `tada:` link from a map that the server sends with the draft.
  The map holds the rendered value and state of each target for this reader.
  A missing entry renders „entfernt“.

Export:

- The PDF export comes in a later slice.
  This ADR does not choose its tool.

## Consequences

- The server and the client use different parsers.
  A draft that the two parsers read differently can show a link that the manifest does not have.
  The client therefore resolves `tada:` links only from the server map, and the `urlTransform` blocks all other schemes.
- `remark-gfm` also enables other extensions of GitHub Flavored Markdown in the client, for example task lists and bare web addresses as links.
  The server ignores them.
  They change only the presentation, and the `urlTransform` checks each link.
- No sanitizer of HTML is necessary, because no component makes HTML from draft text.
- We own a small parser of the `tada:` links and its tests.

## Alternatives

- HTML from the server with `pulldown-cmark` and `ammonia` as the sanitizer: the client would insert server HTML, and a sanitizer configuration would be the only defense.
- `comrak` on the server: it follows GitHub Flavored Markdown more closely, but the three jobs need only CommonMark with tables, and a pull parser is enough for them.
- A Markdown renderer of our own in the client: more code for us, and no gain over `react-markdown` with a strict `urlTransform`.
- `diff` or `dissimilar` for the text difference: `similar` has a line mode and is widely used.
