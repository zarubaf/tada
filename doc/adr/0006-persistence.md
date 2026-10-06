# 0006. PostgreSQL, sqlx and reviewed SQL migrations

- Status: Accepted
- Date: 2026-10-06

## Context

Accepted state, proposals, audit events and jobs need transactions and foreign keys.
The schema changes during event planning while real data exists.
A migration must never turn an assumption into a decision.

## Decision

- The database is PostgreSQL 18.
- The `store-pg` crate uses `sqlx` with SQL that the compiler checks against the schema.
- The committed `.sqlx` query cache lets CI build without a database.
- Migrations are plain SQL files in `crates/store-pg/migrations/`. We write and review each file.
- Only the command `tada migrate` runs migrations. The other roles never run them at startup.
- The migrator tolerates applied migrations that the binary does not know, so the previous image still starts after an expand migration.
- A release that removes schema forms (contract) cannot roll back by image. Its rollback is a restore of the backup from before the release.
- Each schema change follows expand and contract:
  1. Add the new column or table.
  2. Backfill the data.
  3. Support the old and the new form.
  4. Remove the old form in a later release.
- Tenant isolation has these layers:
  1. Each table with organization data has a non-null `organization_id`.
  2. Foreign keys between such tables are composite: `(organization_id, id)`. A row cannot point to another organization.
  3. Each repository method takes a typed `OrgScope` argument. A query without a scope does not compile.
  4. Integration tests check isolation with two organizations.
  5. PostgreSQL row-level security is an option for later defense in depth.
- Each mutable record has a `version` column for optimistic concurrency.
- Repositories implement the ports of the `app` crate. The `domain` and `app` crates do not depend on `sqlx`.
- Full-text search uses PostgreSQL. We add pgvector only if an evaluation shows a benefit.

## Consequences

- Each migration is visible as SQL in review.
- A query that does not match the schema fails at compile time.
- CI applies all migrations to an empty database and to a fixture database.

## Alternatives

- Diesel: a query DSL and its own migration format; `sqlx` keeps plain SQL.
- SeaORM: an ORM layer that hides the SQL we want to review.
- Migrations that an ORM generates: no review of the exact SQL.
