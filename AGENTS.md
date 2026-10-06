# Timeless constraints (not a checklist)

When two principles collide, pick the one that cuts future cost in THIS codebase.
HARD RULE: refactor to the principle FIRST, then change behavior.

01. Separation of Concerns - one kind of work per part (UI / domain / persistence / infra). Root principle.
02. Encapsulation / Information Hiding - small stable contract; hide internals.
03. High Cohesion + Loose Coupling - change-together lives together; independents talk narrow.
04. DRY - one authoritative representation of each piece of knowledge (not every similar line). Avoid over-DRY.
05. KISS - simplest design that works; complexity is the long-term tax.
06. Single Responsibility - one reason to change.
07. Depend on Abstractions - policy doesn't depend on details; both depend on contracts.
08. YAGNI - no speculative features, frameworks, or "later" hooks.
09. Composition over Inheritance - assemble pieces; don't grow fragile hierarchies.
10. Open/Closed (with discipline) - extend at stable boundaries; only where change showed up twice.
11. Honorable: Law of Demeter • fail fast / illegal states unrepresentable • optimize for deletion
12. Unix do-one-thing + compose.
13. Treat as constraints. Violate slogans when judgment says so.

# Project context

tada is an event-planning workspace for clubs. Read these files before you change anything:

- [doc/contributing.md](doc/contributing.md): setup, commits, history and session handoff.
- [doc/PRODUCT.md](doc/PRODUCT.md) and [doc/ARCHITECTURE.md](doc/ARCHITECTURE.md): what we build and how.
- [doc/adr/](doc/adr/README.md): the decisions and their reasons. Do not contradict an accepted ADR; propose a new one.
- [doc/glossary.md](doc/glossary.md): one meaning for each term. Use these terms in code and docs.
- [doc/roadmap.md](doc/roadmap.md): the slices and their acceptance criteria.
- [doc/design/](doc/design/README.md): the design system. Read it before any UI work.
- [doc/operations/deployment.md](doc/operations/deployment.md): runtime, releases and approval gates.
- [doc/writing.md](doc/writing.md): US English and Simplified Technical English.

Rules for each session:

1. Run `mise run check` before each commit. The Git hooks run the same checks.
2. Use Conventional Commits. Make one logical change per commit. Explain why, not what.
3. Do not add `Co-Authored-By` or other AI attribution trailers to commits or pull requests.
4. Do not push, create remotes or publish anything without a request from the product owner.
5. Do not commit real personal data, club contacts or secrets. The repository is public.
6. Do not commit or link to `tasks/`. It holds private notes.
7. Write durable decisions into the repository, not into chat or agent memory.
