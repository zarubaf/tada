# 0015. Single-host deployment with Docker Compose

- Status: Accepted
- Date: 2026-10-06

## Context

The first club deployment must cost less than CHF 100 per month.
Telegram webhooks and magic links need a public HTTPS address.
The data of a Swiss club should stay in Switzerland or the EU.

## Decision

- The first deployment is one virtual machine at a Swiss or EU provider.
- Docker Compose runs these services: server, worker, PostgreSQL, Garage and Caddy.
- Caddy terminates TLS with automatic certificates.
- CI builds one container image for the server and the worker.
- Backups contain a database dump and the object storage, with a manifest. They go to a second provider.
- We test a restore before the first real event data enters the system.
- Development, staging and production are separate databases and buckets.

## Consequences

- One machine is a single point of failure. This is acceptable for planning, not for live event operations.
- A named person must own updates, backups and restores. This is an open question in the PoC.
- The provider and the price need a decision in the operations phase.

## Alternatives

- A home server: no stable public address and a higher risk for data.
- A managed platform (PaaS): simpler operations, but the database cost alone can exceed the budget.
- Kubernetes: operations work far above the need.
