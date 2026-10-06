# 0014. Apache-2.0 license and public repository

- Status: Accepted
- Date: 2026-10-06

## Context

The product owner chose a public repository with a permissive license.
Commercial use stays possible.

## Decision

- tada uses the Apache License 2.0. The full text is in `LICENSE`.
- We prefer Apache-2.0 to MIT because it has an explicit patent grant.
- Each dependency must have a license that is compatible with Apache-2.0 for distribution.
- Services that run as separate processes, for example Garage (AGPL-3.0), are not distributed with tada and do not affect its license.
- The repository contains no real personal data, club contacts, credentials or unpublished agreements.

## Consequences

- Anyone can use, change and sell tada, also in a closed product.
- Fixtures use invented data or confirmed public facts.
- Secrets live in the deployment environment only.

## Alternatives

- MIT: also permissive, but with no patent grant.
- AGPL-3.0: protects against closed hosted forks, but the product owner chose a permissive license.
- Proprietary: rejected by the product owner.
