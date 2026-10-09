# Components and patterns

Screens use only the components in `apps/web/src/ui/` (ADR 0020).
This document lists the core components and the patterns for the main screens.
A new component needs an entry here in the same pull request.
The gallery (`apps/web/src/gallery/`) is a development tool that shows the components, does not ship, and may keep German literals outside Fluent.

## Core components

### Actions

| Component    | Variants                          | Rules                                                                                                                |
| ------------ | --------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Button       | primary, secondary, quiet, danger | At most one primary button in a view. Labels are a verb and an object. The label does not wrap.                      |
| IconButton   | quiet                             | Only in toolbars and rows. Always an accessible name and a tooltip.                                                  |
| Link         | inline, standalone                | Inline links are underlined. A link never does what a button does.                                                   |
| LinkButton   | primary                           | A link in the style of a button. Only for an action that opens a page, for example a form.                           |
| FileButton   | primary                           | A button that opens the file picker. It shows `isPending` while the upload runs and keeps focus.                     |
| FileLink     | secondary                         | A plain link to a file on the server, in the style of a button, for a download or a preview. The browser loads it.   |
| Menu         |                                   | For more than three secondary actions, or the member menu. The trigger is `dots-vertical` or a text.                 |
| ChoiceButton |                                   | A full-width button for one option of a choice, with a title and a detail line. For example the organization choice. |
| CommandMenu  |                                   | Global search and actions, `Ctrl+K`. Shows the shortcut next to each action.                                         |

Button sizes follow `--control-height-*` of the density. The primary button uses `--color-accent` and `--color-text-on-accent`.
The danger button is secondary in style with `--color-danger` text.
Row actions that remove or revoke something use the danger button.
The confirm button of a destructive confirmation also uses the danger button.

### Input

| Component                   | Rules                                                                                    |
| --------------------------- | ---------------------------------------------------------------------------------------- |
| TextField, TextArea         | Label above, help below, error below the help.                                           |
| Select                      | For up to 7 options. More options use ComboBox.                                          |
| ComboBox                    | Search in the options. Used for people, events and records.                              |
| DatePicker, DateRangePicker | `de-CH` format; keyboard input and calendar. Shows the weekday.                          |
| Checkbox, CheckboxGroup     | Label to the right of the box.                                                           |
| RadioGroup                  | For two to five exclusive options that a member must see together.                       |
| Switch                      | Only for a setting that has an immediate effect. Not inside forms with a submit button.  |
| FileDrop                    | Drag and drop with a visible „Datei wählen“ button. Shows size limits before the upload. |

### Display

| Component      | Rules                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| KnowledgeState | Shows the state of knowledge of a value (see [tokens.md](tokens.md)): the value, an icon and the label. An accepted value hides the label from the eye; `showLabel` shows it. Used in each place where a fact appears. Values come from `formatValue`.                                                                                                                                                                                                                                                                          |
| StatusLabel    | Icon and text for a workflow status: „offen“, „in Arbeit“, „blockiert“ and „erledigt“.                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Badge          | A count, for example waiting proposals. Never decorative.                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Avatar         | Initials on `--color-bg-sunken`; a photo only if the member uploads one.                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| RecordId       | `ACT-042` in `--font-family-mono`, with a copy action.                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| RelativeTime   | „vor 2 Stunden“ with the exact time in a tooltip.                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Kbd            | A keyboard key in hints and the shortcut list.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Skeleton       | Gray blocks in the shape of the content. No shimmer animation with reduced motion.                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| DataTable      | A read-only table with a caption, a header that sticks to the page, `--row-height` rows, and monospace or tabular columns where needed. Below 28rem of container width, each row is a two-line list row.                                                                                                                                                                                                                                                                                                                        |
| LiveRegion     | A polite `status` or an assertive `alert` that is in the page before its text. Each screen has one of each; see [accessibility.md](accessibility.md).                                                                                                                                                                                                                                                                                                                                                                           |
| Markdown       | Safe Markdown for text that members or agents write (ADR 0058): CommonMark with tables. Raw HTML is dropped, an image shows its alternative text only, and a link is `https` or `mailto` and opens in a new tab with `rel="noopener noreferrer"`. A `tada:` link is no address. Headings start at level 2 by default, because the page has the h1. A page that places the text deeper in its outline passes `headingLevel` (the level of `#`, at most h6), so that the headings of the text nest under the heading of the page. |

Focus moves are not a component. The hooks `useFocusAfterCommit` and `useRetry` in `apps/web/src/ui/focus.ts` are the only way to move focus after an action.
See [accessibility.md](accessibility.md#focus).

### Navigation

| Component | Rules                                                                                                                             |
| --------- | --------------------------------------------------------------------------------------------------------------------------------- |
| NavLink   | A navigation item. The current page has `aria-current="page"` and the current style. `large` gives the bottom bar a thumb target. |
| SubNav    | The sub-navigation of a template, for example of the settings or of an event. It contains NavLink items.                          |
| SkipLink  | „Zum Inhalt springen“, the first focusable element of each page. It moves to `main`.                                              |

### Page

| Component | Rules                                                                                                               |
| --------- | ------------------------------------------------------------------------------------------------------------------- |
| Page      | The `main` element of each template, with the page gutters. `width="form"` is the narrow column of the public page. |
| PageTitle | The `h1` of a page, in `--font-size-2xl`. Focus moves to it after a route change.                                   |

### Containers

| Component   | Rules                                                                                                                                           |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Panel       | A bordered area in the page flow. No shadow.                                                                                                    |
| Sheet       | Opens from the right (medium, wide) or from the bottom (narrow). For details, filters and the evidence panel.                                   |
| Dialog      | Only for a destructive confirmation or a blocking task. Title, one sentence, two buttons. `ConfirmDialog` is the confirmation.                  |
| Popover     | For small forms and pickers that belong to one control.                                                                                         |
| Tooltip     | Text only, no interactive content. Opens on hover and on focus.                                                                                 |
| Tabs        | For two to six views of one record. Not for navigation between pages.                                                                           |
| Banner      | A message for a whole page or the app, for example „Synchronisation seit 3 Stunden unterbrochen“. Cannot be hidden while the problem exists.    |
| Toast       | Confirms the last action of the member, with an „Rückgängig“ action if possible. Never for errors that need an action.                          |
| EmptyState  | A title, one sentence and one action, if the member can act there. No illustration.                                                             |
| InlineError | A failed request in the area that failed: the message, the request ID and „Erneut versuchen“. `announce` sets how a screen reader learns of it. |

## Patterns for the main screens

### Portfolio (PM, desktop first)

- A table with one row for each event: name, dates, readiness, open exceptions, next milestone, lead.
- Readiness is a StatusLabel with a text, not a percentage ring.
- Exceptions (overdue, unowned, conflict, stale sync) appear as counts that link to the filtered register.
- The default sort is by the next milestone.
- Narrow layout: two-line rows; the exceptions move to the second line.

### Event overview

- A header with the event name, dates, place, the lead and the readiness. Each value has its KnowledgeState.
- Sections in this order: „Offene Fragen“, „Ausnahmen“, „nächste Meilensteine“, „Workstreams“ and „zuletzt geändert“.
- Unknown values show „Unbekannt“, never an empty space.
- „Was ist noch unbekannt?“ opens the Ask Event panel with this question.

### My Work (all members, mobile first)

- A list grouped by due date: „überfällig“, „heute“, „diese Woche“ and „später“.
- Each row: the RecordId, the title, the event, the due date and the StatusLabel.
- The row actions on narrow layouts: „Erledigt“, „Blockiert“, „Neues Datum vorschlagen“. These match the Telegram buttons.
- Items from several events appear in one list. The event name is visible in each row.

### Registers and tables

- A toolbar with: the search field (`/`), filters as chips, the view options and one primary action („Risiko erfassen“).
- The table has a sticky header, tabular numbers and a row height of `--row-height`.
- A click on a row opens the record in a sheet on medium and wide layouts, and as a page on narrow layouts.
- Bulk actions appear in the toolbar only when rows are selected.
- A filter that hides rows shows the count of hidden rows and a "Filter zurücksetzen" action.

### Review Inbox („Eingang“)

- A list with detail. The list shows proposals that wait for the member, oldest first.
- The detail shows, from top to bottom:
  1. The proposed change as a comparison: the current accepted value and the proposed value.
  2. The source: the excerpt with the cited passage marked, the source version and the capture time.
  3. Conditions and assumptions of the proposal.
  4. The actions: „Annehmen“, „Bearbeiten und annehmen“, „Ablehnen“.
- If the target record changed after the proposal, the detail shows a conflict and disables „Annehmen“ with the reason.
- Keyboard: `J` and `K` move through the list; `A` accepts, `E` edits and `R` rejects, each with a visible hint.
- Batch review: a member can select several proposals of the same kind and accept them together after a summary.

### Evidence panel

- Shows the provenance of the selected value: the evidence links, the source versions, who accepted the value and when.
- Each source shows its freshness: „erfasst am 03.10.2026, 14:12“.
- A source that the member cannot see shows „Quelle nicht freigegeben“, never its content.
- Proposals and accepted values have separate sections, so the member never mixes them up.

### Forms

- One column, at most 40rem wide.
- Group related fields under a heading. No more than seven fields in a group.
- The actions are at the bottom: the primary action on the right on wide layouts, full width on narrow layouts.
- Leaving a form with unsaved changes asks for a confirmation. The web client does not do this yet.

### Empty, loading and error states

| State                   | Pattern                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Empty, first use        | EmptyState with one sentence about the purpose and the first action: „Noch keine Risiken erfasst. Risiko erfassen“. |
| Empty, after a filter   | „Keine Treffer für diese Filter.“ and "Filter zurücksetzen".                                                        |
| Loading, first load     | Skeleton rows in the shape of the content.                                                                          |
| Loading, refresh        | The old content stays; a small progress indicator appears in the toolbar.                                           |
| Error, a request failed | Inline message in the area that failed, with „Erneut versuchen“. The rest of the page stays usable.                 |
| Error, no access        | „Sie haben keinen Zugriff auf diesen Bereich.“ and the name of a person who can grant it.                           |
| Stale data              | A Banner with the time of the last successful sync. tada never shows stale data as current.                         |
