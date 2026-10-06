#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run the advisory checks and open an issue for a finding (ADR 0041).

The script runs `cargo deny check advisories` and `pnpm audit`.
If one fails, it opens an issue with the label "advisory", or comments on the open one.
GH_TOKEN must allow `issues: write`.

Usage: report_advisories.py
"""

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LABEL = "advisory"
CHECKS = {
    "cargo deny check advisories": (["cargo", "deny", "--workspace", "check", "advisories"], ROOT),
    "pnpm audit": (["pnpm", "audit", "--prod"], ROOT / "apps" / "web"),
}


def run_checks() -> list[str]:
    findings = []
    for name, (command, cwd) in CHECKS.items():
        result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)  # noqa: S603
        if result.returncode != 0:
            output = (result.stdout + result.stderr)[-6000:]
            findings.append(f"### `{name}`\n\n```text\n{output}\n```")
    return findings


def gh(*args: str) -> str:
    return subprocess.run(["gh", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout  # noqa: S603, S607


def main() -> int:
    findings = run_checks()
    if not findings:
        print("no advisories")
        return 0
    run_url = os.environ.get("RUN_URL", "a local run")
    body = f"The daily advisory check found problems in {run_url}.\n\n" + "\n\n".join(findings)
    open_issues = json.loads(gh("issue", "list", "--label", LABEL, "--state", "open", "--json", "number"))
    if open_issues:
        gh("issue", "comment", str(open_issues[0]["number"]), "--body", body)
    else:
        gh("issue", "create", "--title", "Security advisories in the dependencies", "--label", LABEL, "--body", body)
    print(body)
    return 1


if __name__ == "__main__":
    sys.exit(main())
