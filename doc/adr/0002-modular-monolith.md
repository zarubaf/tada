# 0002. Modular monolith with enforced boundaries

- Status: Proposed
- Date: 2026-10-06

## Context

The team is small and the operations budget is less than CHF 100 per month.
Separation of concerns is the root principle of this project (see [AGENTS.md](../../AGENTS.md)).
Web, Telegram, the worker and AI tools must use the same authorized domain commands.
Review alone does not keep boundaries intact over time.

## Decision

We build one server codebase with two processes: the HTTP server and the worker.
The server code has code modules by bounded context, for example `identity`, `events`, `documents`, `provenance` and `assistant`.

```text
apps/server/src/
  modules/<context>/
    domain/   types, rules and state transitions; no I/O imports
    app/      domain commands, queries and ports
    infra/    adapters that implement the ports
    index.ts  the only public entry of the code module
  entry/      http.ts, telegram.ts, worker.ts (composition roots)
packages/contracts/  API schemas shared with the web client
apps/web/            the web client
```

dependency-cruiser checks these rules in CI:

1. `domain` imports only `domain` of the same code module.
2. `app` imports `domain` and its own ports.
3. `infra` implements ports and does not export to other code modules.
4. A code module imports another code module only through its `index.ts`.
5. Only the composition roots in `entry/` import `infra`.
6. `apps/web` imports only `packages/contracts` and the generated client.

## Consequences

- One deployment, one database and one log stream.
- A code module can move to its own service later because its contract is already narrow.
- CI fails when a change crosses a boundary.

## Alternatives

- Microservices: high operations cost and no team that needs independent deployment.
- One package per code module: more build configuration with no benefit at this size.
- Boundaries by convention only: they erode without an automatic check.
