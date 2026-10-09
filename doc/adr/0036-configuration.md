# 0036. Configuration, secrets and the first owner

- Status: Accepted
- Date: 2026-10-06
- Amended by: [0070](0070-ai-security-review-replaces-the-external-review.md)

## Context

The platform contract (ADR 0025) gives settings through `TADA_*` variables and secrets through files named by `TADA_<NAME>_FILE`.
A process must stop at startup if a setting that it needs is missing or invalid.
We need two things only: parse environment variables into types, and read secret files.
A new installation has no user, so somebody must create the first owner without a sign-up page.

## Decision

Settings:

- Each process role and command has its own typed settings section. The `tada` binary loads only the sections that the started command needs.
- A small loader of our own reads the variables and the secret files and parses each value into its type.
- The loader collects all errors of the needed sections and reports them together. The process then stops with exit code 2.
- Settings that are not secret can have defaults. Secrets never have defaults.
- The database connection is `TADA_DATABASE_URL` without a password, plus `TADA_DATABASE_PASSWORD_FILE`. ADR 0025 follows this rule.
- `tada settings` prints the reference of all settings: name, type, default, secret or not, and the commands that use it.
  `doc/settings.md` is generated from this output. CI fails if the file differs.

Secrets:

- A secret value is a `SecretString` from the `secrecy` crate. Its `Debug` output is redacted, and the crate clears its own buffer on drop.
- Copies that a library makes after `expose_secret()` are not cleared. Only the adapter that needs a secret calls `expose_secret()`, and as late as possible.
- If a secret file is readable by all users, the loader writes a warning. It does not stop, because some runtimes mount secrets in this mode.

Local development:

- `scripts/dev_secrets.py` generates random development secrets into `.dev/secrets/`. Git ignores this folder.
- No secret value, also no fake value, is committed.

Runtime switches:

- Feature flags for each organization are rows in the database, not settings. An owner changes them through the API, and the audit log records each change.

First owner:

1. An operator runs `tada bootstrap --organization-slug <slug> --organization-name <name> --owner-email <address>`.
2. If no organization has this slug, the command creates it.
3. If the organization has no owner, the command revokes any pending owner invitation and creates a new one (ADR 0008).
4. The command queues the invitation email as an outbound job (ADR 0007). The worker sends it.
5. With `--print-link`, the command also writes the link to standard error, but only if standard error is a terminal. Otherwise it refuses. The printed link expires after 30 minutes.
6. If the organization has an owner, the command changes nothing and exits with code 0.

## Consequences

- A wrong setting stops the process at once, with all errors in one message.
- The `worker` does not fail because of a missing `serve` setting.
- The settings reference cannot drift from the code.
- Secret delivery is an operator task in the deployment repository; the app only reads files.

## Alternatives

- The `config` crate: layered sources and file formats that we do not need.
- `envy` and `figment`: they parse variables, but the secret files and the per-command sections still need our own code.
- A setup page in the web client for the first owner: an unauthenticated page that an attacker can reach first.
- A strict permission check on secret files: operators would switch it off on runtimes that mount secrets readable by all, so it would protect nothing.
