# Contributing

This guide describes how humans and LLM agents work on tada.
[AGENTS.md](../AGENTS.md) contains the design principles.
The [writing guide](writing.md) contains the rules for prose.

## Setup

1. Install [mise](https://mise.jdx.dev).
2. Install [rustup](https://rustup.rs). `rust-toolchain.toml` selects the Rust version.
3. Run `mise run setup`. This installs the pinned tools, the Rust toolchain and the Git hooks.
4. Run `mise run check`. All checks must pass before you start.

## Local runtime

Docker Compose starts PostgreSQL, Garage and Mailpit for development.
The file [compose.yaml](../compose.yaml) is not a deployment ([ADR 0033](adr/0033-deployment-outside-this-repository.md)).

1. Run `mise run dev:up`. This generates the missing secrets into `.dev/secrets/`, starts the services and prepares the storage bucket.
2. Run `mise run dev:serve`. This applies the migrations and starts the API on port 8080.
3. Run `mise run dev:web` in a second terminal. It starts the web client and shows its address.
4. Open Mailpit at `http://127.0.0.1:8025` to read the mail that tada sends.
5. Run `mise run dev:down` to stop the services. The data volumes stay.

Do not remove `.dev/secrets/` while the volumes exist. The database and the storage keep the first secrets.
A debug build acts as the owner of a development organization for each request ([ADR 0053](adr/0053-development-authenticator.md)).

## Sources of truth

| Topic                       | File                                       |
| --------------------------- | ------------------------------------------ |
| Product scope and users     | [doc/PRODUCT.md](PRODUCT.md)               |
| Architecture                | [doc/ARCHITECTURE.md](ARCHITECTURE.md)     |
| Decisions and their reasons | [doc/adr/](adr/README.md)                  |
| Terms                       | [doc/glossary.md](glossary.md)             |
| Personal data we store      | [doc/data-inventory.md](data-inventory.md) |
| Slices and acceptance       | [doc/roadmap.md](roadmap.md)               |
| Design system and UI rules  | [doc/design/](design/README.md)            |
| Tool versions and tasks     | [mise.toml](../mise.toml)                  |

Do not keep project state only in chat or in LLM memory.
If a decision or a fact matters later, write it into one of these files.

The `tasks/` folder holds private working notes.
Git ignores it.
Do not commit it, and do not link to it from tracked files.

## Changes

1. Refactor to the principle first, then change behavior. Use separate commits.
2. Change the docs, the ADRs and the code in the same pull request when they belong together.
3. Add an ADR when a change affects a public contract, a dependency, the data model or the operations.
4. Run `mise run gen` after a change of the settings, the problem codes or the API. Commit the generated files.
5. Run `mise run gen:screenshots` after an intended change of the design. The pull request shows the image difference (ADR 0024).
6. Run `mise run check` before you push.

Do not change an accepted ADR to reverse its decision.
Write a new ADR and set the old one to "Superseded by NNNN".

## Agent-driven implementation

The product owner chose this way of work for LLM agents:

1. A controller session plans the work and keeps its own context small.
2. The controller gives all implementation work to new subagents, one task to each subagent.
3. The controller selects the least capable model that can do the task.
4. A subagent does not start subagents of its own.
5. After each task, a task review checks the compliance with the specification and the code quality.
6. After every few tasks and at the end of a slice, a principles review checks the principles in [AGENTS.md](../AGENTS.md), the compliance with the ADRs and the drift of the architecture.
7. Before a merge, a final review checks the whole branch.

Private plans are private notes that stay out of the repository.
Tool state stays outside the repository.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org):

```text
<type>(<scope>): <summary>
```

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- Mark a breaking change of a public contract with `!`, for example `feat(api)!: ...`.
- Keep the subject at 72 characters or fewer, in the imperative mood.
- Make each commit one logical change that builds and passes the checks.
- Keep the message brief. The body explains why. The diff shows what.
- Do not add `Co-Authored-By` or other AI attribution trailers.
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
