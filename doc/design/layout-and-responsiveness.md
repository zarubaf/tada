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
  „Mehr“ is a menu with „Personen“, „Institutionen“, „Einstellungen“, „Datenschutz“ and „Versionen“.
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
3. The main navigation: „Eingang“ (with the count of open proposals), „Meine Arbeit“, „Anlässe“, „Personen“ and „Institutionen“.
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
| Documents        | A folder tree (wide) or a breadcrumb (narrow), then a file list                                             | „Dokumente“                                                      |

### Timeline

The „Zeitplan“ page shows the plan of one event.
It is read-only.
It has no drag.

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
