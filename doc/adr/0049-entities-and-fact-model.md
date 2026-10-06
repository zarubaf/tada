# 0049. Typed entities and a field catalog for facts

- Status: Proposed
- Date: 2026-10-06

## Context

Planning data has two kinds of structure.
Entities, for example events, people, organizations, actions and decisions, have identity, relations, owners and workflows. Code must understand them.
Descriptive details, for example "expected visitors" or "entry fee policy", differ from event to event and appear during the planning.
A test with real planning statements showed this split. It also showed three needs: people and outside organizations as records, a status for their involvement, and an explicit "unknown".
Some details, for example the date window, are both: code computes with them, and they still need a status, evidence and history.
A fixed column for each detail needs a migration for each new detail. Free text or a vector store cannot tell accepted values from assumptions and unknowns.
PRODUCT.md rejects a universal no-code schema designer.

## Decision

The model has three layers.

### 1. Typed entities (code; a migration changes them)

| Entity                                                                    | Scope        | Key content                                                                              |
| ------------------------------------------------------------------------- | ------------ | ---------------------------------------------------------------------------------------- |
| Event                                                                     | organization | key (ADR 0038), name, time zone, lifecycle status                                        |
| Person                                                                    | organization | name, contact channels, optional link to a user account                                  |
| Institution                                                               | organization | name, kind (authority, company, club, other), contact channels                           |
| Participation                                                             | event        | a person or an institution, a participation role and a status                            |
| Action, milestone, decision, risk, requirement, commitment, open question | event        | as in the [glossary](../glossary.md), each with an owner, a status and an event-local ID |
| Document                                                                  | organization | as in ADR 0009                                                                           |

- A participation has a participation role from a role catalog, for example organizing committee member, honorary president, authority, sponsor or supplier.
  The status is `invited`, `interested`, `confirmed` or `declined`.
- A participation is not access. A person can participate without a user account. Access to tada comes only from memberships and event roles (ADRs 0008 and 0052).
- Persons and institutions are organization records. Before a proposal creates a new one, the agent searches the existing ones (ADR 0040). The review shows existing records with similar names as duplicate candidates (ADR 0050).
- These entities change only through migrations (ADR 0006).

### 2. Value types (code; a closed set)

Each fact value has one of these types, as a Rust `enum`:

| Value type    | Content                                                                    | Example                       |
| ------------- | -------------------------------------------------------------------------- | ----------------------------- |
| `text`        | a short text                                                               | "military airfield"           |
| `boolean`     | yes or no                                                                  | public = yes                  |
| `quantity`    | a number or a range, with a unit                                           | 15,000–25,000 persons per day |
| `money`       | an amount or a range, with a currency                                      | CHF 80,000–120,000            |
| `date`        | a calendar date                                                            | 2030-05-18                    |
| `date_window` | a range with a granularity of day, week or month                           | May–June 2030                 |
| `choice`      | one value, or several values if the field allows it, from the field's list | airshow, static display       |
| `reference`   | a link to a person, an institution, a document or an event                 | the airfield operator         |

- A value always matches its field definition. The type system makes an invalid value impossible to construct.
- One Rust type exists for each value type. For example, the `DateWindow` type serves both the `date_window` fact value and the code that computes with the date window.
- The unit is part of the field definition, for example "persons per day". A value has no free-text qualifier.
- A value can be marked as approximate (`approximate: true`), for example "about 20,000".

### 3. Field catalog and facts (data; the project changes them)

A field definition is a record:

| Part        | Content                                               |
| ----------- | ----------------------------------------------------- |
| key         | stable, `snake_case`, for example `visitor_estimate`  |
| label       | German text; built-in fields use Fluent messages      |
| value type  | from layer 2, with its unit or its choice list        |
| multiple    | for `choice` only: whether several values are allowed |
| description | the meaning, for people and for AI agents             |
| module      | for example `core`, `aviation`, `catering`            |
| status      | `active` or `deprecated`                              |

- The value type of a key never changes. A different type needs a new field.
- Only `choice` fields can hold several values. Other fields hold one value.

A fact has two parts:

- The `fact` record is the identity: one event and one field. The database allows at most one `fact` for each pair.
- Each `fact_version` record is one immutable state of the fact. A new state appends a new version.

The state of a version is a Rust `enum`, so that an unknown fact cannot hold a value:

```rust
enum FactState<V> {
    Accepted(V),
    Assumption(V),
    Unknown,
}
```

- Each fact version has evidence: one or more evidence links (ADR 0050).
- Documents and decisions refer to exact fact versions (ADR 0051).
- "Unknown" is a fact state. tada can list all unknowns of an event, and an agent never fills them by guessing.
- Fact versions are immutable, except for a legal redaction (ADR 0045).

Reserved core fields:

- The `core` module contains fields whose keys are reserved in code, for example `date_window` and `exact_dates`.
- Code that computes with such a field, for example schedules, templates and reminders, reads it through a typed accessor in the `app` crate. The accessor returns the `FactState` of the current version.
- A field that code must compute with becomes a reserved core field with a typed accessor. It keeps its status, evidence and history. It never becomes a plain column.

Catalog scope and evolution:

- tada ships module catalogs as versioned seed data, for example `core` and `aviation`.
- For now, new field definitions belong to one event. The event manager accepts them (ADR 0052).
- Organization-wide fields come later. An organization admin accepts them.
- A choice list can grow: an agent proposes a new value, and the event manager accepts it.
- A field is never deleted. It becomes `deprecated`: readable, but closed for new facts.
- A field that is wrong or a duplicate is deprecated. The agent then proposes the facts again on the correct field, with their evidence. There is no automatic merge in Slice 1.
- A label can change; the key never changes.

For agents:

- The MCP tool "get the event schema" returns the entities, the participation role catalog and the active field definitions with their descriptions and JSON Schemas (ADR 0040).
- An agent must use an existing field first. A proposal for a new field definition needs a description that separates it from the existing fields.

Example with invented data:

| Kind          | Content                                                                                                               |
| ------------- | --------------------------------------------------------------------------------------------------------------------- |
| Event         | `TEST30` "Open Day Testwil"                                                                                           |
| Fact          | `date_window` = May–June 2030 (accepted)                                                                              |
| Fact          | `exact_dates`: unknown                                                                                                |
| Fact          | `duration_days` = 2 (accepted)                                                                                        |
| Participation | invented person A: organizing committee member, confirmed; invented person B: organizing committee member, interested |
| Participation | "Testwil Air Navigation": authority, invited                                                                          |
| Fact          | `audience` = public (accepted)                                                                                        |
| Fact          | `entry_fee_policy` = low (accepted); `entry_fee_adult`: unknown                                                       |
| Fact          | `components` = airshow, static display, catering (accepted)                                                           |
| Requirement   | "The event avoids public holidays."                                                                                   |
| Open question | "Which weekend?"                                                                                                      |

## Consequences

- People, institutions and their participation roles are queryable and reusable across events of an organization.
- New details need no release. The catalog grows with the project, and review keeps it clean.
- Dates that code computes with keep their status and evidence, like all other facts.
- Reports across events work for typed entities. For facts, they work only where events share field definitions.
- JSON values in the fact table need validation in code, because the database cannot check them by type.

## Alternatives

- One column for each detail: a migration and a release for each new detail.
- Typed date columns on the event: no status, no evidence and no history for the most important planning facts.
- An entity-attribute-value model without value types: values cannot be compared or validated.
- Free text with a vector index: no difference between accepted, assumption and unknown.
- People and institutions as facts: no identity, so the same person appears as different texts in each event.
- A list cardinality for all value types: one fact could not hold evidence for each item, and two intakes that add items would conflict.
- Field merge in Slice 1: undefined behavior for type mismatches and for a target field that already has a fact.
- A user-defined entity designer: the "universal no-code schema designer" that PRODUCT.md rejects.
