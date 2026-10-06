# 0041. CI build and dependency policy

- Status: Accepted
- Date: 2026-10-06

## Context

The first Rust code arrives with the walking skeleton.
Rust builds in CI take minutes without a cache.
ADR 0028 defines the image, the registry and the attestation, but not how CI builds the image efficiently.
The repository is public, so an attacker can see each dependency and each workflow.
Current versions on 6 October 2026: `Swatinem/rust-cache` v2.9.2, `docker/build-push-action` v7.4.0, `cargo-chef` 0.1.78, `sccache` 0.18.0, Renovate 44.139.0.

## Decision

Workflows:

- One `check` workflow runs on each pull request and each push to `main`: `mise run check`, the Rust checks (`fmt`, `clippy`, `nextest`, `cargo-deny`) and the web checks (Biome, Vitest, the build).
- One `image` workflow runs on each push to `main` after `check` passes. It builds the image, pushes it to GHCR and creates the attestation (ADR 0028).
- The required checks for `main` are the jobs of the `check` workflow.
- All actions are pinned to a commit SHA, with the version in a comment.
- Workflows have `permissions: contents: read` by default. Only the `image` workflow gets `packages: write`, `id-token: write` and `attestations: write`. The daily advisory workflow gets `issues: write`.

Caches:

- Rust jobs use `Swatinem/rust-cache`. It caches the Cargo registry and the `target` folder by lock file and toolchain.
- Rust builds in CI and in the image set `SQLX_OFFLINE=true` and use the committed `.sqlx` query cache (ADR 0006).
- The image build uses `cargo-chef` in the first stage, so that a source change does not rebuild all dependencies. Docker layers use the GitHub Actions cache of `docker/build-push-action`.
- The web build uses the pnpm store cache.
- We do not use `sccache` now. It needs a storage backend and helps mostly for many parallel builds.

Image:

- The build stages use Debian 13 (trixie), so that the C runtime of the build matches the final stage.
- The final stage is `gcr.io/distroless/cc-debian13:nonroot`. The name includes the Debian release, because the name without it moves to new releases. It contains the CA certificates and the C runtime that the binary needs.
- The time zone database is in the binary (ADR 0038), so the image needs no system time zone files.
- The image contains only the `tada` binary and the built web files.

Dependencies:

- Renovate opens update pull requests. It groups updates by ecosystem (Cargo, pnpm, GitHub Actions, Docker base image) and runs once a week.
- Renovate pins Docker base images and GitHub Actions by digest or SHA, through the presets `docker:pinDigests` and `helpers:pinGitHubActionDigests`.
- Renovate waits for a minimum release age of three days before it proposes a new version, so that a broken or malicious release has time to be found.
- Security updates bypass the weekly schedule. Renovate needs the GitHub vulnerability alerts of the repository for this, so these alerts are enabled.
- A scheduled workflow runs `cargo deny check advisories` and `pnpm audit` each day. A finding opens an issue.
- Lock files (`Cargo.lock`, `pnpm-lock.yaml`) are committed.

## Consequences

- A typical CI run reuses the compiled dependencies.
- An update arrives as a reviewed pull request, never as a silent change.
- A new security advisory shows up within one day, also when nobody pushes.

## Alternatives

- Dependabot: built into GitHub, but weaker grouping and no digest pinning of all ecosystems in one tool.
- `sccache` with a cloud bucket: a storage backend to operate, with little gain for one build at a time.
- `scratch` as the final stage: the binary would need static linking and its own CA certificates.
