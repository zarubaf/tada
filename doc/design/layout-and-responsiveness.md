# Layout and responsiveness

Under change by [ADR 0082](../adr/0082-redesign-of-the-web-client.md).

ADR 0023 defines the strategy. This document gives the layouts, the navigation and the page templates.

## App shell

The shell has a sidebar, a top bar and the page.
The viewport width selects the layout.
If the server runs another build than the web client, a Banner below the top bar asks the member to load the page again (ADR 0075).

### Top bar (all widths)

The top bar is 48 px high and sticks to the top of the page. It is the `banner` landmark.

- A toggle for the sidebar. It collapses the docked sidebar on medium and wide layouts and opens the drawer on narrow layouts.
- The title of the context in plain text, not a heading: the name of the current event on an event page, else the name of the section, for example „Eingang“.
  Each page keeps its own `h1`.
  A long name is cut and has the full name as its title.
- The command box „Suchen oder Befehl ausführen…“ with the key hint (`⌘K` or `Ctrl K`).
  It opens the command palette, as does `Ctrl+K` or `⌘K` from anywhere in the shell.
  Below 40rem it is an icon button.
- The theme toggle as an icon button, then the member menu as an avatar.
  The member menu has „Konto“, „Datenschutz“, „Versionen“ and „Abmelden“.

### Narrow (below 40rem)

```text
┌──────────────────────────┐
│ Top bar                  │
├──────────────────────────┤
│                          │
│ Page content             │
│ (one column)             │
│                          │
├──────────────────────────┤
│ Bottom bar (4 items)     │
└──────────────────────────┘
```

- The bottom bar has four items: „Eingang“, „Meine Arbeit“, „Anlässe“ and „Mehr“.
  Inside an event the third item is „Arbeit“ (ADR 0082), and „Anlässe“ moves into „Mehr“.
  „Mehr“ is a menu with „Personen“, „Institutionen“, „Einstellungen“, „Datenschutz“ and „Versionen“, and inside an event „Anlässe“ first.
- The bottom bar shows a count badge on „Eingang“ when proposals wait for the member.
- The toggle of the top bar opens the sidebar as a drawer from the left edge.
  The drawer holds the whole sidebar: the switchers, all destinations and the foot.
  It closes with `Esc`, with its close button, with a press outside and when a destination opens.
  Focus stays inside while it is open and returns to the toggle.
- The targets are 44 px high with a coarse pointer, and the page never scrolls sideways.
- The evidence panel and the filters open as sheets from the bottom, up to 90 % of the screen height.
- The page gutter is `--space-4`.

### Medium (40rem to 80rem)

```text
┌────────┬─────────────────────────────┐
│Sidebar │ Top bar                     │
│        ├─────────────────────────────┤
│        │                             │
│        │ Page content                │
│        │                             │
└────────┴─────────────────────────────┘
```

- The sidebar is 232 px wide, docked, and the full height of the window.
  The toggle of the top bar collapses it completely.
  The browser remembers the choice (`tada-sidebar`).
- The evidence panel opens as a sheet from the right, 400 px wide, over the content.
- The page gutter is `--space-6`.

### Wide (80rem and above)

```text
┌────────┬──────────────────────┬──────────┐
│Sidebar │ Top bar              │          │
│        ├──────────────────────┤ Evidence │
│        │ Page content         │ panel    │
│        │                      │ (docked) │
└────────┴──────────────────────┴──────────┘
```

- The evidence panel is a docked column, 360 px wide. A member can close it.
- The page gutter is `--space-8`.

## Navigation

The destinations are defined once in `apps/web/src/shell/navItems.ts`.
The sidebar, the bottom bar, „Mehr“ and the command palette read that list.
An item appears only when its page exists.

The sidebar is a region with the label „Seitenleiste“; its main navigation is the `nav` „Hauptnavigation“.
Sidebar, from top to bottom:

1. The app name, then the organization switcher (only if the member belongs to more than one organization).
2. The event switcher: the name and the key of the current event, and a chevron.
   The current event is the event of the route, else the event that the member opened last (the browser remembers it for each organization as `tada-event:<organization ID>`), else none.
   The menu lists the events of the member and has „Alle Anlässe“ and, for the owner and the administration, „Neuer Anlass“.
   A closed event shows „abgeschlossen“ in its subtitle.
3. The main navigation: „Eingang“ (with the count of open proposals), „Meine Arbeit“, „Arbeit“, „Zeitplan“, „Anlässe“, „Personen“ and „Institutionen“.
   „Arbeit“ and „Zeitplan“ appear only while the path names an event.
   „Arbeit“ opens the work page of that event. „Zeitplan“ opens the same page in the timeline view, `/events/:id/work?view=timeline`.
   On that page „Arbeit“ is the current item and „Anlässe“ is not: an item can yield a path to another item.
   In the timeline view „Zeitplan“ is the current item and „Arbeit“ is not: an item can also be current only for a value of the query (`onlyWith`, `unlessWith`).
   The bottom bar has no „Zeitplan“; on a phone the view tabs reach it.
4. At the foot: „Einstellungen“, „Datenschutz“, the build of the web client and the member block.
   The build, for example `tada 0.3.0 · 1a2b3c4`, opens the list of the components in a sheet.
   The member block shows the name and the role; the member menu is in the top bar.

The event page has its own sub-navigation: „Übersicht“, „Aufgaben“, „Zeitplan“, „Register“, „Dokumente“, „Mitglieder“ and „Arbeitsbereiche“ (until ADR 0082 replaces it).
It has a second row below „Register“: „Zusagen“, „Meilensteine“, „Entscheide“, „Risiken“ and „Anforderungen“.
Each tab has its own URL: `/events/:id/commitments`, `/milestones`, `/decisions`, `/risks` and `/requirements`.

Rules:

- The current item has the background `--color-accent-subtle` and the text `--color-accent-text`, and `aria-current="page"`.
- Each item has an icon and a label. No item is an icon alone.
- The skip link is the first focusable element and leads to `main`.
- Each page has a stable URL. A member can share a link to any record, filter or sheet.

## Page templates

Each screen uses one of these templates. A new template needs a change to this document.
Each template uses the `Page` component for the `main` element, the page gutters and the `h1` style.

| Template         | Structure                                                                                                   | Used by                                                          |
| ---------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| List with detail | A list on the left, the selected item on the right (wide); the list, then the item as its own page (narrow) | „Eingang“, „Meine Arbeit“                                        |
| Register         | A toolbar with filters and actions, then a table                                                            | Milestones, risks, requirements, decisions, commitments, actions |
| Overview         | A header with the key facts, then sections in one column                                                    | Event overview                                                   |
| Portfolio        | A filter bar, then a table of events with readiness columns                                                 | Portfolio                                                        |
| Form             | One column, at most 40rem wide, the actions at the bottom                                                   | Create and edit pages                                            |
| Public page      | No shell. The app name, one heading and a form column of at most 40rem                                      | Sign-in, magic link, invitation, organization choice             |
| Settings         | The sub-navigation of the settings, then the settings page                                                  | „Mitglieder“, „Telegram“ in the settings                         |
| Event page       | The event header with the key and the name, the sub-navigation of the event, then the sub-page              | „Übersicht“, „Mitglieder“, „Aufgaben“ of an event                |
| Timeline         | The event header, a filter bar, then lanes on a time axis (wide); a list grouped by week (narrow)           | „Zeitplan“ of an event                                           |
| Work             | One toolbar row (title, type filter, „Filter“, view switch), 12 px below it the view                        | „Arbeit“ of an event                                             |
| Documents        | A folder tree (wide) or a breadcrumb (narrow), then a file list                                             | „Dokumente“                                                      |

### Work

The „Arbeit“ page (`/events/:id/work`) shows all work records of an event: actions, commitments, milestones, decisions and risks (ADR 0082).
It has its own `h1` „Arbeit“ and not the header of the event page.
The page does not use the sub-navigation of the event.

- The page is `Page width="flush"`: it has no gutter of its own, and its rows set `--page-gutter` (16 px on a phone, 12 px from `--bp-medium`).
  The toolbar runs edge to edge; the view below it keeps the gutter at both sides and at the bottom.
- The toolbar is one row of `--size-toolbar` (44 px) with a border below and the gutter at both sides.
  From left to right it holds the `h1` „Arbeit“ as a compact heading (`--font-size-base`), the type filter, then at the right the view switch and the ghost button „Filter“.
  The `h1` stays the page heading for screen readers and for the focus moves; it only sits in the row.
  The row has a gap of 8 px.
  When it does not fit, for example next to a docked panel, it wraps into a second row with a gap of 6 px; the switch and the button stay together at the right, and the row never overflows.
- Below the row there are 12 px (8 px on a phone) between the banners, the empty states and the view.
  The columns of the board fill the screen: the page, the tabs and the view are one flex column, and the board takes the height that the toolbar and the banners leave, so the columns have one height and the page does not scroll for them, also where the row wraps.
- The type filter is „Alle“, „Aufgaben“, „Zusagen“, „Meilensteine“, „Entscheide“ and „Risiken“, each with a count.
  A count shows what the other filters let through, so it says what a click adds.
- The button „Filter“ and the key `/` (outside a text field) open a dialog with the search, and focus goes to the search.
  From `--bp-medium` the dialog is a popover that is not modal, so the board stays usable and shows the result of each change behind it; Escape closes it and focus returns to the button.
  On a phone it is a sheet from the bottom.
  It holds the search (title and ID), „Verantwortlich“ („Ich“ first, then the contributors; there is no „Niemand“, because an action needs an owner), „Bereich“, „Fällig“ („Überfällig“, „Diese Woche“, „Ohne Datum“) and „Filter zurücksetzen“, which shows only while a filter is set.
  The button shows the number of set filters (the type is not counted: it has its own control) and has the accessible name „Filter, 2 gesetzt“.
  The active filters are not repeated as chips: the count is enough, and the sheet shows the values.
- The view switch „Board“, „Tabelle“ and „Zeitplan“ is a segmented control at the right end of the row (a frame of 30 px with 24 px buttons) and chooses the view.
  The filters are the same in every view, and so are the counts and the announcements.
  A change of the tab makes a new history entry, so that back and forward move between the views; the filters and the sort stay.
  The table and the timeline are loaded when their tab opens, so the board does not pay for them.
- All state is in the query string: `type`, `owner`, `stream`, `due`, `q`, `view` (`board` is the default and has no key; the others are `table` and `timeline`), the sort keys `sort` and `dir` of the table, and the view keys `column` (the status of the list on a phone) and `canceled` (the open „Abgebrochen“).
  A change of a filter or of the sort replaces the history entry.
  The search writes `q` 300 ms after the last keystroke, and at once on Enter, on blur and when it is cleared.
  A reload or a shared link opens the same view.
- The board has the columns „Offen“, „In Arbeit“, „Blockiert“ and „Erledigt“.
  „Abgebrochen“ is collapsed to a rail with its count and opens with a toggle that keeps focus.
  The counts in the heads follow the filters.
  A column is 16rem to 22.5rem wide, with a subtle border; below the least width the board scrolls inside itself, never the page.
  There is no drag and drop.
- A card is one link to its record.
  An action opens in the record panel: the link goes to the same page with the readable ID as the hash, and it keeps the query, so the view and the filters stay.
  Until the panels of the other kinds exist, they lead to the old page of the kind, with the readable ID as the hash.
  A risk has no such page, so its card has the same look and is no link.
  A row of the table links in the same way.
- The record panel shows the record that the hash names (`#ACT-012`).
  From 80rem it is a docked column 30rem wide beside the list, under the top bar; the list stays usable and gets the rest of the width.
  From 40rem to 80rem it is an overlay from the right, over the list and the top bar.
  Below 40rem it is a full-screen sheet above the bottom bar, and the page behind it is inert, so it leaves the tab order, the reading order and the scroll.
  Its controls are 44 px high on a coarse pointer.
  `Escape` ends an inline edit first; a second `Escape` closes the panel.
  Focus moves into the panel on open and returns to the card or the row that opened it, else to the card or row of the record, else to the heading of the page.
  The „Neu“ button in the toolbar opens the create state of the same panel (`?new=action`); it is absent in a closed event and for a member who may not create.
- A kind that fails to load shows a banner with „Erneut versuchen“ for that kind; the other kinds still show.
  During a retry the board stays, the banner is busy and keeps focus; the skeleton is for the first load.
  The failures are announced once, in the polite live region of the page, not as alerts.
  An event that is unknown or forbidden shows its error with a retry instead of the page.
  An event without records shows an empty state with „Neue Aufgabe“.
  Filters that hide all records show an empty state with „Filter zurücksetzen“.
  After a reset in the sheet, focus moves to the search field; after a reset from the empty state, to the `h1`.
- When the filters change, a polite live region announces the number of records, for example „12 Einträge“.
- Narrow (below 40rem): the board is one list.
  A status picker (a select with the counts) replaces the columns.
  The first row holds the `h1` and the view switch.
  The second row holds the type filter as a select and the „Filter“ button, and the status picker follows directly below it.
  The button shows the number of filters that are set.
  The page does not scroll sideways, and the targets are 44 px.
- The bulk bar floats 20 px above the bottom of the window while the board or the table shows, 44 px high with 30 px buttons (`--size-bulk-button`; 44 px with a coarse pointer), grouped by rules of 20 px.
  The dock has no height, so the bar takes no room of its own; while it shows, the view keeps `--dock-room` free at its end (the bar and its distance from the bottom), so that the bar never covers the last row or card of the board, the table or the list of a phone.
  Below 40rem it sits above the bottom bar, in two rows: the count and „Auswahl aufheben“, then the three buttons, which wrap and never scroll sideways.
  The value lists open as a sheet from the bottom instead of a popover.
  A failure banner sits above the bar in the same dock.

### Table

The view `?view=table` shows one dense row for each record, from 40rem.
The columns are „ID“, „Typ“ (the icon, with the name of the kind as its accessible name), „Titel“, „Status“ (icon and word), „Verantwortlich“ (avatar and name), „Fällig“ (the date chip) and „Bereich“.
A row is `--row-height` high (28 px, and 44 px on coarse pointers).

- The header of each column is a button that sorts.
  The query keys are `sort` (`id`, `kind`, `title`, `status`, `owner`, `due`, `area`) and `dir` (`asc`, `desc`); the defaults `due` and `asc` have no key.
  The default order is by the date of the column: the due date, and for a decision the day it was made; the records without a date last, then by ID.
  The board orders by due date alone, so a decision stays at the end of its column there.
  Pressing the sorted column flips its direction; pressing another column sorts it ascending.
  The sorted header has `aria-sort`, and a polite live region says „Sortiert nach Titel, aufsteigend“.
  The sort is stable. A record without a value (an empty title, no owner, no date, no area) comes last in both directions, and a tie is broken by the readable ID.
- The title is a link with the same target as the card: the old page of the kind with the readable ID as the hash.
  A risk has no page, so its title is text.
  A redacted record shows „entfernt“.
- The title takes the width that is left and ends in an ellipsis in one line; the `title` attribute holds the whole text. Owner and area end in an ellipsis too.
- Where its container is less than 40rem wide (a narrow viewport, a docked panel or a column to select), the table keeps its columns and scrolls inside itself, never the page.
  The scrolling region takes focus, so that the keyboard reaches the columns.
- The states are those of the board: the skeleton, the banner of a kind that failed, and the two empty states.
- Below 40rem there is no table.
  The page shows the cards of the board as one list, in the order of the sort.
  The list has no sort control; the targets are 44 px.
- The markup has one cell for each column, so that a column with a checkbox can join in front.
- Selection: a member who may change a record sees the checkbox column in front, 28 px high (44 px on coarse pointers, as the row).
  „Alle auswählen“ sits in its header cell and selects all records of the list that can be changed.
  One selection serves the board and the table; a switch between them keeps the records that the new view shows (the table shows every record, the board hides a collapsed „Abgebrochen“).
  The timeline has no selection: a switch to it clears the selection, and the bulk bar is not shown.
  The exception is a bulk change that runs, or whose failures are still open: it keeps its records selected through the timeline, and the failures wait in the bar for the return to the table or the board.
  Below 40rem the list of cards selects as the board does, with „Alle auswählen“ above it.

### Timeline

The „Zeitplan“ view of the work page shows the plan of one event: `/events/:id/work?view=timeline`.
The old page `/events/:id/timeline` shows the same plan until the redirect of ADR 0082 replaces it; both use `TimelinePlan`, which takes the records as props.
It is read-only.
It has no drag.

- The view shows the actions and milestones that the filters of the page let through.
  Commitments, decisions and risks are not on the timeline.
  If the type filter names only such kinds, the view says „Diese Ansicht zeigt Aufgaben und Meilensteine.“ and offers „Filter zurücksetzen“.
- The event window comes from the facts of the event.
  The view loads them when it opens, and it shows a skeleton until they are there, so that no empty state flashes.
- One lane for each workstream, and one lane „ohne Arbeitsbereich“.
- An action with a start date and a due date is a bar.
  An action with only a due date is a point.
  Actions without a due date appear as a count with a link to the action register.
- A milestone is a marker with a vertical line across all lanes.
- The event window is a band behind the lanes.
  The window comes from the core facts `exact_dates` or `date_window` (ADR 0081).
  If the window is unknown, the page shows no band.
- A line marks today („heute“).
- The axis shows weeks.
  It runs from the earliest to the latest date of the actions (start and due dates), the milestones and the event window.
  A long span scrolls inside the timeline. The page itself never scrolls horizontally.
- Filters: owner and status.
  Each bar and point links to its action.
- Bars, points and markers do not rely on color alone.
  Each has a label, and the status shows as text in the list.
- Narrow layouts and screen readers get a list of the same items, grouped by week.
- An event with no dated actions, no milestones and no event window shows the empty state of the page, not an empty axis.
  An event with milestones but no dated actions shows its milestones, and the band if the window is known.

## Widths

| Content                               | Maximum width          |
| ------------------------------------- | ---------------------- |
| Prose (descriptions, minutes, drafts) | 70ch                   |
| Forms                                 | 40rem                  |
| Tables and lists                      | the full content width |
| Dialogs                               | 32rem                  |
| Sheets on medium and wide layouts     | 400 px (right sheet)   |

## Container queries

Components adapt to their container. Each component that changes its layout declares a container.

| Container width | Typical change                                                                |
| --------------- | ----------------------------------------------------------------------------- |
| below 28rem     | Table rows become two-line list rows. Toolbars collapse into one menu.        |
| 28rem to 56rem  | Tables show the primary columns only. Other columns move into the row detail. |
| 56rem and above | All columns.                                                                  |

## Mobile rules

- Use `100dvh` and `env(safe-area-inset-*)`. Do not use `100vh`.
- The primary action of a mobile page is at the bottom, within reach of the thumb.
- Inputs use `--font-size-base` (16 px) on touch devices, so that iOS does not zoom.
- Inputs set the correct `inputmode` and `autocomplete` attributes.
- Swipe gestures are never the only way to do an action.

## Review widths

Check each new screen at these widths:

- 320 px (WCAG reflow)
- 375 px (common phone)
- 1024 px (small laptop, medium layout)
- 1440 px (desktop, wide layout)
