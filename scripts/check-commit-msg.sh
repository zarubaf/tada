#!/usr/bin/env sh
# Check that the commit subject follows Conventional Commits.
# See doc/contributing.md#commits.
set -eu

subject=$(head -n 1 "$1")
pattern='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9-]+\))?!?: [^ ].{0,70}$'

case "$subject" in
  Merge\ *|Revert\ *|fixup!\ *|squash!\ *) exit 0 ;;
esac

if ! printf '%s\n' "$subject" | grep -Eq "$pattern"; then
  echo "Commit subject does not follow Conventional Commits:" >&2
  echo "  $subject" >&2
  echo "Expected: <type>(<scope>)?: <summary>, at most 72 characters." >&2
  echo "Types: feat fix docs style refactor perf test build ci chore revert" >&2
  exit 1
fi
