# Architecture decision records

An architecture decision record (ADR) contains one decision, its context and its consequences.
The ADRs are the authority for the decisions in this project.
[ARCHITECTURE.md](../ARCHITECTURE.md) describes the result; the ADRs give the reasons.

## Status values

- **Proposed:** The decision is a recommendation. The product owner did not accept it yet.
- **Accepted:** The project follows the decision.
- **Amended by NNNN:** A newer ADR changes a part of the decision. The other parts stay in force.
- **Superseded by NNNN:** A newer ADR replaces the decision. Do not delete the old ADR.
- **Moved by NNNN:** The decision now lives in another repository. The Git history keeps the text.

## Index

| ADR                                                   | Title                                                    | Status                    |
| ----------------------------------------------------- | -------------------------------------------------------- | ------------------------- |
| [0001](0001-record-architecture-decisions.md)         | Record architecture decisions                            | Accepted                  |
| [0002](0002-monorepo-and-services.md)                 | Monorepo with one core and enforced boundaries           | Accepted                  |
| [0003](0003-runtime-and-tooling.md)                   | Rust for the backend, TypeScript only for the web client | Accepted                  |
| [0004](0004-http-api-contract.md)                     | HTTP API contract and stability                          | Superseded by 0017        |
| [0005](0005-web-client.md)                            | React and Vite web client with German UI                 | Accepted                  |
| [0006](0006-persistence.md)                           | PostgreSQL, sqlx and reviewed SQL migrations             | Accepted                  |
| [0007](0007-jobs-and-schedules.md)                    | Durable jobs and schedules in PostgreSQL                 | Accepted                  |
| [0008](0008-authentication.md)                        | Authentication separate from authorization               | Accepted                  |
| [0009](0009-object-storage.md)                        | Object storage through the S3 API                        | Accepted                  |
| [0010](0010-model-provider.md)                        | One model provider port with limited AI authority        | Accepted                  |
| [0011](0011-telegram.md)                              | Telegram channel adapter                                 | Accepted                  |
| [0012](0012-documentation.md)                         | Documentation language, style and checks                 | Accepted                  |
| [0013](0013-git-workflow.md)                          | Git workflow and checks                                  | Accepted                  |
| [0014](0014-license.md)                               | Apache-2.0 license and public repository                 | Accepted                  |
| [0015](0015-deployment.md)                            | Single-host deployment with Docker Compose               | Moved by 0033             |
| [0016](0016-environments-and-releases.md)             | Environments and release promotion                       | Moved by 0033             |
| [0017](0017-api-contract-rust.md)                     | HTTP API contract from Rust types                        | Accepted                  |
| [0018](0018-design-system-foundation.md)              | Design system foundation and tokens                      | Accepted                  |
| [0019](0019-styling.md)                               | Styling with CSS custom properties and CSS Modules       | Accepted                  |
| [0020](0020-component-primitives.md)                  | React Aria Components as the accessible base             | Accepted                  |
| [0021](0021-typography-and-icons.md)                  | Self-hosted typography and one icon set                  | Accepted                  |
| [0022](0022-accessibility.md)                         | Accessibility standard: WCAG 2.2 AA                      | Accepted                  |
| [0023](0023-responsive-layout.md)                     | Responsive layout: one app, two primary contexts         | Accepted                  |
| [0024](0024-frontend-quality-gates.md)                | Frontend quality gates                                   | Accepted                  |
| [0025](0025-platform-contract.md)                     | Platform contract between the app and its runtime        | Accepted                  |
| [0026](0026-first-runtime.md)                         | Docker Compose as the first runtime, managed by scripts  | Moved by 0033             |
| [0027](0027-ingress-tls-dns.md)                       | Ingress, TLS and DNS                                     | Moved by 0033             |
| [0028](0028-images-and-registry.md)                   | Container images and registry                            | Accepted                  |
| [0029](0029-secrets-delivery.md)                      | Secrets delivery                                         | Moved by 0033             |
| [0030](0030-host-provisioning.md)                     | Host provisioning as code                                | Moved by 0033             |
| [0031](0031-backups.md)                               | Backups                                                  | Moved by 0033             |
| [0032](0032-delivery-flow.md)                         | Delivery flow and approval gates                         | Moved by 0033             |
| [0033](0033-deployment-outside-this-repository.md)    | Deployment outside this repository                       | Accepted                  |
| [0034](0034-python-for-scripts.md)                    | Python for all scripts                                   | Accepted                  |
| [0035](0035-observability.md)                         | Observability without direct identifiers                 | Accepted                  |
| [0036](0036-configuration.md)                         | Configuration, secrets and the first owner               | Accepted                  |
| [0037](0037-error-model.md)                           | Error model                                              | Accepted                  |
| [0038](0038-ids-and-time.md)                          | IDs and time                                             | Accepted                  |
| [0039](0039-actors-and-identities.md)                 | Callers, actors and service identities                   | Accepted                  |
| [0040](0040-ai-intake-through-mcp.md)                 | AI intake through MCP first                              | Accepted                  |
| [0041](0041-ci-build-and-dependencies.md)             | CI build and dependency policy                           | Accepted                  |
| [0042](0042-transactional-email.md)                   | Transactional email                                      | Accepted                  |
| [0043](0043-upload-policy.md)                         | Upload policy                                            | Accepted                  |
| [0044](0044-api-conventions.md)                       | API conventions                                          | Accepted                  |
| [0045](0045-data-protection.md)                       | Data protection under the revDSG                         | Accepted                  |
| [0046](0046-inbound-email.md)                         | Inbound email                                            | Accepted, amended by 0057 |
| [0047](0047-model-adapter.md)                         | Model adapter details                                    | Accepted                  |
| [0048](0048-ai-evaluation-set.md)                     | AI evaluation set                                        | Accepted                  |
| [0049](0049-entities-and-fact-model.md)               | Typed entities and a field catalog for facts             | Accepted                  |
| [0050](0050-proposals-and-review.md)                  | Proposals and review                                     | Accepted                  |
| [0051](0051-document-drafts-and-provenance.md)        | Document drafts and provenance                           | Accepted                  |
| [0052](0052-event-roles-and-ownership.md)             | Event roles and record ownership                         | Accepted                  |
| [0053](0053-development-authenticator.md)             | Development authenticator for the walking skeleton       | Superseded by 0056        |
| [0054](0054-job-queue-implementation.md)              | Job queue in store-pg                                    | Accepted                  |
| [0055](0055-office-format-detection.md)               | Detection of office formats in ZIP containers            | Accepted                  |
| [0056](0056-sign-in-details.md)                       | Sign-in: organization, invitations and rate limits       | Accepted                  |
| [0057](0057-modular-mail-and-inbound-webhook.md)      | Modular mail adapters and an inbound webhook             | Accepted                  |
| [0058](0058-markdown-renderer-for-drafts.md)          | Markdown parser and renderer for drafts                  | Proposed                  |
| [0059](0059-structured-export.md)                     | Structured export of one organization                    | Proposed                  |
| [0060](0060-unicode-normalization-of-source-texts.md) | Unicode normalization of source texts                    | Proposed                  |
| [0061](0061-audit-subject-and-role-detail.md)         | Audit subject and role detail                            | Proposed                  |
| [0062](0062-authenticators-in-app.md)                 | Authenticators in `app`                                  | Proposed                  |
| [0063](0063-ownership-after-removal.md)               | Ownership after removal                                  | Proposed                  |
| [0064](0064-one-proposal-input-for-http-and-mcp.md)   | One proposal input for HTTP and MCP                      | Proposed                  |

## New ADR

1. Copy [template.md](template.md) to the next free number.
2. Set the status to "Proposed".
3. Add a line to the index.
4. Open a pull request. The product owner accepts or rejects it.
