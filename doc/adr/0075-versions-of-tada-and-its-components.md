# 0075. Versions of tada and its components

- Status: Accepted
- Date: 2026-10-10

## Context

The product owner wants to see the version of each component in the footer of the web client.
An operator must know which versions run before and after an upgrade.
tada has no release version yet.
All crates use the workspace version `0.1.0`, and the image has the Git commit SHA as its only tag ([ADR 0028](0028-images-and-registry.md)).

Each component is not a separate deployment unit.
One image holds the binary and the web client, and the binary starts each process role ([ADR 0002](0002-monorepo-and-services.md), [ADR 0025](0025-platform-contract.md)).
But during an upgrade, the components can still run different versions for a short time:

- The roles `serve`, `worker` and `telegram` restart one after another.
- A browser tab keeps the old web client until it loads the page again.
- The database schema changes before the new roles start.

Some contracts already have their own version: the API path `/api/v1` ([ADR 0017](0017-api-contract-rust.md)), the export format ([ADR 0059](0059-structured-export.md)) and the version of each job kind ([ADR 0054](0054-job-queue-implementation.md)).

## Decision

Product version:

- tada has one product version in the format of Semantic Versioning 2.0.0: `MAJOR.MINOR.PATCH`.
- The `version` of the workspace `Cargo.toml` is the only source. A check makes sure that `apps/web/package.json` has the same version.
- Before the first installation with real data, the major version is 0. A minor version marks a completed sub-slice; a patch version marks fixes only.
- Version 1.0.0 is the first release that a club runs with real data.
- From 1.0.0, a major version marks a release whose upgrade needs a manual step of the operator, or that removes an API version.
- A release is a Git tag `vMAJOR.MINOR.PATCH` on `main`. The image gets the version as a second tag. The deployment still pins the digest (ADR 0028).

Build identity:

- The build writes the product version, the commit SHA and the build time into the binary and into the web client.

Contract versions:

| Contract                            | Version                                                                                      |
| ----------------------------------- | -------------------------------------------------------------------------------------------- |
| HTTP API                            | The major version in the path, `/api/v1`. The OpenAPI `info.version` is the product version. |
| Database schema                     | The number of the last migration, for example `26`.                                          |
| MCP tools                           | The product version in the server information of the MCP handshake.                          |
| Export, deletion journal, job kinds | Their own format versions, as their ADRs define.                                             |

Upgrade of one component at a time:

- A migration must keep the previous release working: expand first, contract in a later release.
  So the roles can restart one after another.
- A binary does not start against a schema that is newer than the binary knows.
- Each role writes its version into its heartbeat.
- The web client compares its build version with the version of the server.
  When they differ, it asks the member to load the page again.
- If a component becomes a separate service later (ADR 0002), it gets its own semantic version and talks to the core through the versioned API.

Where tada shows the versions:

- The footer of the web client shows the product version and the short commit SHA, for example `tada 0.3.0 · 1a2b3c4`.
- A selection of the footer opens a list of the components.
  The list shows the web client, the schema version, the API version and the MCP version.
  It also shows each running role with its version and the time of its last heartbeat.
  The list marks each component that runs a different version.
- `GET /api/v1/system/version` returns this list. Only signed-in members can call it.
- tada shows no version to clients that are not signed in, so that an attacker cannot select exploits by version.

## Consequences

- An operator and a member can see which versions run and whether an upgrade is complete.
- Each release needs a tag and an entry in the changelog of the release.
- Migrations need the expand and contract discipline. A review checks it.
- The components still share one image. An upgrade of one component alone needs a split under ADR 0002 first.

## Alternatives

- Calendar versions, for example `2026.10.1`: they do not tell an operator whether an upgrade needs a manual step.
- A separate version for each crate: the crates are not released alone, so the numbers would carry no information.
- One image for each component now: no recorded reason under ADR 0002, and more work for each release.
- The version on the sign-in page: it helps attackers more than members.
