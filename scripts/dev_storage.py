#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Prepare the local Garage node: layout, bucket and access key (ADR 0009).

Each step checks the current state first, so the script can run again.
It uses the Garage command line in the container, because the admin API is not published.

Usage: dev_storage.py
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SECRETS_DIR = ROOT / ".dev" / "secrets"
BUCKET = "tada"
KEY_NAME = "tada-dev"


def garage(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    command = ["docker", "compose", "exec", "-T", "garage", "/garage", *args]
    return subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=check)  # noqa: S603


def ensure_layout() -> None:
    if "NO ROLE ASSIGNED" not in garage("status").stdout:
        return
    node_id = garage("node", "id", "--quiet").stdout.strip().split("@")[0]
    garage("layout", "assign", "--zone", "dev", "--capacity", "1G", node_id)
    garage("layout", "apply", "--version", "1")
    print("assigned the storage layout")


def ensure_key() -> str:
    key_id = (SECRETS_DIR / "s3_access_key_id").read_text().strip()
    if garage("key", "info", key_id, check=False).returncode != 0:
        secret = (SECRETS_DIR / "s3_secret_access_key").read_text().strip()
        garage("key", "import", "--yes", "-n", KEY_NAME, key_id, secret)
        print(f"imported the access key {KEY_NAME}")
    return key_id


def ensure_bucket(key_id: str) -> None:
    if garage("bucket", "info", BUCKET, check=False).returncode != 0:
        garage("bucket", "create", BUCKET)
        print(f"created the bucket {BUCKET}")
    garage("bucket", "allow", "--read", "--write", "--owner", BUCKET, "--key", key_id)


def main() -> int:
    try:
        ensure_layout()
        ensure_bucket(ensure_key())
    except subprocess.CalledProcessError as error:
        # Name only the subcommand: the arguments can contain a secret.
        print(f"garage {' '.join(error.cmd[6:8])} failed:\n{error.stderr}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
