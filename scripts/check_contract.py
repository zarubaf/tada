#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail on a breaking change of the API contract (ADR 0017).

The script compares contracts/openapi.json with the same file at a base revision through oasdiff.
It ignores the changes in contracts/oasdiff-err-ignore.txt: new values of lists that the contract marks as open.
The environment variable CONTRACT_BASE names the base revision. The default is origin/main.
If the base has no contract yet, there is nothing to compare.

Usage: check_contract.py
"""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTRACT = "contracts/openapi.json"
IGNORE = "contracts/oasdiff-err-ignore.txt"


def main() -> int:
    base = os.environ.get("CONTRACT_BASE") or "origin/main"
    show = subprocess.run(  # noqa: S603
        ["git", "show", f"{base}:{CONTRACT}"],  # noqa: S607
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if show.returncode != 0:
        print(f"{base} has no {CONTRACT}; there is nothing to compare.")
        return 0
    with tempfile.NamedTemporaryFile("w", suffix=".json") as base_file:
        base_file.write(show.stdout)
        base_file.flush()
        # WARN also fails, for example on a new value in a response enum (ADR 0017).
        # IGNORE lists the new values of the lists that the contract marks as open.
        command = ["oasdiff", "breaking", "--fail-on", "WARN", "--err-ignore", IGNORE]
        command += [base_file.name, CONTRACT]
        return subprocess.run(command, cwd=ROOT).returncode  # noqa: S603


if __name__ == "__main__":
    sys.exit(main())
