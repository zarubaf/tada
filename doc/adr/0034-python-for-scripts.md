# 0034. Python for all scripts

- Status: Accepted
- Date: 2026-10-06

## Context

The project needs small scripts for checks and operations.
Shell scripts are hard to test, have quoting traps and differ between systems.
The product owner made Python the only script language for this project.

## Decision

- All scripts are Python. Shell scripts are not permitted.
- Each script is a PEP 723 script with its dependencies in its header. `uv run` starts it.
- Scripts prefer the Python standard library.
- `ruff` lints and formats the scripts. `mise run check` runs it.
- Short inline commands in `mise.toml`, `lefthook.yml` and CI workflows are permitted. Any logic goes into a Python script.

## Consequences

- One language for all scripts, with a linter and tests.
- `uv` is a pinned tool in `mise.toml`.

## Alternatives

- Shell with `shellcheck`: still quoting traps and no data structures.
- Rust for scripts: compile step and more code for small tasks.
