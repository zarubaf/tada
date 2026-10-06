# 0051. Document drafts and provenance

- Status: Proposed
- Date: 2026-10-06

## Context

Slice 1 produces a German concept and an enquiry draft. Each asserted fact must trace to an accepted field or an exact source version (see [roadmap](../roadmap.md)).
A member's agent writes the drafts through MCP (ADR 0040). AI can only propose (ADRs 0010 and 0039).
When a fact changes, dependent documents must show it, and two versions must be comparable.
Approved versions are immutable (ADR 0009), except for a legal redaction (ADR 0045).
An agent can write text that disagrees with the fact it cites, and it can write raw HTML into Markdown.

## Decision

Creation:

- An agent proposes a draft with the operation "create document draft" (ADR 0050).
- A draft version exists only after a member accepts this proposal. No tool writes a document version directly.

Format:

- A draft is Markdown (CommonMark with tables), UTF-8, with one sentence per line.
- A claim that depends on a fact is a link with empty text to an exact fact version: `[](tada:fact/<fact-uuid>?v=3)`.
  The renderer fills in the formatted value of this version. A fact link with text is rejected, so that the text cannot disagree with the fact.
- A claim that depends on a source is a link to a passage (ADR 0050): `[supporting words](tada:source/<source-version-uuid>#<start>-<end>)`.
- A draft can cite accepted facts, assumptions and unknowns, never open proposals.
  - The renderer marks an assumption with „Annahme“.
  - The renderer shows an unknown as „unbekannt“.

The `tada:` scheme:

- `tada:` is a private URI scheme. It has a meaning only inside tada.
- Other Markdown tools can show these links as plain text or remove them. Exports therefore replace them (see below).
- This differs from the problem `type` of ADR 0037, which is public and must resolve for any client.

Rendering and security:

- The renderer disables raw HTML. HTML in a draft shows as text.
- The sanitizer allows only the link schemes `https`, `mailto` and `tada`.
- A reader must have access to every target of the provenance manifest. If a target is not visible for the reader, the renderer shows „entfernt“ instead of its content.
- A target with a legal redaction (ADR 0045) also renders as „entfernt“.

Provenance manifest:

- When tada stores a draft proposal, it extracts all `tada:` links into a provenance manifest: the list of fact versions and source passages that the draft uses.
- tada rejects a draft proposal if a link does not resolve, or if the proposing caller cannot see its target.
- The manifest is part of the document version and never changes.

Draft lint:

- A deterministic check runs on each draft proposal. Numbers, dates and money amounts outside a `tada:` link produce warnings in the review.
- The warnings do not block. The reviewer decides.

Changes and differences:

- When a fact gets a new version, tada marks each document whose newest version uses an older version of this fact. This includes a fact that was unknown and is now known. The document shows „Fakten geändert“. tada never rewrites the document.
- A new draft version is a new document version. Approved versions stay unchanged.
- The difference between two versions has two parts:
  1. a text difference by line, which with one sentence per line is a difference by sentence,
  2. a fact difference from the two manifests: which facts changed, appeared or disappeared.

Exports:

- An export to PDF or another format replaces each fact link with its rendered value and a numbered reference to an evidence appendix. It replaces each source link with its text and a numbered reference.
- The renderer is chosen in the walking skeleton.

## Consequences

- A fact in a concept always shows the value of the cited version, and is traceable with one click.
- A changed or newly known date shows which documents need a new version.
- Factual sentences without a link are not fully detectable. The lint finds numbers and dates; the reviewer checks the rest.
- In Slice 1, the member's agent writes the drafts, so tada's evaluation set (ADR 0048) cannot test this agent. The lint and the review are the checks.

## Alternatives

- Link text that the agent writes: the text can disagree with the fact.
- A separate provenance file next to the document: it can drift from the text.
- Inline markers outside Markdown syntax: other tools would show them as noise.
- Word documents as the draft format: no line-based difference, and links are hard to check.
- Automatic regeneration on each fact change: it would change approved content without review.
