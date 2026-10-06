# 0031. Backups

- Status: Proposed
- Date: 2026-10-06

## Context

ADRs 0015 and 0016 decided that backups go to a second provider with append-only credentials, and that each release tests a restore.
The backup must contain the database and the objects in a consistent state.
Objects are immutable and the database refers to them, so an object must exist for each reference in the database.

## Decision

- `restic` makes the backups. It encrypts and deduplicates them.
- The repository is a Backblaze B2 bucket in an EU region. Backblaze is a different provider from Hetzner.
- The VM uses a B2 application key that cannot delete files. Restic can add snapshots but cannot remove them.
- A separate key, held by the product owner outside the VM, runs `restic forget --prune` on a schedule.
- The restic password is a secret (ADR 0029). The product owner keeps a second copy offline.
- Each backup run does these steps in this order:
  1. `pg_dump` in custom format of each production database.
  2. A copy of all objects from the S3 bucket. Objects never change, so a copy after the dump contains every object that the dump refers to.
  3. A manifest with the image digest, the migration version and the SHA-256 hashes.
  4. One `restic backup` of the dump, the objects and the manifest.
- Retention: 7 daily, 4 weekly and 12 monthly snapshots.
- Erasure: a deleted record leaves the backups when its last snapshot expires, at most 12 months later. The privacy notice states this.

## Consequences

- A compromised VM can add backups but cannot delete them.
- A restore needs only the restic password and a read key, so a staging rehearsal (ADR 0016) and a disaster restore use the same steps.
- The B2 cost for the expected data volume is below CHF 1 per month (unverified estimate).
- On a cloud platform, the same restic job runs as a scheduled job.

## Alternatives

- Hetzner Storage Box: cheap, and its snapshots protect against deletion, but it is the same provider as the VM.
- Cloudflare R2: its API tokens cannot exclude delete, so the VM could delete backups.
- Borg: good, but it needs SSH on the target and has no native S3 support.
