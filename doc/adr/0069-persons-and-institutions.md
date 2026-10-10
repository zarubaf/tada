# 0069. Persons and institutions

- Status: Accepted
- Date: 2026-10-10
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
- A create takes an optional client ID, and a change follows the conventions of ADR 0068.
- The JSON name of the institution kind is `kind` in the direct API and `institution_kind` in the operation `CreateInstitution`, where `kind` is the tag of the operation.
- Each view carries `can_change` and the evidence of the accepted changes that created or changed the record (ADR 0068).
  The evidence follows the source reach of the caller. Evidence from a changeset of the organization is visible only to owners and admins.
- Participations are not part of 2a.

Permissions:

| Command                           | Who                                                        |
| --------------------------------- | ---------------------------------------------------------- |
| Create a person or an institution | a member with the contributor or manager role in any event |
| Link a person to an account       | organization owner or admin                                |
| Change a person or an institution | organization owner or admin                                |
| Read persons and institutions     | each member with any event role, owners and admins         |

- A member without an event role does not see persons and institutions (ADR 0052).
- A record of another organization is invisible. A command that names one returns `validation-failed` with the field code `unknown-record`.
- Only an organization owner or admin links a person to the user account of a member of the organization.
  They set or clear the link when they create the person and when they change it.
  A create with a link from any other caller returns `forbidden`, and the API stores nothing.
  A link to a user who is no member gives the field error `unknown-member` on `user_id`.
- A proposal never links an account: the operation `CreatePerson` has no field for it.
  Each reader of the person sees the link.

Proposals:

- The operations `CreatePerson` and `CreateInstitution` create these records.
  The proposal carries the UUID of the new record (ADR 0050).
- These operations name no event, because the records belong to the organization.
  A changeset of an event and a changeset of the organization can both contain them.
  The routing takes the event from the changeset (ADR 0067): in an event changeset these proposals go to the event managers.
- The agent searches the existing persons and institutions before it proposes a new one (ADR 0040). The MCP instructions say this.

Duplicate candidates:

- The review of a create operation shows the records of the organization with a similar name.
  The review shows them for an open proposal only, at most 5, and only records of the same kind as the proposal.
- The Rust code finds them with a normalized name match: it converts the names to lowercase, removes accents, folds `ß` to `ss`, turns each character that is not a letter or a digit into a space, and compares the words.
  So "Tent-Works Ltd." and "Tent Works Ltd" match.
  Two names match if they are equal or if all words of one name are words of the other.
  They also match if they share a word of at least four characters.
  Words of legal forms, for example `AG`, `GmbH` or `Verein`, do not count, because many names share them.
  It needs no PostgreSQL extension.
- The list queries support the parameter `q` for a name search with the same normalization.

Linking:

- The apply request carries `links: [{ proposal_id, record_id }]`.
- A link is valid only if the proposal is a selected `CreatePerson` or `CreateInstitution` without an edit and without a second link.
  The record must be of the same kind and of the same organization.
  Otherwise the apply returns `validation-failed` with the field `links/{i}` and the code `invalid-link`, and nothing changes.
- Each open proposal that depends on the linked proposal must be in the same apply, else the code is `dependents-not-selected`.
  A later apply does not know the link, so it would look for the proposed record.
  The reviewer can reject such a proposal first.
- A linked proposal gets the review result `accepted-with-edit`.
  Its review source version names the chosen record, for example `linked to PER-007`.
- Proposals that depend on the linked proposal resolve the proposed UUID to the linked record in the same transaction.
- A linked proposal creates no new record and takes no readable ID, and its evidence does not go to the existing record.

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
