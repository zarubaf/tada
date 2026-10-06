# 0030. Host provisioning as code

- Status: Proposed
- Date: 2026-10-06

## Context

We must be able to set up a fresh VM the same way each time, for example for the dedicated production VM (ADR 0026).
The existing VPS has an inactive firewall, and it publishes ports that tada does not need, for example 631 (CUPS).
Docker writes its own firewall rules. A port that a container publishes bypasses `ufw`.

## Decision

- One idempotent shell script, `deploy/provision.sh`, sets up a host. `mise run host:provision` runs it over SSH. A second run changes nothing.
- The script does these steps:
  1. Install Docker Engine and the Compose plugin from the Docker repository.
  2. Create a `deploy` user for the deploy scripts. Its SSH key has a forced command (ADR 0032).
  3. Set SSH to key-only authentication and turn off root login.
  4. Turn on `unattended-upgrades` for security updates.
  5. Set `ufw` to deny all inbound traffic except 22, 80 and 443, with `ufw limit` on 22.
  6. Create `/etc/tada/<environment>/` with the correct owners and modes.
- Only Caddy publishes ports. All other containers use internal Compose networks, so Docker cannot open other ports.
- The product owner also sets a Hetzner Cloud Firewall with the same rules, outside the VM.
- A fresh Hetzner VM can run the script through cloud-init on its first start.

## Consequences

- A new VM is ready in minutes, and the script documents its state.
- On the existing VPS, the script must not break the other workloads. A dry-run mode shows each change before it runs.
- On Kubernetes or Cloud Run, the provider manages the hosts, and this script is not used.

## Alternatives

- Ansible: a mature tool, but a Python tool chain and an inventory format for one host.
- Manual setup: not reproducible, and nobody knows the state of the host.
- `fail2ban`: with key-only SSH and `ufw limit`, it adds little protection and one more service.
