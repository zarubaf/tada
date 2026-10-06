# 0009. Object storage through the S3 API

- Status: Proposed
- Date: 2026-10-06

## Context

tada stores originals, document versions, snapshots and exports.
The first deployment is self-hosted (ADR 0015).
The MinIO community edition stopped publishing maintained images in 2025.
Garage has no object lock and no object versioning, so the storage cannot enforce immutability.
An uploaded HTML or SVG file can run script in a browser if tada serves it inline.
Later, a club can want Nextcloud, OneDrive or a hosted S3 service.

## Decision

Storage:

- The `app` crate defines a `BlobStore` port: put, get, head and delete.
- The adapter uses the official `aws-sdk-s3` crate. It works with any S3-compatible service.
- Self-hosted deployments use Garage. Garage is reachable only on the internal Compose network. Its admin API is never public.
- The adapter uses path-style addresses and sets `request_checksum_calculation` to `WhenRequired`. The Garage container test covers this setting.
- Object keys start with the organization ID, followed by a generated ID. They never contain file names.
- PostgreSQL owns document IDs, versions, names, folders and permissions.

Uploads and downloads:

- All uploads and downloads go through the `api` crate. tada issues no presigned URLs.
  At club scale this costs little, and it gives revocation, audit and one place for checks.
- An upload goes to a staging key. The server checks the size, the content type and the scope, and then publishes the version in one transaction.
- The server rejects an upload above the configured size limit.
- Downloads send `Content-Disposition: attachment`, `X-Content-Type-Options: nosniff` and a restrictive `Content-Security-Policy`.
- Inline previews exist only for an allow-list of types: PDF, PNG, JPEG and plain text.

Immutability:

- The application never writes to the key of a published version. No API exists for that.
- Backups (ADR 0016) hold the second copy. The storage itself does not enforce immutability.

## Consequences

- A change of the S3 provider needs configuration only.
- A Nextcloud or OneDrive adapter implements the same port.
- All file traffic goes through the server. If file sizes grow, presigned URLs can come back through a new ADR.

## Alternatives

- Presigned URLs: less server traffic, but bearer credentials that we cannot revoke, and a public storage endpoint.
- MinIO: no maintained community images.
- SeaweedFS: more features and more operations work than we need.
- Files on the local disk: no path to a hosted service.
