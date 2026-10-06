# 0038. IDs and time

- Status: Proposed
- Date: 2026-10-06

## Context

Each record has a UUID and a human-readable ID for its event (see [ARCHITECTURE.md](../ARCHITECTURE.md)).
Members talk about "ACT-042" in Telegram and in meetings, so these IDs must be short and stable.
Events take place in Switzerland. Due dates are calendar dates, and reminders follow local time across daylight saving changes.
`sqlx` 0.9 supports `chrono` and `time`. `jiff-sqlx` 0.2 connects `jiff` to `sqlx` 0.9.

## Decision

Record IDs:

- Each primary key is a UUIDv7, generated in the app with the `uuid` crate before the insert.
  The app knows the ID before the transaction, which helps with idempotent commands.
- Each kind of record has its own ID type, for example `EventId(Uuid)`. The compiler rejects an `EventId` where an `ActionId` is expected.
- A UUIDv7 contains its creation time. tada never uses a record ID as a secret; tokens follow ADR 0008.

Event-local IDs:

- The format is `<PREFIX>-<number>`, with at least three digits: `ACT-042`.
- The prefixes are fixed English codes and do not change with the UI language:

| Kind          | Prefix |
| ------------- | ------ |
| Action        | `ACT`  |
| Decision      | `DEC`  |
| Risk          | `RSK`  |
| Requirement   | `REQ`  |
| Commitment    | `COM`  |
| Open question | `QST`  |
| Document      | `DOC`  |

- A counter row for each event and kind gives the next number. The command increments it in its own transaction, so numbers have no gaps and no duplicates.
- A number is never reused, also not after a deletion.

Time:

- The app uses `jiff` for all time values, and `jiff-sqlx` for the database.
- An instant, for example a creation time, is a `Timestamp` in a `timestamptz` column. Storage and the API use UTC; the API format is RFC 3339.
- A calendar date, for example a due date or an event day, is a `civil::Date` in a `date` column. It has no time zone.
- Each event has an IANA time zone. The default is `Europe/Zurich`.
- A schedule stores a local time and the time zone, not a UTC instant.
  If the local time does not exist because of a daylight saving change, the run starts at the next valid time.
  If the local time exists twice, the run starts once, at the first occurrence.
- The `app` crate has a `Clock` port. Tests use a fixed clock.
- A week starts on Monday. Swiss public holidays come later, through a working-day calendar for each organization.

## Consequences

- IDs in the UI and in Telegram are short and easy to say.
- Daylight saving changes do not move or double reminders.
- `jiff` is not 1.0 yet; an update can need small code changes.

## Alternatives

- `bigint` keys: they show counts and order to other organizations, and need the database to create them.
- UUIDv4: no time order, so indexes grow less well.
- `chrono` with `chrono-tz`: mature, but `chrono-tz` had no release since July 2025, and its daylight saving handling is less explicit.
- Localized prefixes, for example `AUF` for „Aufgabe“: an ID would change with the UI language.
