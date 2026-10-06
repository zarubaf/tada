# 0016. Environments and release promotion

- Status: Accepted
- Date: 2026-10-06

## Context

The team changes tada while real event planning data exists.
A change must meet realistic data before it reaches production.
ADR 0015 puts the deployment on one VM with a budget of less than CHF 100 per month.
Copies of personal data increase the risk and fall under the Swiss data protection act (revDSG), in particular Art. 6 (purpose) and Art. 8 (security).

## Decision

Environments:

| Environment | Where                                           | Data                                      | Lifetime              |
| ----------- | ----------------------------------------------- | ----------------------------------------- | --------------------- |
| local       | Docker Compose on a laptop                      | invented fixtures                         | permanent             |
| CI          | Testcontainers in GitHub Actions                | fixtures                                  | one run               |
| staging     | a separate Compose project on the production VM | a restore of the latest production backup | one release rehearsal |
| production  | a Compose project on the VM                     | real data                                 | permanent             |

Isolation between staging and production:

- Each has its own database, bucket, secrets, domain name and Telegram bot.
- One shared Caddy project routes both domain names. Caddy is the only shared component, because only one process can bind port 443.
- In staging, the composition root does not connect the model adapter and the outbound message adapters. A setting cannot enable them.
- The worker in staging starts paused.

Staging data:

- The deploy script creates staging for a rehearsal and deletes it, with its data, after the rehearsal.
- Backups go to a second provider with append-only credentials. The VM cannot delete backups.
- The production VM has no key that can delete or change backups.
- The privacy notice states that restore tests use production data.

Configuration and secrets:

- Settings that are not secret come from environment variables.
- Secrets come from Compose `secrets:` files, not from environment variables.
- `deploy/SECRETS.md` lists the name, purpose, owner and rotation of each secret. It never contains a value.

Releases:

1. CI builds one image, tagged with the Git commit SHA.
2. The deploy script restores the latest production backup into a new staging project. This also tests the backup.
3. The deploy script runs `tada migrate` on staging as its own step.
4. Smoke tests and a manual check run on staging.
5. A protected GitHub environment needs approval from the product owner.
6. The deploy script runs `tada migrate` on production, then starts the same image digest.
7. The deploy script deletes staging.

`tada serve` never runs migrations at startup.

Rollback:

- An expand-only release can roll back to the previous image. The previous image must tolerate migrations it does not know (ADR 0006).
- A release that removes schema forms (contract) cannot roll back by image. Its rollback is a restore of the backup from before the release.
- The release notes state which kind each release is.

Incomplete features stay behind a feature flag for each organization.

## Consequences

- Each release also tests the backup and the restore.
- Personal data exists in staging only during a rehearsal.
- Staging cannot send messages or call a model provider, so a rehearsal cannot reach real people.
- The VM must have capacity for two Compose projects during a rehearsal.

## Alternatives

- A permanent staging: a second permanent copy of personal data.
- A separate staging VM: better isolation, but a higher cost.
- No staging: the first test of a migration on real data is in production.
- Anonymized staging data: safer, but the anonymizer is one more component, and it does not test the real restore.
