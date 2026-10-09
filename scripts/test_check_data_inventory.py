#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests of check_data_inventory.py, with temporary migration folders.

Usage: test_check_data_inventory.py
"""

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

import check_data_inventory as check

INVENTORY = (
    "| Name | `app_user.display_name` | Shows the member |\n| Memberships | `membership`, `event_membership` | x |\n"
)


def run(
    migrations: dict[str, str],
    inventory: str = INVENTORY,
    allowed: dict[str, str] | None = None,
    allowed_columns: dict[str, str] | None = None,
) -> tuple[int, str]:
    """Runs the check on temporary files. Returns the exit code and the text of standard error."""
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        (root / "migrations").mkdir()
        for name, sql in migrations.items():
            (root / "migrations" / name).write_text(sql)
        (root / "inventory.md").write_text(inventory)
        error = io.StringIO()
        with contextlib.redirect_stderr(error), contextlib.redirect_stdout(io.StringIO()):
            code = check.main(
                [str(root / "migrations"), str(root / "inventory.md")], allowed or {}, allowed_columns or {}
            )
        return code, error.getvalue()


class CheckDataInventory(unittest.TestCase):
    def test_passes_when_each_table_is_named(self) -> None:
        code, _ = run({"0001.sql": "CREATE TABLE app_user (id uuid);\nCREATE TABLE membership (id uuid);"})
        self.assertEqual(code, 0)

    def test_fails_for_a_new_table_that_the_inventory_does_not_name(self) -> None:
        code, error = run(
            {"0001.sql": "CREATE TABLE app_user (id uuid);", "0002_new.sql": "CREATE TABLE guest_list (id uuid);"}
        )
        self.assertEqual(code, 1)
        self.assertIn("guest_list (0002_new.sql)", error)
        self.assertNotIn("app_user", error)

    def test_a_name_with_a_column_names_the_table(self) -> None:
        code, _ = run({"0001.sql": "CREATE TABLE app_user (display_name text);"})
        self.assertEqual(code, 0)

    def test_a_column_name_does_not_name_a_table(self) -> None:
        code, error = run({"0001.sql": "CREATE TABLE display_name (id uuid);"})
        self.assertEqual(code, 1)
        self.assertIn("display_name", error)

    def test_a_table_on_the_allow_list_needs_no_row(self) -> None:
        sql = "CREATE TABLE job (id uuid);\nCREATE TABLE telegram_update (update_id bigint);"
        code, _ = run({"0001.sql": sql}, allowed={"job": "A reason.", "telegram_update": "A reason."})
        self.assertEqual(code, 0)

    def test_ignores_a_table_in_a_comment_and_reads_other_spellings(self) -> None:
        sql = "-- CREATE TABLE commented_out (id uuid);\ncreate table if not exists app_user (id uuid);"
        code, error = run({"0001.sql": sql})
        self.assertEqual(code, 0, error)

    def test_reads_quoted_unlogged_temporary_and_schema_qualified_names(self) -> None:
        sql = (
            'CREATE TABLE "quoted_t" (id uuid);\n'
            "CREATE UNLOGGED TABLE unlogged_t (id uuid);\n"
            "CREATE TEMP TABLE temp_t (id uuid);\n"
            "CREATE GLOBAL TEMPORARY TABLE IF NOT EXISTS temporary_t (id uuid);\n"
            "CREATE TABLE public.schema_t (id uuid);\n"
            'CREATE TABLE "public"."both_quoted_t" (id uuid);\n'
        )
        code, error = run({"0001.sql": sql})
        self.assertEqual(code, 1)
        for name in ("quoted_t", "unlogged_t", "temp_t", "temporary_t", "schema_t", "both_quoted_t"):
            self.assertIn(f"The table {name} (", error)
        self.assertNotIn("The table public ", error)

    def test_fails_for_a_create_table_statement_that_it_cannot_read(self) -> None:
        code, error = run({"0001.sql": "CREATE TABLE app_user (id uuid);\nCREATE TABLE $weird$ (id uuid);"})
        self.assertEqual(code, 1)
        self.assertIn("0001.sql", error)
        self.assertIn("cannot read", error)

    def test_fails_for_an_allowed_table_that_no_migration_creates(self) -> None:
        sql = "CREATE TABLE app_user (id uuid);\nCREATE TABLE membership (id uuid);"
        code, error = run({"0001.sql": sql}, allowed={"gone_table": "A reason."})
        self.assertEqual(code, 1)
        self.assertIn("gone_table", error)
        self.assertIn("no migration creates", error)

    def test_fails_for_a_column_that_an_alter_table_adds_to_an_allowed_table(self) -> None:
        migrations = {
            "0001.sql": "CREATE TABLE job (id uuid);",
            "0002.sql": "ALTER TABLE job ADD COLUMN requester_email text, ADD note text;",
        }
        code, error = run(migrations, allowed={"job": "A reason."})
        self.assertEqual(code, 1)
        self.assertIn("job.requester_email (0002.sql)", error)
        self.assertIn("job.note (0002.sql)", error)

    def test_an_allowed_column_with_a_reason_passes(self) -> None:
        migrations = {
            "0001.sql": "CREATE TABLE job (id uuid);",
            "0002.sql": "ALTER TABLE ONLY public.job\n    ADD COLUMN IF NOT EXISTS attempts integer DEFAULT 0;",
        }
        code, error = run(migrations, allowed={"job": "A reason."}, allowed_columns={"job.attempts": "A reason."})
        self.assertEqual(code, 0, error)

    def test_a_constraint_that_an_alter_table_adds_is_not_a_column(self) -> None:
        migrations = {
            "0001.sql": "CREATE TABLE job (id uuid, other uuid);",
            "0002.sql": (
                "ALTER TABLE job ADD CONSTRAINT job_id CHECK (id IS NOT NULL), ADD FOREIGN KEY (other) "
                "REFERENCES job (id), ADD PRIMARY KEY (id), ADD UNIQUE (other), ADD CHECK (true);\n"
                "ALTER TABLE app_user ADD COLUMN email text;"
            ),
        }
        code, error = run(migrations | {"0000.sql": "CREATE TABLE app_user (id uuid);"}, allowed={"job": "A reason."})
        self.assertEqual(code, 0, error)

    def test_fails_for_an_allowed_column_that_no_migration_adds(self) -> None:
        code, error = run(
            {"0001.sql": "CREATE TABLE job (id uuid);"},
            allowed={"job": "A reason."},
            allowed_columns={"job.gone": "A reason."},
        )
        self.assertEqual(code, 1)
        self.assertIn("job.gone", error)
        self.assertIn("no migration adds", error)

    def test_fails_for_a_folder_without_tables(self) -> None:
        code, error = run({"0001.sql": "SELECT 1;"})
        self.assertEqual(code, 1)
        self.assertIn("no CREATE TABLE", error)

    def test_every_allowed_table_has_a_reason(self) -> None:
        for table, reason in check.ALLOWED.items():
            self.assertTrue(reason.endswith("."), table)
        for table in ("local_id_counter", "job", "worker_heartbeat", "telegram_update"):
            self.assertIn(table, check.ALLOWED)
        for column, reason in check.ALLOWED_COLUMNS.items():
            self.assertTrue(reason.endswith("."), column)
            self.assertIn(column.split(".")[0], check.ALLOWED)

    def test_the_inventory_of_the_repository_covers_the_migrations(self) -> None:
        self.assertEqual(check.main([]), 0)


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1])
