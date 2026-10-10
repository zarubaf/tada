# 0073. Person export and dispute notes

- Status: Proposed
- Date: 2026-10-10

## Context

[ADR 0045](0045-data-protection.md) requires that an owner can export all data about one person as JSON (Art. 25 and Art. 28), and that an owner can correct data or add a dispute note when the evidence must stay (Art. 32).
[ADR 0059](0059-structured-export.md) exports a whole organization. It does not answer a request about one person.
Names appear in free text: in sources, facts, proposals, documents and work records.
tada must find these places without a guess that names the wrong person.
[ADR 0069](0069-persons-and-institutions.md) defines the person record and its link to an account.

## Decision

Subject:

- The subject of an export is a person record (`PER`).
- If the person links to an account, the export includes the account data.
- To export a member without a person record, an owner creates a person and links it ([ADR 0069](0069-persons-and-institutions.md)). There is no second entry point.

Permissions:

- Only an organization owner exports, lists mentions, redacts and adds dispute notes.
- A member and an admin get `forbidden`.
- An AI caller (MCP or API token) cannot call these operations: the REST API takes no API token, and the MCP server has no tool for them.
- An owner or an admin of another organization gets `not-found`.

Mentions:

- `GET /api/v1/persons/{id}/mentions` lists the places that mention the person.
- A mention has a kind, an ID, an event ID, an excerpt of at most 200 characters around the match, and the flags `redacted` and `disputed`.
- tada matches the name, the email address and the phone number of the person.
  Each term has at least 3 characters.
  The match is a case-insensitive match of the term inside the text, after NFC normalization ([ADR 0060](0060-unicode-normalization-of-source-texts.md)).
- It searches source versions, fact versions, proposals, open questions, actions, commitments, workstreams and the Markdown of document versions.
  A document version also matches by the file name of its upload and by the name of its document.
- It also lists the records that refer to the person by ID, such as the promisor of a commitment.
- tada does not guess other spellings. The owner can search more with the existing search.

Export:

- `GET /api/v1/persons/{id}/export` answers with `Content-Disposition: attachment` and a JSON document with `format_version` 1.
- The document holds the time of creation, the organization (ID and name), the person, the references, the mentions with their full text and the dispute notes.
- For a linked account it adds: the display name, the email address, the organization role, the event memberships and roles, the Telegram link, the sessions (created, last used, user agent), the API tokens (name and dates), the invitations to the email address, and the audit events where the user is actor or subject (IDs and actions).
- It holds no secret and no token hash.
- The audit log records `person.export`.

Correction and dispute notes:

- The existing change commands of persons, institutions and work records stay the way to correct data.
- Evidence is immutable, so a correction cannot change it. An owner adds a dispute note instead (Art. 32(3)).
- A dispute note has an ID, an organization, a target kind (`source-item`, `fact-version`, `document-version` or `proposal`), a target ID, a text of 1 to 1000 characters, an author and a time.
- `POST /api/v1/dispute-notes` creates a note. The audit log records `dispute_note.create`.
- A note is also possible in a closed event: it changes no evidence.
- A redacted target gets no note (`invalid-transition`).
- Each reader of the target sees its notes: the API responses that show evidence or facts have the field `dispute_notes`.
- A redaction of an event redacts its notes ([ADR 0072](0072-legal-redaction-and-deletion-journal.md)).

Web client:

- The person detail shows the section „Datenschutz“ to owners.
- It offers „Export herunterladen“, the list of mentions with checkboxes, „Bestreiten“ (a dialog with a text) and „Entfernen“.
- „Entfernen“ opens a confirmation dialog that names the count and says that the action cannot be undone.
- The places that show evidence or fact history show the dispute note and „entfernt“.

## Consequences

- An owner can answer an access request in one place.
- The matching rule can miss a spelling variant. The owner must find such a mention by hand.
- It can also list a mention of another person with the same name. The owner checks the list before a redaction.
- Each new text column with free text joins the mention search in the same change.
- The export of an account reads several tables. A new table with account data joins the export and the data inventory together.

## Alternatives

- Fuzzy name matching: rejected, because it lists the wrong people and a redaction of a false match destroys evidence.
- An export of every member without a person record: rejected, because it adds a second entry point and a second subject type.
- A correction of evidence in place: rejected, because evidence is immutable ([ADR 0045](0045-data-protection.md)).
