#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the built web client shows the product version or the commit (ADR 0078).

The files of the web client are public: a client that is not signed in can load each of them.
They must hold only the opaque build ID, never the version or the commit (ADR 0075).
The commit is the one that the build uses: `TADA_BUILD_COMMIT`, or else the Git commit of HEAD.

Usage: check_bundle_identity.py
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEB = ROOT / "apps" / "web"
DIST = WEB / "dist"
TEXT_SUFFIXES = {".js", ".css", ".html", ".json", ".map", ".svg", ".txt", ".webmanifest"}


def problems(version: str, commit: str | None, files: dict[str, str]) -> list[str]:
    """The files that show the version or the commit. An empty list means that the bundle is clean."""
    # A version or a commit inside a longer number or hex string is a coincidence, not a disclosure.
    # A `v` before the version, as in `v0.3.0`, still shows it.
    patterns = {"the version": re.compile(rf"(?<![\w.])v?{re.escape(version)}(?![\w.])")}
    if commit:
        patterns["the commit"] = re.compile(rf"(?<![0-9A-Za-z]){re.escape(commit)}(?![0-9A-Za-z])")
    return [
        f"{name} shows {what}"
        for name, text in sorted(files.items())
        for what, pattern in patterns.items()
        if pattern.search(text)
    ]


def build_commit() -> str | None:
    """The short commit that `vite.config.ts` uses, or `None` without one."""
    given = os.environ.get("TADA_BUILD_COMMIT", "").strip()
    if given:
        return given[:7]
    try:
        result = subprocess.run(  # noqa: S603
            ["git", "rev-parse", "--short=7", "HEAD"],  # noqa: S607
            capture_output=True,
            text=True,
            check=True,
            cwd=WEB,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip() or None


def main() -> int:
    if not (DIST / "index.html").is_file():
        print(f"{DIST / 'index.html'} does not exist; build the web client first.", file=sys.stderr)
        return 1
    version = json.loads((WEB / "package.json").read_text())["version"]
    files = {
        str(path.relative_to(DIST)): path.read_text(errors="replace")
        for path in DIST.rglob("*")
        if path.is_file() and path.suffix in TEXT_SUFFIXES
    }
    found = problems(version, build_commit(), files)
    for problem in found:
        print(f"{problem} (ADR 0078)", file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
