# 0045. Data protection under the revDSG

- Status: Accepted
- Date: 2026-10-06

## Context

tada processes personal data of members, volunteers, suppliers and third parties named in emails and documents.
The Swiss Federal Act on Data Protection (revDSG, in force since 1 September 2023, [SR 235.1](https://www.fedlex.admin.ch/eli/cc/2022/491/de)) applies to clubs in Switzerland.
From Slice 1, the MCP read tools (ADR 0040) give personal data to each member's own AI client. Some consumer AI plans can train on their inputs.
Slice 2 sends source texts to a model provider (ADR 0047).
Evidence, fact versions, provenance manifests and approved document versions are immutable (ADRs 0009, 0049, 0051). A deletion request can conflict with this.
Backups restore data that was deleted after the backup.
This ADR states what the product must support. It is not legal advice; the club must check its own duties.

## Decision

Gate:

- No real personal data enters an installation of tada before this ADR is accepted. This applies also to Slice 1.

Roles:

- The organization (the club) is the controller for its data in tada.
- The operator of an installation is a processor of the club (Art. 9). The club and the operator need a written processing agreement.
- The operator's providers are sub-processors: hosting, backups, mail and, from Slice 2, the model provider.
- Telegram is a channel that each member chooses.

Member AI clients (MCP):

- Before a member creates an API token for an MCP client, tada shows a notice:
  - the token gives the member's own AI client access to the personal data that the member can see,
  - the club's policy requires an AI plan that does not train on its inputs.
- The member confirms the notice. tada records the confirmation with the token.
- The organization can switch off MCP tokens.

Data inventory:

- `doc/data-inventory.md` lists each category of personal data, where tada stores it, why, and for how long.
- Each new table or field with personal data updates the inventory in the same pull request.

Product requirements:

- Privacy by design and by default (Art. 7): new members see only their events; optional channels are off until a member enables them.
- Security (Art. 8): the measures of ADRs 0008, 0009, 0035 and 0039 are the technical measures of tada.
- Information (Art. 19): the web client shows a privacy notice. The organization edits its text; tada provides a template with the processors as categories.
- Data of third parties: names in emails and documents are data that the club did not collect from the person. The club must inform these people (Art. 19(5)), unless an exception of Art. 20 applies. tada lists the persons and institutions that came from sources, so that the club can check this.
- Access and portability (Art. 25 and Art. 28): an owner can export all data about one person as JSON. The club answers a request within 30 days.
- Correction (Art. 32): an owner can correct data about a person. If the evidence must stay, the owner can add a dispute note to the record instead (Art. 32(3)).
- Retention (Art. 6(4)): each organization sets retention periods for sources and for closed events. A job deletes data after the period.

Legal redaction:

- Deletion is the one audited exception to immutability.
- A redaction replaces the content of a source version, a fact version, a snapshot or a document version with a tombstone. The IDs stay, so that links and manifests still resolve.
- tada keeps no hash of redacted personal data, because a hash of a short value can identify the person.
- A provenance manifest entry that points to redacted content resolves to „entfernt“ (redacted).
- The audit log records who redacted what and why, without the redacted content.
- A deletion journal stores the IDs of all redactions. After each restore of a backup, the operator replays the journal before the installation goes back online. The deployment repository contains this step (ADR 0033).

Backups and logs:

- Deleted data stays in backups until they expire; the deletion journal removes it again after a restore.
- The operator states the backup and log retention in the privacy notice (ADR 0035).

Model provider (Slice 2):

- Before the first call, the operator checks and records: the provider's processing terms, no training on the data, the retention period, the processing region and the legal basis for a transfer abroad (Art. 16 and Art. 17).
- Each organization can switch off all model calls (ADR 0010).
- Prompts contain only the data that the task needs.

Breaches (Art. 24):

- The club, as the controller, reports a breach that likely leads to a high risk to the FDPIC as soon as possible (Art. 24(1)).
- The operator, as the processor, reports each breach to the club as soon as possible (Art. 24(3)).
- The club informs the affected people when this is necessary for their protection, or when the FDPIC requires it (Art. 24(4)).
- tada provides an export of the affected records for this. The deployment repository contains the operator's procedure (ADR 0033).

Impact assessment:

- A large public event with many volunteers and suppliers can need a data protection impact assessment (Art. 22). The club decides this before it enters real data of third parties. tada provides the data inventory as input.

## Consequences

- No real personal data enters tada, and no data goes to a model provider, before the checks above.
- Deletion is a product feature with tests, not a manual database task.
- A redaction leaves visible gaps in evidence and documents. This is intended.
- Each schema change with personal data also changes the inventory.

## Alternatives

- Treat data protection as an operator task only: the product would lack export, deletion and retention, and each operator would build them alone.
- Strict immutability without redaction: a valid deletion request could not be fulfilled.
- Delete records completely: links and manifests would break, and the audit log would lose the fact that a deletion happened.
- Anonymize all source texts before AI calls: names and roles are often the content that the extraction needs.
