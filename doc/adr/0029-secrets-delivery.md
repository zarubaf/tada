# 0029. Secrets delivery

- Status: Accepted
- Date: 2026-10-06

## Context

ADR 0016 decided that secrets come from Compose `secrets:` files and that `deploy/SECRETS.md` lists them.
The repository is public, so no secret value can be in it, also not in encrypted form, because a leaked key exposes the full history.
An agent runs most operations (ADR 0026). The agent must not see secret values.

## Decision

- The host stores each secret as one file in `/etc/tada/<environment>/secrets/`, owned by root, with mode `0600`.
- Compose mounts these files as secrets. The app reads them through `TADA_<NAME>_FILE` (ADR 0025).
- The product owner's password manager is the source of each value.
- The command `mise run secrets:set <environment> <name>` reads one value from standard input and writes it to the host over SSH. It never prints the value or writes it to a local file.
- Only the product owner runs `secrets:set`. An agent can run `mise run secrets:check <environment>`, which reports only the names that are missing or older than their rotation period.
- GitHub Actions holds only the SSH deploy key (ADR 0032), as an environment secret.
- `deploy/SECRETS.md` lists the name, purpose, owner and rotation period of each secret. It never contains a value.

## Consequences

- No secret value is in Git, in CI logs or in the agent's context.
- A new host needs each secret again from the password manager.
- On Kubernetes, a Secret object or Google Secret Manager mounts the same files. The app does not change.

## Alternatives

- `sops` and `age` with encrypted files in the repository: values stay in the public history forever, and a leaked key exposes all of them.
- Secrets in GitHub environment secrets that CI copies to the host: CI then holds all production secrets.
- Environment variables: rejected in ADR 0016.
