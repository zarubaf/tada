#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Fail if the built CSS of the web client does not start with the layer order (ADR 0019).

The first mention of a cascade layer fixes its rank, so the statement in
apps/web/src/styles/layers.css must be the first rule of the built CSS. The page must also carry
no inline <style>, so it does not depend on 'unsafe-inline' in the style policy.

Usage: check_css_layers.py
"""

import re
import sys
from pathlib import Path

WEB = Path(__file__).resolve().parent.parent / "apps" / "web"
STATEMENT = re.compile(r"@layer\s+([\w\s,-]+?)\s*;")
STYLESHEET = re.compile(r'<link[^>]*rel="stylesheet"[^>]*\shref="/([^"]+\.css)"')
COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)


def layer_order(css: str) -> list[str] | None:
    """The layers of the first rule of `css`, or None if the first rule is not a layer statement."""
    match = STATEMENT.match(COMMENT.sub("", css).lstrip())
    return [name.strip() for name in match.group(1).split(",")] if match else None


def problems(source: str, index: str, built: dict[str, str]) -> list[str]:
    """What is wrong with the built page; `built` maps each style sheet path to its text."""
    expected = layer_order(source)
    if expected is None:
        return ["layers.css does not start with a layer statement."]
    found = []
    if "<style" in index:
        found.append("index.html has an inline <style>; the layer order belongs in layers.css.")
    sheets = STYLESHEET.findall(index)
    if not sheets:
        return [*found, "index.html loads no style sheet; the pattern of this script is out of date."]
    first = layer_order(built.get(sheets[0], ""))
    if first != expected:
        found.append(f"{sheets[0]} starts with {first or 'no layer statement'}, expected {expected}.")
    return found


def main() -> int:
    dist = WEB / "dist"
    index = dist / "index.html"
    if not index.is_file():
        print(f"{index} does not exist; build the web client first.", file=sys.stderr)
        return 1
    text = index.read_text()
    built = {name: (dist / name).read_text() for name in STYLESHEET.findall(text)}
    found = problems((WEB / "src" / "styles" / "layers.css").read_text(), text, built)
    for line in found:
        print(line, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
