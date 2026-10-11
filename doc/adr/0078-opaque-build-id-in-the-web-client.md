# 0078. Opaque build ID in the web client

- Status: Proposed
- Date: 2026-10-11
- Amends: [0075](0075-versions-of-tada-and-its-components.md)

## Context

[ADR 0075](0075-versions-of-tada-and-its-components.md) says that tada shows no version to clients that are not signed in.
It also says that the build writes the product version and the commit SHA into the web client.
The two rules conflict.
The files of the web client are public: the sign-in page loads the main chunk, and a client can load each other chunk by its URL.
So each client could read the version and the commit from the files.
The repository is public, so the commit names the exact source of a deployment.

The web client still needs a build identity: it compares its build with the server and asks the member to load the page again when they differ.

## Decision

- Each build has a random build ID of 128 bits, written as 32 hex digits.
  The ID has no relation to the version or the commit, so it reveals nothing about them.
- The image build makes one ID and gives the same value as the build argument `TADA_BUILD_ID` to the web build and to the build of the binary.
- A build without `TADA_BUILD_ID` makes its own random ID.
  The development tasks `dev:serve` and `dev:web` set the fixed ID `development`, and the unit tests and the browser checks use a fixed ID too.
- The web build writes only the build ID into the client.
- `GET /api/v1/system/version` returns the build ID of the server as `web_build`, next to the versions.
- The web client compares its build ID with `web_build`.
  When they differ, it asks the member to load the page again.
- The footer shows `tada <version> · <commit>` from `GET /api/v1/system/version`, so only after a sign-in.
  Until the answer arrives, the footer says „Versionen“.
- A check of the built files fails if a file holds the product version or the commit.

The other parts of ADR 0075 stay in force.

## Consequences

- A client that is not signed in cannot read the version or the commit from the files, and it cannot compute them from the ID.
- The list of the components shows the version of the web client only if its ID is the one of the server.
  Otherwise it shows the ID and marks the web client as another version.
- Two builds of the same commit have different IDs.
  A member gets the reload prompt after each new image, also when only the image changed.
- A server and a web client from two separate builds without the same `TADA_BUILD_ID` always differ.
  So each build that delivers both must give them the same ID.

## Alternatives

- Load the build identity from a lazy chunk after the sign-in: each chunk is public by its URL.
- Accept the disclosure and remove the rule of ADR 0075 for the web client: the commit names the exact source of a deployment.
- A hash of the version and the commit: the repository is public, so anyone can compute the hash of each tag and commit and find the version.
