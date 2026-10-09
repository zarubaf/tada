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
Every `pub trait` is a port unless NON_PORTS names it with a reason, so a new trait fails until someone classifies it.
A trait method passes if one of its parameters carries the scope (`OrgScope` or a caller type
in SCOPED_TYPES), or if its doc comment has the marker. Otherwise the script prints file:line.

A method signature that the script cannot read fails the check with "cannot parse".

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
# A port is a trait that a store or adapter implements to read or write data.
# Every other `pub trait` in the app is listed here with its reason; all others are checked.
NON_PORTS = {
    "Authenticator": "turns a credential into a caller; it delegates the reads to the stores",
    "Clock": "reads the time, no data",
    "CommandError": "maps a command error to a problem code, no data access",
    "DependencyCheck": "probes a dependency for the health endpoint, no data access",
    "ExportSink": "writes the files of an export; it reads no data, the export source does",
    "JobHandler": "runs one job; it reaches data only through the ports",
    "MailTexts": "renders the mail texts, no data access",
    "Mailer": "sends a mail, no data access",
    "MayPropose": "a marker on a caller type, no data access",
    "Principal": "describes a caller type, no data access",
    "ServiceIdentity": "describes a service caller type, no data access",
    "WorkRecord": "reads the owner and the workstream of a work record view, no data access",
}
TRAIT = re.compile(r"\bpub(?:\([^)]*\))?\s+trait\s+(\w+)[^{;]*\{")
FUNCTION = re.compile(r"\bfn\b")
FUNCTION_HEAD = re.compile(r"\s+(\w+)\s*")


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


def skip_generics(text: str, start: int) -> int | None:
    """The index after the `<...>` group at `start`, with nested angle brackets; None if it is unbalanced."""
    depth, end = 0, start
    while end < len(text):
        char = text[end]
        if char == "-" and text[end + 1 : end + 2] == ">":
            end += 2  # the arrow in a bound such as `F: Fn() -> T`
            continue
        depth += {"<": 1, ">": -1}.get(char, 0)
        end += 1
        if depth == 0:
            return end
    return None


def function_head(text: str, start: int) -> tuple[str, int] | None:
    """The name of the function after the `fn` at `start` and the index of its `(`; None if unreadable."""
    head = FUNCTION_HEAD.match(text, start)
    if not head:
        return None
    end = head.end()
    if text[end : end + 1] == "<":
        end = skip_generics(text, end)
        if end is None:
            return None
        end += len(re.match(r"\s*", text[end:]).group())
    return (head.group(1), end) if text[end : end + 1] == "(" else None


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
    """The line and name of each trait method without a scope parameter and without the marker.

    A method that the script cannot read has the name "cannot parse"; it is never skipped.
    """
    found = []
    for trait in TRAIT.finditer(text):
        if trait.group(1) in NON_PORTS:
            continue
        body_end = matching_brace(text, trait.end())
        body = text[trait.end() : body_end]
        for keyword in FUNCTION.finditer(body):
            offset = trait.end() + keyword.start()
            line = text.count("\n", 0, offset) + 1
            head = function_head(text, trait.end() + keyword.end())
            if head is None:
                found.append((line, "cannot parse this method signature"))
                continue
            name, open_paren = head
            params = parameters(text, open_paren)
            if any(re.search(rf"\b{scope}\b", params) for scope in SCOPED_TYPES):
                continue
            if any(MARKER_LINE.match(doc) for doc in doc_comment(text, offset).splitlines()):
                continue
            found.append((line, name))
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
