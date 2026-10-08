#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check that the data inventory names each table of the migrations (ADR 0045).

Each table of `crates/store-pg/migrations/*.sql` must appear in `doc/data-inventory.md` in backticks,
alone (`session`) or with a column (`session.user_id`).
A table without personal data can instead be on ALLOWED below, with the reason.
The check keeps the inventory in step with the schema: a new table needs a row or a reason in the same change.

Usage: check_data_inventory.py [MIGRATIONS_DIR [INVENTORY_FILE]]
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "crates" / "store-pg" / "migrations"
INVENTORY = ROOT / "doc" / "data-inventory.md"

# Tables without personal data. Each reason says what the table holds.
# A table that can hold personal data needs a row in the inventory instead.
# An `ALTER TABLE` that adds a column to one of these tables needs a new look at its reason.
ALLOWED = {
    "worker_heartbeat": "A worker ID and times of its loop, and the depth of the queue. No data about a person.",
    "job": (
        "The queue of the worker. A payload holds record IDs and purposes only (ADR 0042), "
        "and a failure reason follows the log rule of ADR 0035: no personal data."
    ),
    "telegram_update": "The update ID of a Telegram message and its receive time. No content.",
    "source_item": "The kind of an incoming item, its event and its time. Text and author are in `source_version`.",
    "fact": "The field and the current version number of one fact. The values are in `fact_version`.",
    "proposal_dependency": "Pairs of proposal IDs of one changeset. No content.",
    "local_id_counter": "The next number of an event-local ID kind, for example `QST`. No content.",
    "organization_feature": "One switch of an organization and its version, for example `mcp-tokens`. No person.",
}

# Any `CREATE TABLE`, also with a modifier. The name after it must then be readable, or the check fails.
STATEMENT = re.compile(
    r"^\s*CREATE\s+(?:(?:GLOBAL|LOCAL|UNLOGGED|TEMP|TEMPORARY)\s+)*TABLE\s+(?P<rest>[^\n]*)",
    re.IGNORECASE | re.MULTILINE,
)
# `name`, `"name"`, `schema.name` and `"schema"."name"`, after an optional `IF NOT EXISTS`.
NAME = re.compile(
    r'(?:IF\s+NOT\s+EXISTS\s+)?(?:"?[a-z_][a-z0-9_]*"?\.)?"?([a-z_][a-z0-9_]*)"?(?=[\s(;]|$)', re.IGNORECASE
)
CODE = re.compile(r"`([a-z_][a-z0-9_]*)(?:\.[a-z_][a-z0-9_*]*)?`")


def tables(migrations: Path) -> tuple[dict[str, str], list[str]]:
    """The tables that the migrations create, each with the file of its first `CREATE TABLE`.

    The second value lists the statements whose table name this script cannot read.
    """
    found: dict[str, str] = {}
    unreadable: list[str] = []
    for path in sorted(migrations.glob("*.sql")):
        sql = re.sub(r"--[^\n]*", "", path.read_text())
        for statement in STATEMENT.finditer(sql):
            name = NAME.match(statement["rest"])
            if name:
                found.setdefault(name[1].lower(), path.name)
            else:
                unreadable.append(f"{path.name}: {statement[0].strip()[:60]}")
    return found, unreadable


def named(inventory: str) -> set[str]:
    """The names in backticks that can be a table: `name` and `name.column`."""
    return set(CODE.findall(inventory))


def main(argv: list[str], allowed: dict[str, str] | None = None) -> int:
    allowed = ALLOWED if allowed is None else allowed
    migrations = Path(argv[0]) if argv else MIGRATIONS
    inventory = Path(argv[1]) if len(argv) > 1 else INVENTORY
    found, unreadable = tables(migrations)
    if not found:
        print(f"{migrations} has no CREATE TABLE statement; check the path.", file=sys.stderr)
        return 1
    covered = named(inventory.read_text()) | allowed.keys()
    missing = {table: file for table, file in found.items() if table not in covered}
    stale = sorted(allowed.keys() - found.keys())
    for statement in unreadable:
        print(f"This script cannot read the table name of: {statement}", file=sys.stderr)
    for table, file in missing.items():
        print(f"The table {table} ({file}) is not in {inventory.name}.", file=sys.stderr)
    for table in stale:
        print(f"ALLOWED names {table}, but no migration creates it. Remove the entry.", file=sys.stderr)
    if missing:
        print(
            "Add a row for the personal data of the table to the inventory (ADR 0045).\n"
            "If the table holds no personal data, add it to ALLOWED in scripts/check_data_inventory.py,"
            " with the reason.",
            file=sys.stderr,
        )
    if missing or stale or unreadable:
        return 1
    print(f"The inventory covers {len(found)} tables.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
