#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run a command only when the change can affect the web client.

The change is the uncommitted work: staged and unstaged files against HEAD, and untracked files.
The command runs always when TADA_CHECK_ALL is set (CI and `mise run check:all`),
and when Git cannot tell what changed.
Otherwise the script prints one line and skips the command.

Usage: run_when_web_changed.py <label> <command> [<argument>...]
"""

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Files and folders that can change the result of the web and browser checks.
WEB_PATHS = (
    "apps/web/",
    "locales/",
    # The API contract that the web client consumes.
    "contracts/openapi.json",
    # The browser-check script holds the Playwright container pin.
    "scripts/check_browser.py",
    "scripts/check_bundle_size.py",
)


def git(*args: str) -> list[str]:
    result = subprocess.run(  # noqa: S603
        ["git", *args],  # noqa: S607
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.splitlines()


def changed_files() -> set[str]:
    files = set(git("diff", "--name-only", "HEAD"))
    files.update(git("ls-files", "--others", "--exclude-standard"))
    return files


def main(label: str, command: list[str]) -> int:
    if os.environ.get("TADA_CHECK_ALL"):
        return subprocess.run(command).returncode  # noqa: S603
    try:
        web_files = sorted(f for f in changed_files() if f.startswith(WEB_PATHS))
    except subprocess.CalledProcessError:
        return subprocess.run(command).returncode  # noqa: S603
    if web_files:
        return subprocess.run(command).returncode  # noqa: S603
    print(f"{label} skipped: no web file changed (`mise run check:all` runs it)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2:]))
