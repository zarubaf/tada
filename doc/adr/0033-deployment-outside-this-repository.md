# 0033. Deployment outside this repository

- Status: Accepted
- Date: 2026-10-06

## Context

ADRs 0015, 0016 and 0026–0032 described one operator's deployment: the domain, the providers, the host and the scripts.
The product owner does not want operator details in the public repository.
These details also coupled the product to one way of running it.

## Decision

- This repository contains the product and its platform contract ([ADR 0025](0025-platform-contract.md)) only.
- This repository also builds and publishes the image ([ADR 0028](0028-images-and-registry.md)).
- Each operator keeps the deployment in a separate repository: runtime, ingress, DNS, secrets delivery, provisioning, backups, environments and release approvals.
- The first operator's deployment repository is private.
- The ADRs above have the status "Moved". The Git history keeps their text.

## Consequences

- The product contains no host names, provider names or operation scripts.
- A change that the deployment needs from the app goes through the platform contract, with a tada ADR.
- Release rules that concern the data, for example expand-and-contract migrations, stay in [ADR 0006](0006-persistence.md).

## Alternatives

- Deployment in a `deploy/` folder of this repository: public operator details, and a tight coupling of product and operation.
- A deployment folder with placeholders only: the real values still need a private place, so the split happens anyway.
