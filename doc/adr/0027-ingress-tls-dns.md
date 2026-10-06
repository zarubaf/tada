# 0027. Ingress, TLS and DNS

- Status: Proposed
- Date: 2026-10-06

## Context

The domain `zaruba.email` uses Cloudflare DNS.
Telegram webhooks and magic links need a public HTTPS address.
The rate limits in ADR 0008 need the real client IP address.
tada handles personal data. A party that decrypts the TLS traffic can read all of it.
Cloudflare's free plan limits a proxied request body to 100 MB.

## Decision

- Hostnames (proposed; the product owner confirms):
  - Production: `tada.zaruba.email`.
  - Staging: `staging.tada.zaruba.email`.
- Cloudflare holds the DNS records in "DNS only" mode. Cloudflare does not proxy the traffic.
- Caddy is the TLS endpoint on the VM, with automatic ACME certificates.
- Caddy routes the web client, the API and the Telegram webhook path to the `serve` and `telegram` roles.
- Caddy is the only container with published ports: 80 and 443.
- The app trusts `X-Forwarded-For` only from the Caddy network range (`TADA_TRUSTED_PROXIES`, ADR 0025).
- Caddy sets the security headers that apply to all responses, for example `Strict-Transport-Security`.

## Consequences

- No third party can read the traffic between the users and tada.
- The VM must accept inbound traffic on ports 80 and 443.
- The public IP address of the VM is visible in DNS.
- On Kubernetes, an Ingress controller and cert-manager take the role of Caddy. The app does not change.

## Alternatives

- Cloudflare proxy with an origin certificate: DDoS protection, but Cloudflare reads all personal data, and uploads above 100 MB fail.
- Cloudflare Tunnel: no open inbound ports, but the same TLS termination at Cloudflare, plus one more daemon.
  It stays the fallback if the VM must not accept inbound traffic.
