# 0003. TypeScript on Node.js, pnpm and mise

- Status: Proposed
- Date: 2026-10-06

## Context

The briefs propose TypeScript.
The authentication library (ADR 0008) and the web client (ADR 0005) are TypeScript.
The tool versions must be the same on each laptop and in CI.

## Decision

- Language: TypeScript with `strict` mode and ES modules.
- Runtime: Node.js 26. Node.js 26 becomes an LTS release in October 2026.
- Package manager: pnpm workspaces with a committed lock file.
- Tool versions and project tasks: `mise.toml`. CI runs `mise run check`.
- Lint and format for TypeScript: Biome.
- Architecture rules: dependency-cruiser (ADR 0002).
- Tests: Vitest. Integration tests use Testcontainers with real PostgreSQL and S3.

## Consequences

- One command installs all tools: `mise run setup`.
- An update of a tool is one reviewed change to `mise.toml`.
- We must update Node.js before the end of its support in April 2029.

## Alternatives

- Deno or Bun: smaller ecosystem for the libraries in ADRs 0006–0011.
- ESLint and Prettier: two tools and more configuration than Biome.
- npm: no strict dependency isolation between workspace packages.
- Python: the briefs and the authentication library assume TypeScript.
