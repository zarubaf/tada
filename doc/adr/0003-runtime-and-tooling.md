# 0003. Rust for the backend, TypeScript only for the web client

- Status: Proposed
- Date: 2026-10-06

## Context

The product owner wants compiled code for everything except the parts that must be TypeScript.
Only the web client must be TypeScript, because it runs in the browser.
AGENTS.md asks for illegal states to be unrepresentable.
The tool versions must be the same on each laptop and in CI.

## Decision

Backend:

- Language: Rust, edition 2024, stable toolchain. `rust-toolchain.toml` pins the version (1.99 at the time of this ADR).
- Async runtime: Tokio.
- Format and lint: `rustfmt` and `clippy` with warnings as errors.
- Tests: `cargo nextest`. Integration tests use `testcontainers` with real PostgreSQL and S3.
- Dependencies: `cargo-deny` checks licenses (ADR 0014), security advisories and duplicate crates.
- Domain types use enums and newtypes. A constructor checks each value, so other code cannot create an invalid value.

Web client:

- Language: TypeScript with `strict` mode, on Node.js with pnpm.
- Node.js: the current Active LTS line, now 24. We move to 26 after it becomes LTS.
- Format and lint: Biome.
- Tests: Vitest.

Shared:

- `mise.toml` pins all tools, except the Rust toolchain, and defines the project tasks. CI runs `mise run check`.

## Consequences

- The backend has one language, and the compiler finds many errors before run time. This gives agents fast feedback.
- Rust builds are slower than TypeScript builds. CI needs a build cache.
- Some libraries are less mature than their TypeScript equivalents. ADRs 0007, 0008 and 0011 name the gaps.
- Anthropic publishes no official Rust SDK (ADR 0010).

## Alternatives

- TypeScript backend: mature authentication and job libraries, but not compiled.
- Go: fast builds and an official Anthropic SDK, but the product owner prefers Rust.
- A TypeScript core with Rust services: two backend languages and two domain models.
