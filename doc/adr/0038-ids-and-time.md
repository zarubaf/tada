# 0038. IDs and time

- Status: Proposed
- Date: 2026-10-06

## Context

Each record has a UUID and a human-readable ID (see [ARCHITECTURE.md](../ARCHITECTURE.md)).
Members talk about "ACT-042" in Telegram and in meetings, so these IDs must be short and stable.
A member of two events must still know which record an ID means.
Some records, for example documents and resources, belong to the organization, not to one event.
Events take place in Switzerland. Due dates are calendar dates, and reminders follow local time across daylight saving changes.
`sqlx` 0.9 supports `chrono` and `time`. `jiff-sqlx` 0.2 connects `jiff` to `sqlx` 0.9.
A minimal container image has no time zone database.

## Decision

Record IDs:

- Each primary key is a UUIDv7.
- Each kind of record has its own ID type, for example `EventId(Uuid)`. The compiler rejects an `EventId` where an `ActionId` is expected.
- A client can send the UUID of a new record in a create command. If a record with this UUID and the same content exists, the command returns it and changes nothing. This makes retries safe.
  If the client sends no UUID, the app generates one.
- A UUIDv7 contains its creation time. tada never uses a record ID as a secret; tokens follow ADR 0008.

Readable IDs:

- Each event has a short key, unique in its organization: two to eight capital letters and digits, for example `FLY28`.
- Event records have IDs that are local to their event. Organization records have IDs that are local to their organization.
- The format is `<PREFIX>-<number>`, with at least three digits. The full reference of an event record adds the event key: `FLY28/ACT-042`.
- Inside one event, for example in an event view or in a Telegram group bound to the event, the short form `ACT-042` is enough.
  In a private Telegram chat, the gateway resolves a short form only if exactly one of the member's events has it. Otherwise it asks.
- The prefixes are fixed English codes and do not change with the UI language:

| Kind          | Scope        | Prefix |
| ------------- | ------------ | ------ |
| Action        | event        | `ACT`  |
| Decision      | event        | `DEC`  |
| Risk          | event        | `RSK`  |
| Requirement   | event        | `REQ`  |
| Commitment    | event        | `COM`  |
| Open question | event        | `QST`  |
| Document      | organization | `DOC`  |
| Resource      | organization | `RES`  |

- A record gets its readable ID when it becomes accepted state. Proposals have no readable ID.
- A counter row for each scope and kind gives the next number. The command that creates the record runs `UPDATE … RETURNING` on the counter in the same transaction as the insert.
- Numbers are unique and never reused, also not after a deletion. Gaps are possible and acceptable.

Time:

- The app uses `jiff` for all time values. The `jiff` feature `tzdb-bundle-always` puts the time zone database into the binary, so the image needs no system time zone files.
- `store-pg` converts between `jiff` types and the wrapper types of `jiff-sqlx`. Queries name the wrapper types in their type overrides. Other crates never see the wrapper types.
- `utoipa` documents `jiff` types through its feature `jiff_0_2`.
- An instant, for example a creation time, is a `Timestamp` in a `timestamptz` column. Storage and the API use UTC; the API format is RFC 3339.
- A calendar date, for example a due date or an event day, is a `civil::Date` in a `date` column. It has no time zone.
- Each event has an IANA time zone. The default is `Europe/Zurich`.
- A schedule stores a local time and the time zone, not a UTC instant. It converts with `jiff`'s `Disambiguation::Compatible` (as in RFC 5545):
  - If the local time does not exist because of a daylight saving change, the run moves forward by the length of the gap, for example from 02:30 to 03:30.
  - If the local time exists twice, the run starts once, at the first occurrence.
- The `app` crate has a `Clock` port. Tests use a fixed clock.
- The database clock is the authority for job due times and leases (ADR 0007). The app clock is the authority for domain times, for example the creation time of a record.
- A week starts on Monday. Swiss public holidays come later, through a working-day calendar for each organization.

## Consequences

- IDs in the UI and in Telegram are short and easy to say, and the event key makes them unique for each member.
- Daylight saving changes do not skip or double reminders.
- `jiff` is not 1.0 yet; an update can need small code changes.
- A time zone rule change needs a new release of tada, because the time zone database is in the binary.

## Alternatives

- `bigint` keys: they show counts and order, and need the database to create them.
- UUIDv4: no time order, so indexes grow less well.
- `chrono` with `chrono-tz`: mature, but `chrono-tz` had no release since July 2025.
- Gap-free numbering: it needs a lock for each event and kind for the whole transaction, and nobody needs it.
- Localized prefixes, for example `AUF` for „Aufgabe“: an ID would change with the UI language.
