# 0032. Delivery flow and approval gates

- Status: Accepted
- Date: 2026-10-06

## Context

The product owner wants an agent to do as much as possible.
A production deploy and a destructive action need a human decision (ADR 0016).
The flow must also work without the agent, for example from a laptop or from CI alone.

## Decision

The same scripts serve all callers. Who may call what:

| Action                                   | Agent                     | GitHub Actions              | Product owner |
| ---------------------------------------- | ------------------------- | --------------------------- | ------------- |
| Build and push the image                 | no                        | yes, on each push to `main` | no            |
| Deploy to the shared VPS (invented data) | yes                       | yes                         | yes           |
| Staging rehearsal                        | yes                       | yes                         | yes           |
| Deploy to production                     | only through the workflow | yes, after approval         | approves      |
| Restore into production                  | no                        | no                          | yes           |
| Set or rotate a secret                   | no                        | no                          | yes           |
| Provision a host                         | dry run only              | no                          | yes           |

- The agent starts a production deploy with `gh workflow run deploy-production`. The `production` GitHub environment requires the product owner's approval before the job runs.
- The workflow connects with an SSH key whose `authorized_keys` entry has a forced command. That command accepts only `deploy <environment> <digest>` and `rehearse <digest>`.
- Each deploy records the environment, the image digest, the caller and the time in a deploy log on the host.
- A rollback is a deploy of the previous digest. A release that removes schema forms rolls back only by a restore (ADR 0016).

## Consequences

- The agent runs the full rehearsal on its own and reports the result. The product owner only approves.
- GitHub records each production deploy and its approval.
- A stolen deploy key can only deploy an image that is already in the registry. It cannot open a shell.

## Alternatives

- Deploys only from a laptop: no record and no approval gate.
- A pull-based agent on the host, for example Watchtower: it deploys each new tag without a rehearsal or an approval.
- Argo CD or Flux: the right tool on Kubernetes, but they need a cluster.
