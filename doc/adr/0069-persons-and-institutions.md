# 0069. Persons and institutions

- Status: Proposed
- Date: 2026-10-09
- Amends: [0049](0049-entities-and-fact-model.md), [0050](0050-proposals-and-review.md)

## Context

ADR 0049 defines persons and institutions as organization records.
A commitment names who promised (ADR 0068). A free text would give the same supplier a different spelling in each event.
Persons hold personal data: a name, an email address and a phone number (ADR 0045).
ADR 0050 asks the review to show duplicate candidates and to let the reviewer link a proposal to an existing record.
This ADR sets the permissions, the duplicate search and the linking for Slice 2a.

## Decision

Records:

- A person (`PER`, organization scope) has a name (at most 200 characters), an optional email address, an optional phone number (at most 50) and an optional link to a user account.
- An institution (`INS`, organization scope) has a name (at most 200), a kind (`authority`, `company`, `club` or `other`), an optional email address and an optional phone number.
- Both have a record version. Readable IDs follow ADR 0038.
- Participations are not part of 2a.

Permissions:

| Command                           | Who                                                        |
| --------------------------------- | ---------------------------------------------------------- |
| Create a person or an institution | a member with the contributor or manager role in any event |
| Change a person or an institution | organization owner or admin                                |
| Read persons and institutions     | each member with any event role                            |

- A member without an event role does not see persons and institutions (ADR 0052).
- A record of another organization is invisible. A command that names one returns `validation-failed` with the field code `unknown-record`.

Proposals:

- The operations `CreatePerson` and `CreateInstitution` create these records.
  The proposal carries the UUID of the new record (ADR 0050).
- These operations name no event, because the records belong to the organization.
  A changeset of an event and a changeset of the organization can both contain them.
- The agent searches the existing persons and institutions before it proposes a new one (ADR 0040). The MCP instructions say this.

Duplicate candidates:

- The review of a create operation shows the records of the organization with a similar name.
- The Rust code finds them with a normalized name match: it converts the names to lowercase, removes accents and punctuation, and compares the words.
  It needs no PostgreSQL extension.
- The list queries support the parameter `q` for a name search with the same normalization.

Linking:

- The apply request carries `links: [{ proposal_id, record_id }]`.
- A link is valid only if the proposal is a `CreatePerson` or a `CreateInstitution` and the record is of the same kind and of the same organization. Otherwise the request fails and changes nothing.
- A linked proposal gets the review result `accepted-with-edit`. Its review source version names the chosen record.
- Proposals that depend on the linked proposal resolve the proposed UUID to the linked record in the same transaction.
- A linked proposal creates no new record and takes no readable ID.

## Consequences

- One supplier is one record across the events of an organization.
- A club with a few hundred records needs no search index. The normalized match runs over a small set.
- The match finds spelling variants but not typing errors. A reviewer can still link by hand.
- Personal data in these records enters the data inventory and the structured export in the same commit.
- Real personal data stays out until the gates of the roadmap close.

## Alternatives

- A free-text promisor on the commitment: the same supplier appears under different names, and a person cannot be found in another event.
- A fuzzy match with the PostgreSQL extension `pg_trgm`: it finds typing errors, but it adds an extension to the image and to the operator's duty (ADR 0033).
- Automatic merge of equal names: two different persons with the same name would merge without a human check.
- Any member creates and changes records: a viewer could change the contact data of a supplier.
