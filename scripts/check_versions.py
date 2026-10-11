#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check that the web client has the product version of the workspace (ADR 0075).

The `version` of `[workspace.package]` in `Cargo.toml` is the only source.
`apps/web/package.json` must have the same value, in the format MAJOR.MINOR.PATCH.

Usage: check_versions.py [ROOT]
"""

import json
import re
import sys
import tomllib
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)")


def problems(cargo: dict, package: dict) -> list[str]:
    """The reasons why the versions are wrong. An empty list means that they are right."""
    product = cargo.get("workspace", {}).get("package", {}).get("version")
    web = package.get("version")
    found = []
    if not isinstance(product, str) or not SEMVER.fullmatch(product):
        found.append(f"Cargo.toml: workspace version {product!r} is not MAJOR.MINOR.PATCH")
    if not isinstance(web, str):
        found.append("apps/web/package.json: no version")
    elif web != product:
        found.append(f"apps/web/package.json: version {web!r} differs from the product version {product!r}")
    return found


def main(root: Path) -> int:
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    package = json.loads((root / "apps/web/package.json").read_text())
    found = problems(cargo, package)
    for line in found:
        print(line, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT))
