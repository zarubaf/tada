# 0045. Data protection under the revDSG

- Status: Proposed
- Date: 2026-10-06

## Context

tada processes personal data of members, volunteers, suppliers and third parties named in emails and documents.
The Swiss Federal Act on Data Protection (revDSG, in force since 1 September 2023, [SR 235.1](https://www.fedlex.admin.ch/eli/cc/2022/491/de)) applies to clubs in Switzerland.
Slice 2 sends source texts to a model provider (ADR 0047). ADR 0010 forbids this before this ADR is accepted.
This ADR states what the product must support. It is not legal advice; the club must check its own duties.

## Decision

Roles:

- The organization (the club) is the controller for its data in tada.
- The operator of an installation is a processor of the club (Art. 9). The club and the operator need a written processing agreement.
- The operator's providers are sub-processors: hosting, backups, mail and, from Slice 2, the model provider.
- Telegram is a channel that each member chooses. Member AI clients that use MCP (ADR 0040) process data under the member's own account; tada gives them only what the member can see.

Data inventory:

- `doc/data-inventory.md` lists each category of personal data, where tada stores it, why, and for how long.
- Each new table or field with personal data updates the inventory in the same pull request.

Product requirements:

- Privacy by design and by default (Art. 7): new members see only their events; optional channels are off until a member enables them.
- Information (Art. 19): the web client shows a privacy notice. The organization edits its text; tada provides a template with the processors as categories.
- Access and portability (Art. 25 and Art. 28): an owner can export all data about one person as JSON.
- Correction and deletion (Art. 32): an owner can correct or delete data about a person. Deletion removes the person from records, sources, snapshots, extracted facts and search indexes. Accepted decisions keep a placeholder instead of the name.
- Retention: each organization sets retention periods for sources and for closed events. A job deletes data after the period.
- Backups and logs: deleted data disappears from backups when the backups expire. The operator states the backup and log retention in the privacy notice (ADR 0035).

Model provider (Slice 2):

- Before the first call, the operator checks and records: the provider's processing terms, no training on the data, the retention period, the processing region and the legal basis for a transfer abroad (Art. 16 and Art. 17).
- Each organization can switch off all model calls (ADR 0010).
- Prompts contain only the data that the task needs.

Breaches:

- The operator reports a breach that likely leads to a high risk to the FDPIC as soon as possible (Art. 24), and informs the club at once.
- The deployment repository contains the procedure (ADR 0033).

Impact assessment:

- A large public event with many volunteers and suppliers can need a data protection impact assessment (Art. 22). The club decides this before it enters real data of third parties. tada provides the data inventory as input.

## Consequences

- No real personal data goes to a model provider before the operator records the checks above.
- Deletion is a product feature with tests, not a manual database task.
- Each schema change with personal data also changes the inventory.

## Alternatives

- Treat data protection as an operator task only: the product would lack export, deletion and retention, and each operator would build them alone.
- Anonymize all source texts before AI calls: names and roles are often the content that the extraction needs.
