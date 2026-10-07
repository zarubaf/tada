#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the Rust sources contain a way around the session authenticator (ADR 0056, ADR 0062).

The session authenticator is the only way in. The script checks two kinds of rules:

- The names of the removed development code must not return.
- The structure must stay closed: only `crates/app/src/session.rs` creates a `MemberCaller`
  with `MemberCaller::create(`, and `SessionAuthenticator` is the only `Authenticator`
  outside tests. Code after the first `#[cfg(test)]` line and files under `tests/` count as tests.

Usage: check_no_dev_auth.py [ROOT]
"""

import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = ("DevAuthenticator", "dev_authenticator", "ensure_dev_organization")
CREATOR = "crates/app/src/session.rs"
ALLOWED_AUTHENTICATOR = "SessionAuthenticator"
IMPL_AUTHENTICATOR = re.compile(r"\bimpl\b[^{;]*?\bAuthenticator\s+for\s+(\w+)")


def production_lines(path: Path) -> list[tuple[int, str]]:
    """The numbered lines of a file without its test module."""
    lines = path.read_text().splitlines()
    for index, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]":
            lines = lines[:index]
            break
    return list(enumerate(lines, start=1))


def main(root: Path) -> int:
    found = []
    for path in sorted((root / "crates").rglob("*.rs")):
        relative = path.relative_to(root)
        lines = production_lines(path)
        for number, line in lines:
            found.extend(f"{relative}:{number}: {name}" for name in FORBIDDEN if name in line)
        if "tests" in relative.parts:
            continue
        for number, line in lines:
            if "MemberCaller::create(" in line and relative.as_posix() != CREATOR:
                found.append(f"{relative}:{number}: MemberCaller::create outside {CREATOR}")
            match = IMPL_AUTHENTICATOR.search(line)
            if match and match.group(1) != ALLOWED_AUTHENTICATOR:
                found.append(f"{relative}:{number}: Authenticator for {match.group(1)}")
    if found:
        print("A way around the session authenticator (ADR 0056, ADR 0062):", file=sys.stderr)
        print("\n".join(found), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT))
