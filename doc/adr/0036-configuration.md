# 0036. Configuration, secrets and the first owner

- Status: Proposed
- Date: 2026-10-06

## Context

The platform contract (ADR 0025) gives settings through `TADA_*` variables and secrets through files named by `TADA_<NAME>_FILE`.
The app must stop at startup if a setting is missing or invalid.
`envy` had no release since 2021 and `figment` none since 2024. `config` is active, but it merges many sources that we do not need.
A new installation has no user, so somebody must create the first owner without a sign-up page.

## Decision

Settings:

- One typed `Settings` struct in the `tada` binary holds all settings. Each role reads only the part it needs.
- A small loader of our own reads the variables and the secret files. It parses each value into its type.
- The loader collects all errors and reports them together. The process then stops with exit code 2.
- Settings that are not secret can have defaults. Secrets never have defaults.
- `tada settings` prints the reference of all settings: name, type, default, secret or not, and the role that uses it.
  `doc/settings.md` is generated from this output. CI fails if the file differs.

Secrets:

- A secret value is a `SecretString` from the `secrecy` crate. Its `Debug` output is redacted, and its memory is cleared on drop.
- Only the adapter that needs a secret calls `expose_secret()`.
- The loader rejects a secret file that other users can read, unless `TADA_ALLOW_INSECURE_SECRET_FILES` is set for local development.

Local development:

- `scripts/dev_secrets.py` generates random development secrets into `.dev/secrets/`. Git ignores this folder.
- No secret value, also no fake value, is committed.

Runtime switches:

- Feature flags for each organization are rows in the database, not settings. An owner changes them through the API, and the audit log records each change.

First owner:

1. An operator runs `tada bootstrap --organization <name> --owner-email <address>` once.
2. If the organization has no owner, the command creates the organization and an owner invitation (ADR 0008).
3. The command sends the invitation by email. With `--print-link`, it writes the link to the operator's terminal instead. It never writes the link to the log.
4. If an owner exists, the command changes nothing and exits with code 0. A second run is safe.

## Consequences

- A wrong setting stops the process at once, with all errors in one message.
- The settings reference cannot drift from the code.
- Setting a secret is an operator task in the deployment repository; the app only reads files.

## Alternatives

- The `config` crate: layered sources that we do not need, and secret handling that we must add anyway.
- `envy`: no release since 2021.
- A setup page in the web client for the first owner: an unauthenticated page that an attacker can reach first.
