# Migrations

Each file is one migration of the database schema (ADR 0006).
Never change a migration after it is on `main`.

A new migration takes the next number after the highest number in this folder.
The numbers 0016 and 0018 stay unused.
An existing database applies a migration with a lower number after the newer migrations, and a new database applies it before them.
The two databases then have different schemas.
A unit test in `src/database.rs` names each migration number, so add the new number there too.
