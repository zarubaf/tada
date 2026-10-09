# Release gates

This checklist names the checks that must pass before a release of tada.
CI runs the automatic checks.
The operator does the backup restoration in the deployment repository of the operator ([ADR 0033](adr/0033-deployment-outside-this-repository.md)).
This file names no host, domain or provider.

## Automatic checks

`mise run check:all` runs these tests, with all other checks:

- The schema upgrade test, `crates/store-pg/tests/upgrade.rs` ([ADR 0006](adr/0006-persistence.md)).
  It applies the migrations up to a cut and loads a fixture of that schema.
  Then it applies all later migrations and checks that the data stays.
  Each slice moves the cut to its own last migration and adds a fixture of that schema.
- The restart test, `crates/tada/tests/restart.rs`: after a restart of `serve` and `worker`, the sessions, the records, the files and the sent links still work.
- The token scan in the same test ([ADR 0008](adr/0008-authentication.md)).
  No table holds a magic link, an invitation, a session, a link code or an API token in plain text.
- The export test, `crates/tada/tests/export.rs`: an empty database that a test rebuilds from an export holds the same data ([ADR 0059](adr/0059-structured-export.md)).
- The browser checks of the privacy notice and the invitation page ([ADR 0045](adr/0045-data-protection.md)).

## Backup restoration

A backup is a copy of the database and the object storage of one installation.
It is not an export.
Before a release, the operator restores a backup and upgrades it to the release:

1. Restore the backup of the database and the object storage into an empty installation.
2. Run `tada export --organization-slug <slug> --output <directory>` with the image of the current release, for each organization.
   Keep these exports as the reference.
3. Run `tada migrate` with the image of the new release.
4. Start `serve` and `worker` of the new release, and sign in as a member with a magic link.
5. Run `tada export` with the image of the new release, for each organization.
6. Compare each new export with its reference:
   - Each row of the reference is in the new export with the same values.
   - Each file of the reference is in the new export with the same SHA-256 hash.
   - The new export can have new columns from new migrations, and new rows from the sign-in, for example its outbound intent.
   - `tada_version` and `created_at` in `manifest.json` can differ.
7. Record the result in the deployment repository.

An export holds all personal data of the organization.
Delete the exports and the restored installation after the check.
