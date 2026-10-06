# 0006. PostgreSQL, Drizzle and reviewed migrations

- Status: Proposed
- Date: 2026-10-06

## Context

Accepted state, proposals, audit events and jobs need transactions and foreign keys.
The schema changes during event planning while real data exists.
A migration must never turn an assumption into a decision.

## Decision

- The database is PostgreSQL 18.
- Drizzle defines the tables in TypeScript inside each code module's `infra` folder.
- `drizzle-kit generate` writes SQL migration files. We commit and review each file.
- We never use `drizzle-kit push` outside a local database.
- Each schema change follows expand and contract:
  1. Add the new column or table.
  2. Backfill the data.
  3. Support the old and the new form.
  4. Remove the old form in a later release.
- Each table with organization data has a non-null `organization_id`.
- Each mutable record has a `version` column for optimistic concurrency.
- Repositories implement ports. Domain code does not import Drizzle.
- Full-text search uses PostgreSQL. We add pgvector only if an evaluation shows a benefit.

## Consequences

- Each migration is visible as SQL in review.
- CI applies all migrations to an empty database and to a fixture database.
- We can change the query library later because only `infra` uses it.

## Alternatives

- Prisma: a separate schema language and a query engine binary.
- Kysely with hand-written migrations: good, but the table types must be kept in sync by hand.
- An ORM with automatic schema sync: no review of the SQL that runs in production.
