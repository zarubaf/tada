# 0012. Documentation language, style and checks

- Status: Accepted
- Date: 2026-10-06

## Context

Humans and LLM agents read and write the documentation.
An agent copies the words it reads, so unclear terms spread.
Formatting noise in diffs hides the real changes.

## Decision

- All documentation, code identifiers and comments use US English.
- Prose follows a subset of ASD-STE100 (Simplified Technical English). [doc/writing.md](../writing.md) describes the rules.
- The [glossary](../glossary.md) defines each term with one meaning.
- Vale checks the prose with the built-in `Vale` style and the project style `Tada` in `.vale/styles/Tada/`.
- Only Vale errors fail CI: banned words, spelling and British spelling. Style findings are warnings.
- mdformat formats all Markdown, with the `gfm` and `frontmatter` plugins and `wrap = "keep"`.
- We write one sentence per line.
- `AGENTS.md` is the single file of agent instructions. `CLAUDE.md` only imports it.
- The UI text is German through i18n keys (ADR 0005). This ADR covers the repository only.

## Consequences

- `mise run check:docs` runs both checks locally, in the Git hook and in CI.
- A new term needs a glossary entry and, for spelling, a Vale vocabulary entry.
- The first briefs used British spelling. We convert them to US spelling in a separate commit.

## Alternatives

- No prose linting: terms drift, and agents copy the drift.
- markdownlint and Prettier: they check layout but not the language.
- Hard line wrapping: one changed word moves all line breaks of the paragraph and hides the change.
