# 0072. Legal redaction and deletion journal

- Status: Proposed
- Date: 2026-10-10

## Context

[ADR 0045](0045-data-protection.md) makes legal redaction the one audited exception to immutability.
Evidence, fact versions, proposals and document versions are immutable, and database triggers enforce this ([ADR 0049](0049-entities-and-fact-model.md), [ADR 0051](0051-document-drafts-and-provenance.md)).
A redaction must remove content without opening a general way to change evidence.
A restored backup brings deleted content back, so a record of all redactions must survive a restore.
[ADR 0045](0045-data-protection.md) forbids a hash of redacted personal data, because a hash of a short value can identify the person.

## Decision

Tombstone:

- Each table with personal content gets a column `redacted_at`.
- A redaction sets the content columns to NULL and sets `redacted_at`.
- A CHECK constraint allows NULL content only when `redacted_at` is set.
- Content per target:
  - Source version: text, URL, hash and the personal parts of the author. The actor kind and ID stay. The search vector becomes empty. The quotes that copy the text are redacted too: in evidence links, proposal evidence and record evidence.
  - Fact version: the value.
  - Document version: the Markdown, the file name and the hash. The stored object is deleted after the commit. This is best effort, and the orphan sweep ([ADR 0074](0074-orphan-sweep.md)) is the backstop.
    An upload version and its source version hold the same file, so a redaction of one redacts the other too.
  - Proposal: the text values of the operation (the kind and the IDs stay), the reason, the quotes of the manifest and the lint warnings.
  - Person and institution: name, email address, phone number and account link.
    The proposals that created the person or institution lose their text values and their reason too, as a proposal target does.
  - Event (by retention only): all source items, fact versions and proposals of the event, the texts of open questions, actions, commitments and workstreams, the quotes of record evidence, the document names and versions, and the dispute notes.
- The name of an event, the field labels, statuses, dates, numbers and IDs stay after an event redaction. They should not name a person.
- A redacted fact version, proposal, action, commitment, person or institution also loses the quotes of its evidence.
- The dispute notes about a redacted source item, fact version, document version or proposal are redacted too.
- A redaction that empties an open proposal also rejects it, in the same transaction. The reviewer is the actor of the redaction, and the result has no free text.

Trigger rule:

- The triggers on immutable tables allow exactly one shape of UPDATE.
- It runs in a transaction that sets `SET LOCAL tada.redaction = 'on'`.
- `OLD.redacted_at` is NULL and `NEW.redacted_at` is not NULL.
- Only content columns change, and only to NULL.
- Every other UPDATE and every DELETE stays refused. This holds also with the setting on.
- A redaction locks its target and the cited source items and versions `FOR UPDATE` before it reads the content that copies them.
  Each writer that copies or cites a source version locks it `FOR SHARE` first. A writer that finds the version redacted refuses, writes a tombstone of the quote, or starts a new source item.
- Only the `RedactionStore` port sets the setting. The `app::redaction` module owns the rules, and `store-pg` implements the port.
- A test proves that an UPDATE of another column, a non-NULL content value or a redacted row fails with the setting on.

Reads and downloads:

- A read maps a redacted row to a `Redacted` variant. The API returns `"redacted": true` and empty content.
- The download of a redacted document version answers `410` with the problem code `content-redacted`.
- The web client shows „entfernt“.
- A manifest entry that points to redacted content resolves to „entfernt“. Links and manifests still resolve.

Redaction record and command:

- The table `redaction` stores the ID, the organization, the target kind, the target ID, the reason (`request` or `retention`), the actor and the time. It stores no content and no hash.
- The audit log records `<kind>.redact` with `detail.reason`.
  A migration widens the `CHECK` constraint of the audit detail for the key `reason`.
  Its value is `request` or `retention`.
- `POST /api/v1/redactions` takes a list of targets and the reason `request`. Only an owner calls it. All targets change in one transaction.
- The targets are `source-item` (all its versions), `fact-version`, `document-version`, `proposal`, `person` and `institution`. The target `event` exists for the retention job only.
- A redaction of a redacted target does nothing.

Deletion journal:

- In the transaction of a redaction, tada adds the job `write-redaction-journal` ([ADR 0054](0054-job-queue-implementation.md)).
- The job writes one object for each redaction under `redaction-journal/{organization_id}/{redaction_id}.json`.
- The object holds the redaction ID, the organization ID, the target kind and ID, the reason and the time. It holds no content.
- The journal objects are append-only. Only the redaction job writes them.
- The command `tada redactions replay` reads all journal objects, applies each redaction that is not yet applied in time order, and prints the counts. It is idempotent.
- The replay acts as the service identity `redaction-replay`. It writes the `redaction` row with the ID, the reason and the time of the journal, and without an actor, because the journal does not name the member.
- A restored database can hold open an event that the retention job redacted. The replay then closes the event first, with the audit event `event.close`, at the time of the redaction if the event has no close time. Outside the replay, a redaction of an open event is an invalid transition.
- The operator keeps the journal objects out of a restore of the object storage and runs the replay after each restore ([ADR 0033](0033-deployment-outside-this-repository.md)).

## Consequences

- A redaction cannot change evidence. The trigger allows tombstones and nothing else.
- The journal lives outside the database, so a database restore does not lose it.
- A job writes the journal, so a short delay is possible between the redaction and the object.
  The replay applies a redaction only if its journal object exists.
  A restore of the database to a time before the job wrote the object cannot recover that redaction.
- Every new table with personal content must join the redaction rules and the trigger in the same change.
- The replay needs the journal prefix in the object storage. A storage restore that overwrites it loses the journal.

## Alternatives

- A separate database role with a `SECURITY DEFINER` function: rejected, because it needs more operator work and a second set of grants.
- A journal in the database only: rejected, because a restore loses it.
- Hashes of the redacted content in the journal: rejected, because [ADR 0045](0045-data-protection.md) forbids them.
- Disable the triggers during a redaction: rejected, because it would allow any change in that transaction.
