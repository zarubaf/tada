#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run the Playwright checks of the web client in the pinned Playwright container (ADR 0024).

The screenshots come from this container only, so that they do not depend on the fonts of a laptop.
The container runs as the current user, so that it writes no files that belong to root.

Usage: check_browser.py [--update-snapshots]
"""

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# The version must match @playwright/test in apps/web/package.json.
IMAGE = (
    "mcr.microsoft.com/playwright:v1.63.0-noble@sha256:eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27"
)


def main(args: list[str]) -> int:
    command = [
        "docker", "run", "--rm", "--ipc=host",
        "--user", f"{os.getuid()}:{os.getgid()}",
        "--env", "HOME=/tmp",
        "--env", "CI=1",
        "--volume", f"{ROOT}:/work",
        "--workdir", "/work/apps/web",
        IMAGE,
        "node_modules/.bin/playwright", "test", *args,
    ]  # fmt: skip
    return subprocess.run(command).returncode  # noqa: S603


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
