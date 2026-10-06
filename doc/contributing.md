# Contributing

This guide describes how humans and LLM agents work on tada.
[AGENTS.md](../AGENTS.md) contains the design principles.
The [writing guide](writing.md) contains the rules for prose.

## Setup

1. Install [mise](https://mise.jdx.dev).
2. Run `mise run setup`. This installs the pinned tools and the Git hooks.
3. Run `mise run check`. All checks must pass before you start.

## Sources of truth

| Topic                       | File                                   |
| --------------------------- | -------------------------------------- |
| Product scope and users     | [doc/PRODUCT.md](PRODUCT.md)           |
| Architecture                | [doc/ARCHITECTURE.md](ARCHITECTURE.md) |
| Decisions and their reasons | [doc/adr/](adr/README.md)              |
| Terms                       | [doc/glossary.md](glossary.md)         |
| Tool versions and tasks     | [mise.toml](../mise.toml)              |

Do not keep project state only in chat or in LLM memory.
If a decision or a fact matters later, write it into one of these files.

The `tasks/` folder holds private working notes.
Git ignores it.
Do not commit it, and do not link to it from tracked files.

## Changes

1. Refactor to the principle first, then change behavior. Use separate commits.
2. Change the docs, the ADRs and the code in the same pull request when they belong together.
3. Add an ADR when a change affects a public contract, a dependency, the data model or the operations.
4. Run `mise run check` before you push.

Do not change an accepted ADR to reverse its decision.
Write a new ADR and set the old one to "Superseded by NNNN".

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org):

```text
<type>(<scope>): <summary>
```

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- Mark a breaking change of a public contract with `!`, for example `feat(api)!: ...`.
- Keep the subject at 72 characters or fewer, in the imperative mood.
- Make each commit one logical change that builds and passes the checks.
- Do not mix formatting changes with content changes.

The `commit-msg` hook and CI check the subject.

## History

- `main` has a linear history. Merge pull requests with rebase or squash.
- Do not force-push to `main`.
- Clean up a feature branch with `git rebase` before review, not after approval.

## Session handoff

End each working session with a short note in the pull request or commit body:

- What changed.
- The evidence: tests, checks and demonstrations.
- The unresolved decisions.
- The next demonstrable slice.

## Public repository

This repository is public.
Do not commit real personal data, club contacts, credentials or unpublished agreements.
Use invented data or confirmed public facts in fixtures.
