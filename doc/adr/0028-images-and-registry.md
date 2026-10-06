# 0028. Container images and registry

- Status: Proposed
- Date: 2026-10-06

## Context

All runtimes pull the same image (ADR 0025).
The repository is public.
A deploy must run exactly the image that CI built and tested.

## Decision

- GitHub Actions builds the image on each push to `main`. A multi-stage build compiles the Rust binary and the web client.
- The final image is a minimal base (distroless or `scratch` with CA certificates) that contains only the `tada` binary and the built web files.
- The image goes to the GitHub Container Registry (GHCR) as `ghcr.io/zarubaf/tada`, tagged with the Git commit SHA.
- CI creates a build provenance attestation with `actions/attest-build-provenance`.
- The deploy scripts pin the image by its digest, not by its tag.
- Before a production deploy, the deploy script checks the attestation with `gh attestation verify`.

## Consequences

- GHCR is free for public images, so this adds no cost.
- Every runtime that can pull from a registry can run tada. A move to another registry changes only the image name.
- A deployed image always traces back to one commit and one CI run.

## Alternatives

- Docker Hub: rate limits for pulls, and one more account.
- Builds on the VM: the deployed binary differs from the tested binary, and the build uses the memory of the VM.
- Cosign with a key pair: more key management than the GitHub attestation, for the same result.
