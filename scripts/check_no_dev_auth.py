#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the Rust sources contain a way around the real authenticators (ADR 0056, ADR 0062).

Two authenticators are the only ways in: sessions (ADR 0008) and personal API tokens (ADR 0039).
The token authenticator gives an `AiCaller` only, never a `MemberCaller` to a handler.
The script is a tripwire against an accidental return of development code, not a parser that
resists a determined bypass. It checks two kinds of rules:

- The names of the removed development code must not return.
- The structure must stay closed: only the files in CREATORS create a `MemberCaller` with
  `MemberCaller::create(`, and only the types in ALLOWED_AUTHENTICATORS implement `Authenticator`
  outside tests. The bodies of `#[cfg(test)] mod name { ... }` blocks and the integration test
  directories `crates/*/tests/` count as tests.

Out of scope: a trait alias (`use ...Authenticator as A; impl A for X`), and braces inside
strings or comments.

Usage: check_no_dev_auth.py [ROOT]
"""

import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = ("DevAuthenticator", "dev_authenticator", "ensure_dev_organization")
CREATORS = ("crates/app/src/session.rs", "crates/app/src/tokens/mod.rs")
ALLOWED_AUTHENTICATORS = ("SessionAuthenticator", "TokenAuthenticator")
# The header of an `impl` block, over several lines. Group 1 is the type after `for`.
IMPL_AUTHENTICATOR = re.compile(r"\bimpl\b[^{;]*?\bAuthenticator\s+for\s+(.*?)\s*(?:\bwhere\b[^{]*)?\{", re.S)
TEST_MODULE = re.compile(r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?mod\s+\w+\s*\{")


def without_test_modules(text: str) -> str:
    """The text without the bodies of `#[cfg(test)] mod` blocks; the lines keep their numbers."""
    while match := TEST_MODULE.search(text):
        depth, end = 1, match.end()
        while depth and end < len(text):
            depth += {"{": 1, "}": -1}.get(text[end], 0)
            end += 1
        blanked = re.sub(r"[^\n]", "", text[match.start() : end])
        text = text[: match.start()] + blanked + text[end:]
    return text


def is_integration_test(relative: Path) -> bool:
    return len(relative.parts) > 3 and relative.parts[0] == "crates" and relative.parts[2] == "tests"


def implemented_type(target: str) -> str:
    """The type name without references, lifetimes and whitespace."""
    return re.sub(r"'\w+|&|\bmut\b|\s+", "", target)


def main(root: Path) -> int:
    found = []
    for path in sorted((root / "crates").rglob("*.rs")):
        relative = path.relative_to(root)
        text = without_test_modules(path.read_text())
        for number, line in enumerate(text.splitlines(), start=1):
            found.extend(f"{relative}:{number}: {name}" for name in FORBIDDEN if name in line)
            creator = relative.as_posix() in CREATORS or is_integration_test(relative)
            if "MemberCaller::create(" in line and not creator:
                found.append(f"{relative}:{number}: MemberCaller::create outside {', '.join(CREATORS)}")
        if is_integration_test(relative):
            continue
        for match in IMPL_AUTHENTICATOR.finditer(text):
            target = implemented_type(match.group(1))
            if target not in ALLOWED_AUTHENTICATORS:
                number = text.count("\n", 0, match.start()) + 1
                found.append(f"{relative}:{number}: Authenticator for {target}")
    if found:
        print("A way around the real authenticators (ADR 0056, ADR 0062):", file=sys.stderr)
        print("\n".join(found), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT))
