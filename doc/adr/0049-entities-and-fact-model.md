# 0049. Typed entities and a field catalog for facts

- Status: Proposed
- Date: 2026-10-06

## Context

Planning data has two kinds of structure.
Entities, for example events, people, organizations, actions and decisions, have identity, relations, owners and workflows. Code must understand them.
Descriptive details, for example "expected visitors" or "entry fee policy", differ from event to event and appear during the planning.
A test with real planning statements showed this split. It also showed three needs: people and outside organizations as records, a status for their involvement, and an explicit "unknown".
A fixed column for each detail needs a migration for each new detail. Free text or a vector store cannot tell accepted values from assumptions and unknowns.
PRODUCT.md rejects a universal no-code schema designer.

## Decision

The model has three layers.

### 1. Typed entities (code; a migration changes them)

| Entity                                                                    | Scope        | Key content                                                                              |
| ------------------------------------------------------------------------- | ------------ | ---------------------------------------------------------------------------------------- |
| Event                                                                     | organization | key (ADR 0038), name, time zone, lifecycle status, date window, exact dates (optional)   |
| Person                                                                    | organization | name, contact channels, optional link to a user account                                  |
| Institution                                                               | organization | name, kind (authority, company, club, other), contact channels                           |
| Participation                                                             | event        | a person or an institution, a role and a status                                          |
| Action, milestone, decision, risk, requirement, commitment, open question | event        | as in the [glossary](../glossary.md), each with an owner, a status and an event-local ID |
| Document                                                                  | organization | as in ADR 0009                                                                           |

- A participation has a role from a role catalog, for example organizing committee member, honorary president, authority, sponsor or supplier.
  The status is `invited`, `interested`, `confirmed` or `declined`.
- A participation is not access. A person can participate without a user account. Access to tada comes only from memberships (ADR 0008).
- The date window and the exact dates are typed, because schedules and templates compute with them.
- These entities change only through migrations (ADR 0006).

### 2. Value types (code; a closed set)

Each fact value has one of these types, as a Rust `enum`:

| Value type    | Content                                                    | Example                 |
| ------------- | ---------------------------------------------------------- | ----------------------- |
| `text`        | a short text                                               | "military airfield"     |
| `boolean`     | yes or no                                                  | public = yes            |
| `quantity`    | a number or a range, with a unit                           | 15,000–25,000 persons   |
| `money`       | an amount or a range, with a currency                      | CHF 80,000–120,000      |
| `date`        | a calendar date                                            | 2030-05-18              |
| `date_window` | a range with a granularity of day, week or month           | May–June 2030           |
| `choice`      | one or more values from the field's list                   | airshow, static display |
| `reference`   | a link to a person, an institution, a document or an event | the airfield operator   |

- A value always matches its field definition. The type system makes an invalid value impossible to construct.

### 3. Field catalog and facts (data; the project changes them)

A field definition is a record:

| Part        | Content                                          |
| ----------- | ------------------------------------------------ |
| key         | stable, for example `visitor_estimate`           |
| label       | German text; built-in fields use Fluent messages |
| value type  | from layer 2, with its unit or its choice list   |
| cardinality | one value or a list                              |
| description | the meaning, for people and for AI agents        |
| module      | for example `core`, `aviation`, `catering`       |
| status      | `active` or `deprecated`                         |

A fact connects an event, a field definition and a value:

| Part      | Content                                                  |
| --------- | -------------------------------------------------------- |
| status    | `accepted`, `assumption` or `unknown`                    |
| value     | none if the status is `unknown`                          |
| qualifier | an optional short note, for example "about" or "per day" |
| version   | the record version (ADR 0006)                            |
| evidence  | one or more evidence links                               |

- An event has at most one current fact for each field. A field with the cardinality "list" holds the list in one fact.
- Each change creates a new fact version. Old versions stay, because documents and decisions refer to exact versions (ADR 0051).
- "Unknown" is a fact. tada can list all unknowns of an event, and an agent never fills them by guessing.

Catalog scope and evolution:

- tada ships module catalogs as versioned seed data, for example `core` and `aviation`.
- For now, new field definitions belong to one event. The event manager accepts them.
- Organization-wide fields come later. An organization admin accepts them.
- A choice list can grow: an agent proposes a new value, and the event manager accepts it.
- A field is never deleted. It becomes `deprecated`: readable, but closed for new facts.
- A merge proposal moves the facts of one field to another field, with their evidence. The old key stays as an alias.
- A label can change; the key never changes.
- A field becomes a typed column of an entity only when code must compute with it. This change is a migration.

For agents:

- The MCP tool "get the event schema" returns the entities, the role catalog and the active field definitions with their descriptions and JSON Schemas (ADR 0040).
- An agent must use an existing field first. A proposal for a new field definition needs a description that separates it from the existing fields.

Example with invented data:

| Kind          | Content                                                                                                 |
| ------------- | ------------------------------------------------------------------------------------------------------- |
| Event         | `TEST30` "Open Day Testwil", 2 days, date window May–June 2030 (accepted), exact dates unknown          |
| Participation | invented person A: organizing committee, confirmed; invented person B: organizing committee, interested |
| Participation | "Testwil Air Navigation": authority, invited                                                            |
| Fact          | `audience` = public (accepted)                                                                          |
| Fact          | `entry_fee_policy` = low (accepted); `entry_fee_adult` unknown                                          |
| Fact          | `components` = airshow, static display, catering (accepted)                                             |
| Requirement   | "The event avoids public holidays."                                                                     |
| Open question | "Which weekend?"                                                                                        |

## Consequences

- People, institutions and their roles are queryable and reusable across events of an organization.
- New details need no release. The catalog grows with the project, and review keeps it clean.
- Reports across events work for typed entities. For facts, they work only where events share field definitions.
- JSON values in the fact table need validation in code, because the database cannot check them by type.

## Alternatives

- One column for each detail: a migration and a release for each new detail.
- An entity-attribute-value model without value types: values cannot be compared or validated.
- Free text with a vector index: no difference between accepted, assumption and unknown.
- People and institutions as facts: no identity, so the same person appears as different texts in each event.
- A user-defined entity designer: the "universal no-code schema designer" that PRODUCT.md rejects.
