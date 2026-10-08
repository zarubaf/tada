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


def run(migrations: dict[str, str], inventory: str = INVENTORY) -> tuple[int, str]:
    """Runs the check on temporary files. Returns the exit code and the text of standard error."""
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        (root / "migrations").mkdir()
        for name, sql in migrations.items():
            (root / "migrations" / name).write_text(sql)
        (root / "inventory.md").write_text(inventory)
        error = io.StringIO()
        with contextlib.redirect_stderr(error), contextlib.redirect_stdout(io.StringIO()):
            code = check.main([str(root / "migrations"), str(root / "inventory.md")])
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
        code, _ = run({"0001.sql": "CREATE TABLE job (id uuid);\nCREATE TABLE telegram_update (update_id bigint);"})
        self.assertEqual(code, 0)

    def test_ignores_a_table_in_a_comment_and_reads_other_spellings(self) -> None:
        sql = "-- CREATE TABLE commented_out (id uuid);\ncreate table if not exists app_user (id uuid);"
        code, error = run({"0001.sql": sql})
        self.assertEqual(code, 0, error)

    def test_fails_for_a_folder_without_tables(self) -> None:
        code, error = run({"0001.sql": "SELECT 1;"})
        self.assertEqual(code, 1)
        self.assertIn("no CREATE TABLE", error)

    def test_every_allowed_table_has_a_reason(self) -> None:
        for table, reason in check.ALLOWED.items():
            self.assertTrue(reason.endswith("."), table)
        for table in ("local_id_counter", "job", "worker_heartbeat", "telegram_update"):
            self.assertIn(table, check.ALLOWED)

    def test_the_inventory_of_the_repository_covers_the_migrations(self) -> None:
        self.assertEqual(check.main([]), 0)


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1])
