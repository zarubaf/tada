# 0074. Orphan sweep of the object storage

- Status: Proposed
- Date: 2026-10-10
- Amends: [0009](0009-object-storage.md)

## Context

An upload goes to the object storage before the transaction publishes the version ([ADR 0009](0009-object-storage.md)).
If the transaction fails or the process stops, the object stays without a document version.
An interrupted multipart upload leaves parts that cost space and have no key in a normal listing.
A legal redaction deletes the object of a document version after the commit, and this can fail ([ADR 0072](0072-legal-redaction-and-deletion-journal.md)).
The database knows which objects are referenced. The storage does not.

## Decision

Port:

- The `BlobStore` port gets three methods: `list(prefix)`, `list_incomplete_uploads(prefix)` and `abort_upload(key, upload_id)`.
- `list` streams pages of keys with their last-modified time.
- `list_incomplete_uploads` returns the key, the upload ID and the time of initiation.

Sweep:

- The worker runs the sweep once in 24 hours, in the pattern of [ADR 0065](0065-worker-deletes-ended-rate-limit-counters.md).
- The grace period is 24 hours.
- For each organization, the sweep deletes each object with a key `{organization_id}/{uuid}` that is older than the grace and that no document version with content refers to.
- It aborts each incomplete multipart upload that began before the grace.
- Other prefixes stay untouched. This protects the deletion journal under `redaction-journal/`.
- The grace covers an upload whose version row is not yet committed when the listing starts.

Configuration:

- The worker settings get the storage section, as the `serve` role has. `doc/settings.md` shows the change.
- The deployment repository can add a bucket lifecycle rule for multipart parts ([ADR 0033](0033-deployment-outside-this-repository.md)). tada does not need it.

## Consequences

- Failed uploads and failed object deletes clean themselves up within about two days.
- The worker needs access to the object storage.
- The sweep reads the full listing of the bucket. At club scale this costs little.
- A change of the key format must update the sweep in the same change.

## Alternatives

- A bucket lifecycle rule only: rejected, because it cannot know which objects the database refers to and would delete live documents or keep orphans.
- Delete at the failure site only: rejected, because a crash between upload and cleanup leaves the object.
