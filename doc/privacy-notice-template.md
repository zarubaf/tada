# Privacy notice template

This file explains the template of the privacy notice for operators and clubs (ADR 0045).
The German text is in `locales/de-CH/privacy.ftl`.
This file does not repeat it.

## How the notice works

- Each organization has one privacy notice.
- Until an owner writes an own text, tada shows the template.
- The owner edits the text on the page „Organisation“ in the settings.
  The text starts as a copy of the template.
- The owner can go back to the template.
- Each member reads the notice on the page „Datenschutz“.
  An invitee reads it on the invitation page, before the invitee accepts the invitation.
- The text is Markdown.
  The web client shows no raw HTML and no images.
  It shows a link only for `https` and `mailto` addresses.
- The notice is a record with a version.
  An owner who saves with an old version gets a conflict.
  Each change writes an audit event without the text.
- The structured export holds the notice with the other data of the organization (ADR 0059).

## What the template covers

- The controller: the club.
- The processors, as categories: operator, hosting, backups and mail.
  The template names no provider.
- The purposes of the processing.
- The categories of data, as in [the data inventory](data-inventory.md).
  They include the counters of sign-in attempts: keyed hashes of the email address and the IP address, kept for two hours at most.
- Telegram, as an optional channel.
  tada stores the Telegram user ID and the name of an account as soon as the account sends a valid code, before the member confirms the link.
  A message with the command `/vorschlag` becomes a source text that tada keeps with the event.
  Each reader of the event sees it, also through the AI clients of members.
  The message and the reply of the bot pass through Telegram.
- The AI clients of members.
  They read the data of the member through the read tools of MCP.
  They can also create proposals, and tada keeps the text of each proposal as a source text.
- Quotes as evidence in proposals and drafts.
  A quote can show the whole text of an organization-level changeset, which is not bound to one event.
  Each reader of the event then sees this text, in the web client and in AI clients.
- No scan of uploaded files for malware.
- The retention of backups, logs and exports.
  The operator keeps an export like a backup (ADR 0059).
- The retention periods of the club.
  An owner sets a period for closed events and a period for sources, or none.
  tada has no default period (ADR 0071).
  After a period, tada replaces the content with a tombstone and keeps the structure.
- The legal redaction on request.
  An owner removes the content of a source, a fact, a document version, a proposal or a person.
  tada keeps a record of each redaction with IDs and no content, in the database and in a journal in the object storage (ADR 0072).
  A restored backup can bring removed content back until the operator replays the journal.
- The access request.
  An owner exports all data about one person as a JSON document and lists the places that name the person (ADR 0073).
- The dispute note.
  When evidence must stay, an owner adds a note that the data is disputed.
  Each reader of the evidence sees the note, also in the AI clients of members.
- The rights to access, correction and deletion, and the right to receive the data in a common format, with a contact.

## Placeholders

The template has placeholders in square brackets.
The club and the operator replace each one before real personal data enters tada.

| Placeholder              | Who fills it | What to write                                                                          |
| ------------------------ | ------------ | -------------------------------------------------------------------------------------- |
| `[Name des Vereins]`     | Club         | The legal name of the club.                                                            |
| `[Adresse des Vereins]`  | Club         | The postal address of the club.                                                        |
| `[Dauer der Backups]`    | Operator     | How long the operator keeps each backup.                                               |
| `[Dauer der Protokolle]` | Operator     | How long the operator keeps the logs of the service (ADR 0035).                        |
| `[Kontakt für Anfragen]` | Club         | An address where members ask for access, correction, deletion or a copy of their data. |

The operator gives the club the values of the retention periods.
Keep operator details such as host names and providers out of the notice and out of this repository (ADR 0033).

## When the template changes

A change of the template changes the text that organizations without an own text show.
An organization with an own text keeps it.
Tell the owners of these organizations about a change that matters to them.
