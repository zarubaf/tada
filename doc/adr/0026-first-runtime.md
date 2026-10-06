# 0026. Docker Compose as the first runtime, managed by scripts

- Status: Proposed
- Date: 2026-10-06

## Context

ADR 0015 decided Docker Compose on one VM.
This ADR checks that decision against the real VPS and the alternatives, and decides how we operate it.
The available VPS (2 vCPU, 3.7 GiB RAM, 16 GB free disk, Ubuntu 24.04) already runs other personal workloads.
It has about 2 GiB of free memory.
The product owner wants an agent to run most operations through scripts, and a path to Kubernetes later.

## Decision

- The runtime is Docker Compose, as in ADR 0015.
- `deploy/compose/` contains one Compose file for each environment and one shared file for Caddy.
- Idempotent scripts in `deploy/` do all operations. `mise` tasks start them, for example `mise run deploy:staging`.
- The scripts connect to the host over SSH. The host name comes from `deploy/inventory.local`, which Git ignores.
- [doc/operations/deployment.md](../operations/deployment.md) maps each Compose concept to its Kubernetes and Cloud Run equivalent.

Hosts:

- The walking skeleton and Slice 1 demonstrations run on the existing VPS, with invented data only.
- Before real personal data enters tada, production moves to a VM that runs only tada.
  The existing VPS runs other workloads, so a fault or a compromise there can reach tada data.

## Consequences

- No control plane uses memory or needs operation. Compose fits into the free memory of the VPS.
- The move to Kubernetes is a translation of manifests, not a code change (ADR 0025).
- The scripts are code: they need review and tests on a fresh VM.
- A dedicated production VM adds a monthly cost (see the deployment document).

## Alternatives

- Single-node k3s: Kubernetes manifests from the start, but its control plane needs about 0.5–1 GiB of memory, which the shared VPS does not have.
- Kamal: good zero-downtime deploys, but its own proxy and a Ruby tool chain, and it does not manage Garage or PostgreSQL well.
- Nomad: a scheduler for a cluster that we do not have.
- Coolify or Dokploy: a web control plane whose state is outside Git, and one more service to secure.
