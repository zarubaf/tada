#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the Rust sources contain the development authenticator (ADR 0056).

The session authenticator is the only way in. A release build must not contain a way around it.
The script searches the Rust sources for the names of the removed development code.

Usage: check_no_dev_auth.py
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = ("DevAuthenticator", "dev_authenticator", "ensure_dev_organization")


def main() -> int:
    found = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        for number, line in enumerate(path.read_text().splitlines(), start=1):
            found.extend(f"{path.relative_to(ROOT)}:{number}: {name}" for name in FORBIDDEN if name in line)
    if found:
        print("The development authenticator is back (ADR 0056):", file=sys.stderr)
        print("\n".join(found), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
