# 0009. Object storage through the S3 API

- Status: Proposed
- Date: 2026-10-06

## Context

tada stores originals, document versions, snapshots and exports.
The first deployment is self-hosted (ADR 0015).
The MinIO community edition stopped publishing maintained images in 2025.
Later, a club can want Nextcloud, OneDrive or a hosted S3 service.

## Decision

- The `documents` code module defines a `BlobStore` port: put, get, head, delete and presign.
- The first adapter uses the AWS SDK S3 client and works with any S3-compatible service.
- Self-hosted deployments use Garage as the S3 service.
- Object keys are generated IDs, never file names.
- PostgreSQL owns document IDs, versions, names, folders and permissions.
- Uploads go to a staging key. The server checks the object and then publishes the version in one transaction.
- Approved versions are immutable. No upload URL can overwrite them.

## Consequences

- A change of the S3 provider needs configuration only.
- A Nextcloud or OneDrive adapter implements the same port.
- Integration tests run against Garage in a container.

## Alternatives

- MinIO: no maintained community images.
- SeaweedFS: more features and more operations work than we need.
- Files on the local disk: no presigned URLs and no path to a hosted service.
