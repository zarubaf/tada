# 0013. Git workflow and checks

- Status: Accepted
- Date: 2026-10-06

## Context

The history must explain why the code changed.
LLM agents make many commits; without rules the history becomes noise.

## Decision

- `main` has a linear history. Pull requests merge with rebase or squash.
- Commit subjects follow Conventional Commits. `scripts/check_commit_msg.py` checks them in a hook and in CI.
- A breaking change of a public contract has a `!` in the subject.
- Each commit is one logical change and passes `mise run check`.
- Formatting changes and content changes are separate commits.
- lefthook runs the checks on staged files before each commit.
- GitHub Actions runs `mise run check` on each push to `main` and on each pull request.
- Agents do not push or create remote branches without a request from the product owner.

## Consequences

- `git log --oneline` reads as a change log.
- A release note can come from the commit subjects.

## Alternatives

- Merge commits: a nonlinear history that is harder to read and bisect.
- The pre-commit framework: a second tool manager next to mise.
