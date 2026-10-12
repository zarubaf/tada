# 0079. Changesets without member words

- Status: Accepted
- Date: 2026-10-12
- Amends: [0040](0040-ai-intake-through-mcp.md)

## Context

[ADR 0040](0040-ai-intake-through-mcp.md) requires the member's own words as `source_text` in each proposal call.
tada stores the text as a source version, and the passages of the proposals point into it.
Its amendment of 2026-10-08 lets a passage also cite another source version that is readable in the event of the proposal.
An agent can propose changes that rest only on stored sources, for example a draft from an earlier intake or an uploaded text file.
The member then said nothing new, but the agent must still send a `source_text`.
The agent then writes a text in the name of the member, and the source version claims words that the member never said.
This weakens the provenance that the rule protects.

## Decision

We allow a changeset without a source text.

- `source_text` of `NewChangeset` is optional. The default is an empty text.
- A text that is empty after the normalization ([ADR 0060](0060-unicode-normalization-of-source-texts.md)) or that contains only white space counts as absent.
- Without a source text, tada stores no source version for the changeset.
  The column `changeset.source_version_id` is NULL, and the API leaves out `source_version_id`.
- Without a source text, each passage names a `source_version_id`.
  The source version must be readable in the event of the proposal (`app::access::event_source_reach`), as before.
  A passage without a `source_version_id` gets the field error `no-source-text`.
- The rule "each proposal has at least one passage" stays. A changeset without a source text still has evidence for each proposal.
- The agent writes its own explanation into the `reason` of each proposal. There is no other field for the words of the agent.
- The Review Inbox states that the changeset contains no words of the member.

The API contract changes: `source_version_id` becomes optional in the responses of `GET /api/v1/changesets/{changeset_id}` and `POST /api/v1/events/{event_id}/changesets`.
[ADR 0017](0017-api-contract-rust.md) counts this as a breaking change.
We keep the prefix `/api/v1`, and the product owner accepts this exception.
tada is before version 1.0.
The web client is the only client of these routes. It ships in the same image as the server, and it handles an absent field.
`contracts/oasdiff-err-ignore.txt` lists the changes until `main` contains them.

## Consequences

- An agent never has to write words in the name of the member.
- Each stored member text is again a text that a member wrote, sent or confirmed.
- Each reader of `changeset.source_version_id` must handle NULL: the review read, the retry comparison, the check of cited sources and the insert.
  The export ([ADR 0059](0059-structured-export.md)) writes the NULL value as it is.
  Redaction ([ADR 0072](0072-legal-redaction-and-deletion-journal.md)), retention ([ADR 0071](0071-event-closing-and-retention.md)), the person export and search read source versions, not the changeset column, so they need no change.
- The migration only removes the constraint `NOT NULL`. A release before this change cannot read a changeset without a source version.
  The gap lasts only until all roles run the new release. We accept it.
- Telegram always sends the text of the member, so it always stores a source version.

## Alternatives

- Keep the rule, and let the agent write a short text: the stored source version then claims words that the member never said.
- A second field `agent_note` for the words of the agent: two fields for the words of the agent, next to the `reason` of each proposal.
- Store an empty source version: a source version without content, which the review would show as an empty quote.
- A new version prefix `/api/v2` for the contract change: a second set of routes before version 1.0, for a client that ships with the server.
