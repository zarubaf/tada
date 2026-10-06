# 0016. Environments and release promotion

- Status: Proposed
- Date: 2026-10-06

## Context

The team changes tada while real event planning data exists.
A change must be tested on realistic data before it reaches production.
ADR 0015 puts the deployment on one VM with a budget of less than CHF 100 per month.

## Decision

We use four environments:

| Environment | Where                                           | Data                                      | Purpose           |
| ----------- | ----------------------------------------------- | ----------------------------------------- | ----------------- |
| local       | Docker Compose on a laptop                      | invented fixtures                         | development       |
| CI          | Testcontainers in GitHub Actions                | fixtures, created for each run            | automatic tests   |
| staging     | a separate Compose project on the production VM | a restore of the latest production backup | release rehearsal |
| production  | a Compose project on the VM                     | real data                                 | live use          |

Staging and production share nothing except the VM:

- Each has its own database, bucket, secrets and domain name, for example `staging.<domain>`.
- Each has its own Telegram bot.
- Staging sends mail only to an allow-list of test addresses.
- The worker in staging starts paused. An operator enables jobs.

Each release follows these steps:

1. CI builds one image for each service, tagged with the Git commit SHA.
2. The deploy script restores the latest production backup into staging. This also tests the backup.
3. The deploy script deploys the same images to staging and runs the migrations.
4. Smoke tests and a manual check run on staging.
5. A protected GitHub environment needs approval from the product owner.
6. The deploy script deploys the same image digests to production.

Configuration comes only from environment variables.
The images are the same in all environments.

Migrations in a release only expand the schema (ADR 0006), so the previous image still works.
A rollback deploys the previous image.
A later release removes the old schema forms.

Incomplete features stay behind a feature flag for each organization.

## Consequences

- Each release also tests the backup and restore.
- Staging contains real personal data. It needs the same access rules as production.
- The VM must have capacity for two Compose projects.
- A failed migration in staging stops the release before it reaches production.

## Alternatives

- A separate staging VM: better isolation, but a higher cost.
- No staging: the first test of a migration on real data is in production.
- Anonymized staging data: safer, but the anonymizer is one more component, and it does not test the real restore.
