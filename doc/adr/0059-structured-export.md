# 0059. Structured export of one organization

- Status: Proposed
- Date: 2026-10-08

## Context

ARCHITECTURE.md ("Safe evolution") requires exports with versioned JSON and CSV, originals, retained versions, hashes and relationship manifests.
A test must rebuild the data from an export.
The roadmap of Slice 1 asks for a demonstration of the structured export and for exports that stay in one organization.
An operator runs the export, not a member.
ADR 0039 fixes the service identities in code.
Only `bootstrap` works without an organization.
The database holds token hashes and other secrets (ADR 0008).
A hash of a token does not give the token.
But it identifies the token, and it has no use outside the installation.

## Decision

The command:

- An operator runs `tada export --organization-slug <slug> --output <directory>`.
- Its settings are `ExportSettings = (Database, Storage)`.
- The command acts as the new service identity `exporter`, with the channel `cli`.
  ADR 0039 requires a code change for a new service identity; this ADR is that change.
- The exporter resolves the slug to an organization ID through one named infrastructure query in `store-pg`.
  It then holds an `OrgScope` and reads only the rows of that organization.
  `bootstrap` stays the only service identity without an organization.
- The command writes into a new or empty directory only.
  It creates the directories with mode 0700 and the files with mode 0600, because the export holds personal data.
- The command reads all rows in one read-only transaction, so the export is one consistent state of the database.

The layout:

- `manifest.json`:
  - `format_version`: 1.
  - `tada_version`: the version of the `tada` binary.
  - `created_at`: the time of the export.
  - `organization_id`.
  - `tables`: for each table, its name, its columns, its row count and its foreign keys (the relationship manifest).
    The tables are in an order where each table comes after the tables that it refers to.
  - `files`: the path, the size and the SHA-256 of each other file of the export.
- `tables/<table>.jsonl`: one JSON object for each row, with the PostgreSQL JSON form of each value.
  This file is the authority of the export.
- `tables/<table>.csv`: the same rows as RFC 4180 CSV with a header line, for spreadsheets.
  An empty field is a null value or an empty text; the JSON Lines file tells them apart.
  A `jsonb` value is its JSON text.
- `blobs/<sha256>`: the original file of each upload version, named by the hex SHA-256 of its content.
  The column `document_version.sha256` names the file.
  The export checks the hash of each file against that column, and fails on a difference or a missing object.

The tables:

- One list in `store-pg` names each table of the schema.
  For an exported table, the list gives the rule that selects its rows.
  For a table that the export leaves out, the list gives the reason.
  The export refuses to run if the schema has a table that the list does not name.
  So a new migration cannot add a table that escapes the export by accident.
- For each exported table, the list also names the exported columns and the secret columns.
  The export refuses to run if the table has a column that the list does not name, or if the list names a column that the table does not have.
  So a new migration cannot add a secret column that goes into the export by accident.
- The export contains the organization, its events, memberships, invitations, outbound intents, audit events, API tokens and switches.
  It also contains the local ID counters, event field definitions, sources, changesets, proposals and review results.
  It also contains the facts, fact versions, evidence links, open questions, documents, document versions and draft manifests.
- It also contains the users that the rows of the organization refer to (`app_user`), with their email identities and Telegram identities.
  A user who is a member of two organizations is in the export of each one.
- It does not contain the shipped field definitions: `tada migrate` writes them with fixed IDs from the code (ADR 0049).
- It does not contain generated columns, for example `source_version.search`.

Never exported, because they hold tokens, token hashes or other secrets:

- The tables `session`, `magic_link`, `invitation_token`, `telegram_link_code` and `rate_limit_counter`.
- The column `api_token.token_hash`.

Not exported, because they hold no organization data:

- `job`: the queue of the worker. A job is pending work, not a record. Its result is a record in another table.
- `worker_heartbeat`, `telegram_update` and `_sqlx_migrations`: state of the installation.

The import:

- Slice 1 has no production import command (YAGNI).
- The test-only import `tada_store_pg::testing::TestDatabase::import_export` rebuilds the data into an empty database.
  It gives each excluded secret column a new random value, so an exported API token never works again.
- A test fills two organizations and exports one.
  It checks that the files hold no row and no text of the other organization, and no token.
  It checks each blob hash, imports the export and compares the data with the source.

## Consequences

- An operator can give a club all of its data in open formats, with the files and their hashes.
- An export holds all personal data of the organization.
  The operator stores, transfers and deletes it like a backup (ADR 0031, ADR 0045).
- A new table or a new column needs a decision in the list of `store-pg`; the export fails until it has one.
- The export holds all rows in memory. This is enough at club scale; a larger organization needs a streaming export.
- An import into another installation must use the same schema version. A production import needs a new ADR.

## Alternatives

- `pg_dump` of the rows of one organization: it has no relationship manifest and no originals, and it includes the secrets.
- An export through the HTTP API for owners: the API shows what a member can see, not all data.
  A download of all personal data also needs more protection than a session.
- Only JSON, without CSV: a club without a developer cannot open it in a spreadsheet.
