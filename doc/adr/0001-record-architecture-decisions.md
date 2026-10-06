# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-10-06

## Context

tada is a co-development between a product owner and LLM agents.
An agent starts each session without memory of earlier sessions.
Decisions that exist only in chat are lost or are decided again in a different way.

## Decision

We record each significant decision as an ADR in `doc/adr/`.
A decision is significant when it affects a public contract, a dependency, the data model, security or operations.
We never edit an accepted ADR to reverse it; a new ADR supersedes it.

## Consequences

- Agents read the ADRs before they change the architecture.
- A pull request that changes a decision includes the ADR.
- The ADR index in `doc/adr/README.md` stays current.

## Alternatives

- Decisions in `ARCHITECTURE.md` only: the file shows the result but loses the reasons and the history.
- Decisions in issues or chat: not versioned with the code, and not available to agents offline.
