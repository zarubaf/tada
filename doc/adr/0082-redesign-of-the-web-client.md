# 0082. Redesign of the web client

- Status: Accepted
- Date: 2026-10-14
- Amends: [0018](0018-design-system-foundation.md), [0020](0020-component-primitives.md), [0021](0021-typography-and-icons.md), [0022](0022-accessibility.md), [0023](0023-responsive-layout.md)

## Context

Slice 2d adds milestones, decisions and risks to actions and commitments.
The web client now has three registers, an actions page and a form below each list.
The product owner finds the client ugly and its structure confusing.
The audit of the current client found these problems:

- The same work is in four places: the actions page and three registers.
  A member must know the kind of a record to find it.
- A form sits below each list, so the member scrolls to create or change a record.
- Six pages use six different list patterns, and a record opens on another page.
- The words are internal: „Register“ is a term of the data model, not of a club.
- The look is calm but sparse.
  A committee that follows 50 records cannot see many of them at once.

On 2026-10-14, the product owner approved a mock of a dense client with a dark and a light theme.
The mock is a visual reference of density and structure, as is the public planning app [itsaplan](https://github.com/croffasia/itsaplan).
This ADR records the decisions and their reasons.

## Decision

### Structure

- The sidebar has these items: „Eingang“ (with a count of open proposals), „Meine Arbeit“, „Arbeit“, „Zeitplan“, „Dokumente“ and „Kontakte“.
  „Einstellungen“ and the member menu sit at the foot.
  An item appears only when its page exists.
- The sidebar head holds the organization and event switchers.
  The sidebar can collapse.
- The top bar has the page title, a command box (`Ctrl+K`), a „Neu“ button, the theme toggle and the member menu.
- „Arbeit“ is one page for all work records of an event: actions, commitments, milestones, decisions and risks.
  - A type filter selects all kinds or one kind.
  - Three view tabs select the view: „Board“, „Tabelle“ and „Zeitplan“.
    The route is `/events/:id/work`.
    The query string keeps the view and the filters, for example `?view=timeline`.
    The „Zeitplan“ item in the sidebar opens the same route with the timeline view.
  - The workstream is a filter („Bereich“), not a page.
    A manage dialog in the filter menu replaces the workstream page.
- A record opens in a side panel.
  The URL hash names the record, for example `#ACT-012`, as it does today.
  The record page, the register pages and the forms below lists go away.
- [ADR 0081](0081-milestones-decisions-risks-and-requirements.md) adds the kinds and a timeline page.
  This ADR turns that timeline page into a view of the „Arbeit“ page and keeps the event overview lean.
  Requirements („Anforderungen“: open, met, waived) join „Arbeit“ when they ship.
  The type filter then gets this kind.
- Merges:
  - „Kontakte“ is on the organization level.
    It has the tabs „Personen“ and „Institutionen“.
  - The members of an event move to the event home „Übersicht“.
  - „Einstellungen“ is one page with tabs.
- The event overview stays as a lean „Übersicht“ with open questions, facts and the members of the event.
  The event switcher reaches it.
  The event header and `CloseEvent` move there.
- The old event URLs keep working through permanent redirects.
  They are `/events/:id/actions`, `/commitments`, `/milestones`, `/decisions`, `/workstreams`, `/members` and `/timeline`, and the hash IDs.
  `/events/:id/timeline` goes to the „Zeitplan“ view of `/events/:id/work`.
- `/persons`, `/institutions` and `/settings/*` are organization routes, not event routes.
  `/persons` and `/institutions` become the tabs of „Kontakte“, and each old URL redirects to its tab.
  The `/settings/*` routes become the tabs of „Einstellungen“, and each old URL redirects to its tab.

### Look

- The look is dense.
  It has a dark and a light theme.
  The theme toggle in the top bar cycles through system, light and dark.
  The default follows the system ([ADR 0018](0018-design-system-foundation.md)), and the member's choice is stored.
- The accent is one blue (hue 262).
  It replaces the petrol accent.
  It marks primary actions, selection, links and focus only.
- Mona Sans and Tabler Icons stay ([ADR 0021](0021-typography-and-icons.md)).
- Status shows with an icon and a label, never with color alone.
  The icons are: open (dashed circle), in progress (half-filled circle), blocked (circle with a bar), done (circle with a check) and canceled (circle with a cross).
- The state of knowledge keeps its vocabulary ([ADR 0018](0018-design-system-foundation.md)).
  A proposal gets a dashed border and a distinct hue, and an overdue date gets a danger chip.
- Cards and rows show a type icon, the ID, the title, and chips for the status, the due date, the workstream and the owner.
- Shadows stay on overlays only.
  There are no gradients, no glow and no blur.
- The values live in `tokens.css`.
  The mock names map onto the existing `--color-*` and `--space-*` names.
  There is no parallel token set.

### Board columns

A card is in the column of its status group.
Each card shows its own real status chip, because a column can hold several statuses.

| Column                    | Records                                                                                                          |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| „Offen“                   | Action `open`. Commitment `conditional` or `firm`. Milestone `open`. Decision `in-force`. Risk `open`.           |
| „In Arbeit“               | Action `in-progress`.                                                                                            |
| „Blockiert“               | Action `blocked`. Risk `occurred`.                                                                               |
| „Erledigt“                | Action `done`. Commitment `fulfilled`. Milestone `reached`. Risk `mitigated` or `closed`.                        |
| „Abgebrochen“ (collapsed) | Action `canceled`. Commitment `broken` or `withdrawn`. Milestone `canceled`. Decision `superseded` or `revoked`. |

- The counts in the column heads follow the current filters.
- The bulk action „Status“ is enabled only when all selected records have one kind.
  It offers the intersection of their allowed next statuses.
- A kind that fails to load shows a partial-error banner with a retry button.
  The other kinds still show.
- The board has no drag and drop.
  The member changes a status in the panel or in a menu on the card.

### Amendments

- [ADR 0018](0018-design-system-foundation.md) (type, accent, density):
  - The sentence „Density changes spacing and row height, never the font size of body text below 14 px“ is replaced.
  - In compact density, body text and panel text are 14 px, and dense rows and cards are 13 px.
  - In comfortable density, body text and inputs stay at 16 px.
    `tokens.css` sets 16 px on coarse pointers, because iOS zooms into inputs below 16 px.
  - Chips and meta text are at least 12 px.
    No text is smaller.
  - The density tokens set the row heights and gaps.
    The two densities stay.
  - The accent is blue (hue 262), not petrol.
- [ADR 0020](0020-component-primitives.md) (components):
  - The side panel is non-modal.
    On wide layouts it is docked beside the list, and the list stays usable.
  - The view tabs and the type filter are tabs and toggle buttons.
  - We build these on React Aria Components where RAC has the piece (`Tabs`, `ComboBox`, `Toolbar`).
  - The docked panel is the exception, because RAC has no docked, non-modal region.
    It is a plain landmark region in `apps/web/src/ui/` with its own focus rules.
    Focus moves into the panel on open, and `Esc` closes it.
    Focus returns to the element that opened the panel, else to the list.
- [ADR 0021](0021-typography-and-icons.md) (type and icons):
  - IDs stay in the monospace face.
  - Dense numbers, dates and counts use tabular figures.
  - Icons have the sizes 12, 14, 16 and 20 px.
    Chip icons are 12 px, and icons in dense rows are 14 px.
- [ADR 0022](0022-accessibility.md) (accessibility):
  - Targets are 44 × 44 px on coarse pointers, also in the dense views.
  - Checkboxes for selection are always visible on touch devices.
    On fine pointers they show on hover and on focus within the card.
    They are also always visible while a selection exists.
  - The new color pairs meet the contrast rules of the ADR in both themes, and tests check them.
- [ADR 0023](0023-responsive-layout.md) (layout):
  - The shell has the sidebar and the top bar described above.
    The bottom bar on phones has four items: „Eingang“, „Meine Arbeit“, „Arbeit“ and „Mehr“.
  - The side panel is docked on wide layouts (1280 px and above).
  - On medium layouts (640 to 1280 px, including the 1024 px review width), it is an overlay sheet from the right.
  - On narrow layouts it is a full-screen sheet.
  - The panel replaces the evidence-panel column of the ADR. The evidence of a record shows inside the side panel.
  - On narrow layouts the board becomes one column list with a status picker.
    It does not scroll sideways.
  - The table view becomes a list of rows on narrow layouts, as the ADR already requires.

### Plain words

The client uses words that a club member knows:

- „Vorschlag“ for a proposal, and „Quelle“ for the source of a value.
- „Bereich“ for a workstream.
- No „Register“.
  The record types have their own names: „Aufgaben“, „Zusagen“, „Meilensteine“, „Entscheide“ and „Risiken“.

The glossary and the Fluent catalog follow in the pull request that changes the words.

### Out of scope

The redesign shows only data that the server has today.
It does not add:

- A history („Verlauf“) of a record.
- The sender of a source and a forwarded badge.
- A created and updated line.
- Drag and drop.
- The „Ablegen“ button.
  It comes with the capture feature.

The panel shows the existing evidence of a record through the evidence panel.
A proposal chip on a card means that a proposal in the member's „Eingang“ targets this record.

## Consequences

- A member finds all work of an event in one place, with one list pattern in three views.
- Create and change happen in the panel, so the lists lose their forms.
- The change is large.
  We deliver it in small steps and keep the app working after each step.
  Each step updates its part of `doc/design/` with its code.
  Until then the documents `tokens.md`, `components.md` and `layout-and-responsiveness.md` carry a note.
- The hash and the old URLs keep external links alive.
  The public URL of a record points to the „Arbeit“ page.
- The dense look raises the risk of small targets and low contrast.
  The checks of [ADR 0024](0024-frontend-quality-gates.md) and the screenshots guard against it.
- The Slice 2g views ADR will overlap with this one.
  The „Arbeit“ page delivers a board, a table and a timeline of the work records now, with fixed filters and no saved views.
  The Slice 2g ADR must start from this page.
  It adds saved views, custom fields and the resource view.
  It must not build a second set of views.
- The initial JavaScript bundle has little headroom.
  New code loads lazily.

## Alternatives

- Restyle the current pages only.
  We rejected it, because the confusing structure is the larger problem.
- A separate board page next to the registers.
  We rejected it, because it keeps four places for the same work.
- A modal dialog for a record.
  We rejected it, because the member cannot compare the record with the list.
- A new font and a new icon set.
  We rejected it, because the type and the icons were not the problem, and ADR 0021 holds.
- A column for each status.
  We rejected it, because five kinds have 20 statuses, and the board would be too wide.
