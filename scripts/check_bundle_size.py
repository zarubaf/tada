#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the initial JavaScript of the web client is above its budget (ADR 0024).

The initial JavaScript is each script and module preload that dist/index.html loads.
The script measures the gzip size, as the browser receives it.

Usage: check_bundle_size.py
"""

import gzip
import re
import sys
from pathlib import Path

DIST = Path(__file__).resolve().parent.parent / "apps" / "web" / "dist"
BUDGET_BYTES = 200_000
INITIAL = re.compile(r'<(?:script[^>]*\ssrc|link[^>]*rel="modulepreload"[^>]*\shref)="/([^"]+\.js)"')


def main() -> int:
    index = DIST / "index.html"
    if not index.is_file():
        print(f"{index} does not exist; build the web client first.", file=sys.stderr)
        return 1
    files = INITIAL.findall(index.read_text())
    if not files:
        print("index.html loads no JavaScript; the pattern of this script is out of date.", file=sys.stderr)
        return 1
    total = sum(len(gzip.compress((DIST / name).read_bytes())) for name in files)
    print(f"initial JavaScript: {total / 1000:.1f} kB gzip, budget {BUDGET_BYTES / 1000:.0f} kB")
    return 0 if total <= BUDGET_BYTES else 1


if __name__ == "__main__":
    sys.exit(main())
