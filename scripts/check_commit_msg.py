#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check that commit subjects follow Conventional Commits (doc/contributing.md#commits).

Usage: check_commit_msg.py <message-file>...
"""

import re
import sys
from pathlib import Path

TYPES = "feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert"
PATTERN = re.compile(rf"^({TYPES})(\([a-z0-9-]+\))?!?: \S.{{0,70}}$")
EXEMPT = ("Merge ", "Revert ", "fixup! ", "squash! ")


def check(subject: str) -> bool:
    return subject.startswith(EXEMPT) or bool(PATTERN.match(subject))


def main(paths: list[str]) -> int:
    failed = False
    for path in paths:
        lines = Path(path).read_text().splitlines()
        subject = lines[0] if lines else ""
        if not check(subject):
            failed = True
            print(f"Commit subject does not follow Conventional Commits:\n  {subject}", file=sys.stderr)
    if failed:
        print(
            f"Expected: <type>(<scope>)?: <summary>, at most 72 characters.\nTypes: {TYPES.replace('|', ' ')}",
            file=sys.stderr,
        )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
