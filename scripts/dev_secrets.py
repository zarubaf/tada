#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Generate random development secrets into .dev/secrets/ (ADR 0036).

The script never overwrites a secret, because the database and the storage volumes keep the first value.
To start again, stop the services, remove their volumes and the folder, and run the script again.

Usage: dev_secrets.py
"""

import secrets
import sys
from collections.abc import Callable
from pathlib import Path

SECRETS_DIR = Path(__file__).resolve().parent.parent / ".dev" / "secrets"

# Garage accepts imported keys only in its own format: "GK" and 24 hex digits, and a secret of 64 hex digits.
GENERATORS: dict[str, Callable[[], str]] = {
    "database_password": lambda: secrets.token_urlsafe(32),
    "garage_rpc_secret": lambda: secrets.token_hex(32),
    "garage_admin_token": lambda: secrets.token_urlsafe(32),
    "s3_access_key_id": lambda: "GK" + secrets.token_hex(12),
    "s3_secret_access_key": lambda: secrets.token_hex(32),
}


def main() -> int:
    # The folder is private. The files are readable by all users, because Docker mounts them with
    # the host mode, and the services in the containers do not run as the host user.
    SECRETS_DIR.mkdir(parents=True, exist_ok=True)
    SECRETS_DIR.chmod(0o700)
    for name, generate in GENERATORS.items():
        path = SECRETS_DIR / name
        if path.exists():
            continue
        path.write_text(generate())
        path.chmod(0o644)
        print(f"created {path.relative_to(SECRETS_DIR.parent.parent)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
