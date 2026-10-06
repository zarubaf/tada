# 0051. Document drafts and provenance

- Status: Proposed
- Date: 2026-10-06

## Context

Slice 1 produces a German concept and an enquiry draft. Each asserted fact must trace to an accepted field or an exact source version (see [roadmap](../roadmap.md)).
A member's agent writes the drafts through MCP (ADR 0040).
When an accepted fact changes, dependent documents must show it, and two versions must be comparable.
Approved versions are immutable (ADR 0009).

## Decision

Format:

- A generated draft is Markdown (CommonMark with tables), UTF-8, with one sentence per line.
- A claim that depends on a fact or a source is a Markdown link with the `tada:` scheme:
  - a fact version: `[15,000–25,000 visitors](tada:fact/<fact-uuid>?v=3)`,
  - a source passage: `[…](tada:source/<source-version-uuid>#<passage>)`.
- These links are valid Markdown. Other tools show them as normal links. The web client opens the evidence panel for them.
- The web client marks a link to a fact with the status `assumption` with „Annahme“. A link to an `unknown` fact is not permitted; the text must state the unknown.

Provenance manifest:

- When tada stores a draft version, it extracts all `tada:` links into a provenance manifest: the list of fact versions and source passages that the version uses.
- tada rejects a draft if a link does not resolve, or if the caller cannot see its target.
- The manifest is part of the document version and never changes.

Changes and differences:

- When a fact gets a new version, tada marks each document whose newest version uses an older version of this fact. The document shows „Fakten geändert“. tada never rewrites the document.
- A new draft version is a new document version. Approved versions stay unchanged.
- The difference between two versions has two parts:
  1. a text difference by line, which with one sentence per line is a difference by sentence,
  2. a fact difference from the two manifests: which facts changed, appeared or disappeared.

Exports:

- An export to PDF or another format replaces `tada:` links with numbered references to an evidence appendix.
- The renderer is chosen in the walking skeleton.

## Consequences

- A sentence in a concept is traceable with one click.
- A changed date window shows which documents need a new version.
- Factual sentences without a link are not detectable by code. The AI evaluation set (ADR 0048) measures them, and the reviewer checks them.

## Alternatives

- A separate provenance file next to the document: it can drift from the text.
- Inline markers outside Markdown syntax: other tools would show them as noise.
- Word documents as the draft format: no line-based difference, and links are hard to check.
- Automatic regeneration on each fact change: it would change approved content without review.
