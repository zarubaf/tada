# Architecture decision records

An architecture decision record (ADR) contains one decision, its context and its consequences.
The ADRs are the authority for the decisions in this project.
[ARCHITECTURE.md](../ARCHITECTURE.md) describes the result; the ADRs give the reasons.

## Status values

- **Proposed:** The decision is a recommendation. The product owner did not accept it yet.
- **Accepted:** The project follows the decision.
- **Superseded by NNNN:** A newer ADR replaces the decision. Do not delete the old ADR.

## Index

| ADR                                           | Title                                                    | Status             |
| --------------------------------------------- | -------------------------------------------------------- | ------------------ |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions                            | Accepted           |
| [0002](0002-monorepo-and-services.md)         | Monorepo with one core and enforced boundaries           | Accepted           |
| [0003](0003-runtime-and-tooling.md)           | Rust for the backend, TypeScript only for the web client | Accepted           |
| [0004](0004-http-api-contract.md)             | HTTP API contract and stability                          | Superseded by 0017 |
| [0005](0005-web-client.md)                    | React and Vite web client with German UI                 | Accepted           |
| [0006](0006-persistence.md)                   | PostgreSQL, sqlx and reviewed SQL migrations             | Accepted           |
| [0007](0007-jobs-and-schedules.md)            | Durable jobs and schedules in PostgreSQL                 | Accepted           |
| [0008](0008-authentication.md)                | Authentication separate from authorization               | Accepted           |
| [0009](0009-object-storage.md)                | Object storage through the S3 API                        | Accepted           |
| [0010](0010-model-provider.md)                | One model provider port with limited AI authority        | Accepted           |
| [0011](0011-telegram.md)                      | Telegram channel adapter                                 | Accepted           |
| [0012](0012-documentation.md)                 | Documentation language, style and checks                 | Accepted           |
| [0013](0013-git-workflow.md)                  | Git workflow and checks                                  | Accepted           |
| [0014](0014-license.md)                       | Apache-2.0 license and public repository                 | Accepted           |
| [0015](0015-deployment.md)                    | Single-host deployment with Docker Compose               | Accepted           |
| [0016](0016-environments-and-releases.md)     | Environments and release promotion                       | Accepted           |
| [0017](0017-api-contract-rust.md)             | HTTP API contract from Rust types                        | Accepted           |
| [0018](0018-design-system-foundation.md)      | Design system foundation and tokens                      | Accepted           |
| [0019](0019-styling.md)                       | Styling with CSS custom properties and CSS Modules       | Accepted           |
| [0020](0020-component-primitives.md)          | React Aria Components as the accessible base             | Accepted           |
| [0021](0021-typography-and-icons.md)          | Self-hosted typography and one icon set                  | Accepted           |
| [0022](0022-accessibility.md)                 | Accessibility standard: WCAG 2.2 AA                      | Accepted           |
| [0023](0023-responsive-layout.md)             | Responsive layout: one app, two primary contexts         | Accepted           |
| [0024](0024-frontend-quality-gates.md)        | Frontend quality gates                                   | Accepted           |

## New ADR

1. Copy [template.md](template.md) to the next free number.
2. Set the status to "Proposed".
3. Add a line to the index.
4. Open a pull request. The product owner accepts or rejects it.
