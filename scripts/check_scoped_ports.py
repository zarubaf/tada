#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if a port method has no organization scope and no infrastructure marker (ADR 0039).

ADR 0039 says: infrastructure queries without an organization scope exist only in `store-pg`,
and each one is named in the code. The name is one doc line on the method of the port trait:

    /// Infrastructure query (ADR 0039): <reason>.

The script reads every port trait in `crates/app/src/**/*.rs` outside `#[cfg(test)] mod` bodies.
A port is a trait with a name that ends in Store, Queue or Links.
A trait method passes if one of its parameters carries the scope (`OrgScope` or a caller type
in SCOPED_TYPES), or if its doc comment has the marker. Otherwise the script prints file:line.

Out of scope: braces inside strings or comments, and traits that a macro generates.

Usage: check_scoped_ports.py [ROOT]
"""

import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parent.parent
# The parameter types that carry an organization scope.
SCOPED_TYPES = ("OrgScope", "MemberCaller")
MARKER = "/// Infrastructure query (ADR 0039): <reason>."
MARKER_LINE = re.compile(r"^///\s+Infrastructure query \(ADR 0039\): \S.*\.$")
TEST_MODULE = re.compile(r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?mod\s+\w+\s*\{")
# A port is a trait whose name ends in one of PORT_SUFFIXES: the repositories and queues of the app.
PORT_SUFFIXES = ("Store", "Queue", "Links")
TRAIT = re.compile(r"\btrait\s+(\w+)[^{;]*\{")
FUNCTION = re.compile(r"\bfn\s+(\w+)\s*(?:<[^>]*>)?\s*\(")


def matching_brace(text: str, start: int) -> int:
    """The index after the brace that closes the block whose body starts at `start`."""
    depth, end = 1, start
    while depth and end < len(text):
        depth += {"{": 1, "}": -1}.get(text[end], 0)
        end += 1
    return end


def without_test_modules(text: str) -> str:
    """The text without the bodies of `#[cfg(test)] mod` blocks; the lines keep their numbers."""
    while match := TEST_MODULE.search(text):
        end = matching_brace(text, match.end())
        blanked = re.sub(r"[^\n]", "", text[match.start() : end])
        text = text[: match.start()] + blanked + text[end:]
    return text


def parameters(text: str, open_paren: int) -> str:
    """The text between the parenthesis at `open_paren` and its closing parenthesis."""
    depth, end = 1, open_paren + 1
    while depth and end < len(text):
        depth += {"(": 1, ")": -1}.get(text[end], 0)
        end += 1
    return text[open_paren + 1 : end - 1]


def doc_comment(text: str, start: int) -> str:
    """The doc lines and attributes directly above the line that holds `start`."""
    lines = text[:start].splitlines()[:-1]
    block = []
    for line in reversed(lines):
        stripped = line.strip()
        if not stripped.startswith(("///", "#[")):
            break
        block.append(stripped)
    return "\n".join(reversed(block))


def unmarked_methods(text: str) -> list[tuple[int, str]]:
    """The line and name of each trait method without a scope parameter and without the marker."""
    found = []
    for trait in TRAIT.finditer(text):
        if not trait.group(1).endswith(PORT_SUFFIXES):
            continue
        body_end = matching_brace(text, trait.end())
        body = text[trait.end() : body_end]
        for function in FUNCTION.finditer(body):
            offset = trait.end() + function.start()
            params = parameters(text, trait.end() + function.end() - 1)
            if any(re.search(rf"\b{name}\b", params) for name in SCOPED_TYPES):
                continue
            if any(MARKER_LINE.match(line) for line in doc_comment(text, offset).splitlines()):
                continue
            found.append((text.count("\n", 0, offset) + 1, function.group(1)))
    return found


def main(root: Path) -> int:
    found = []
    for path in sorted((root / "crates/app/src").rglob("*.rs")):
        text = without_test_modules(path.read_text())
        found.extend(f"{path.relative_to(root)}:{line}: {name}" for line, name in unmarked_methods(text))
    if found:
        print(
            f'A port method without an organization scope needs the doc line "{MARKER}" (ADR 0039):',
            file=sys.stderr,
        )
        print("\n".join(found), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ROOT))
