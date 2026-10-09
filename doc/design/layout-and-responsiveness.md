# Layout and responsiveness

ADR 0023 defines the strategy. This document gives the layouts, the navigation and the page templates.

## App shell

The app shell has three layouts. The viewport width selects the layout.

### Narrow (below 40rem)

```text
┌──────────────────────────┐
│ Top bar: title · search  │
├──────────────────────────┤
│                          │
│ Page content             │
│ (one column)             │
│                          │
├──────────────────────────┤
│ Bottom bar (4 items)     │
└──────────────────────────┘
```

- The bottom bar has four items: „Meine Arbeit“, „Eingang“, „Anlässe“ and „Einstellungen“.
- The member menu holds „Personen“ and „Institutionen“, so the narrow layout reaches them too.
- The bottom bar shows a count badge on „Eingang“ when proposals wait for the member.
- The evidence panel and the filters open as sheets from the bottom, up to 90 % of the screen height.
- The page gutter is `--space-4`.

### Medium (40rem to 80rem)

```text
┌────────┬─────────────────────────────┐
│Sidebar │ Top bar: breadcrumb · search│
│        ├─────────────────────────────┤
│        │                             │
│        │ Page content                │
│        │                             │
└────────┴─────────────────────────────┘
```

- The sidebar is 240 px wide. A member can collapse it to 56 px (icons with tooltips).
- The evidence panel opens as a sheet from the right, 400 px wide, over the content.
- The page gutter is `--space-6`.

### Wide (80rem and above)

```text
┌────────┬──────────────────────┬──────────┐
│Sidebar │ Page content         │ Evidence │
│        │                      │ panel    │
│        │                      │ (docked) │
└────────┴──────────────────────┴──────────┘
```

- The evidence panel is a docked column, 360 px wide. A member can close it.
- The page gutter is `--space-8`.

## Navigation

Sidebar, from top to bottom:

1. Organization switcher (only if the member belongs to more than one organization).
2. Search and command menu (`Ctrl+K` or `⌘K`).
3. „Portfolio“, „Meine Arbeit“ and „Eingang“.
4. „Anlässe“: the events of the member, with the current event expanded:
   „Übersicht“, „Aufgaben“, „Register“, „Dokumente“ and „Personen“.
5. At the bottom: „Einstellungen“ and the member menu.

Rules:

- The current item has the background `--color-accent-subtle` and the text `--color-accent-text`, and `aria-current="page"`.
- Each page has a breadcrumb on medium and wide layouts. On narrow layouts, the top bar has a back button.
- Each page has a stable URL. A member can share a link to any record, filter or sheet.

## Page templates

Each screen uses one of these templates. A new template needs a change to this document.
Each template uses the `Page` component for the `main` element, the page gutters and the `h1` style.

| Template         | Structure                                                                                                   | Used by                                              |
| ---------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| List with detail | A list on the left, the selected item on the right (wide); the list, then the item as its own page (narrow) | „Eingang“, „Meine Arbeit“                            |
| Register         | A toolbar with filters and actions, then a table                                                            | Risks, requirements, decisions, commitments, actions |
| Overview         | A header with the key facts, then sections in one column                                                    | Event overview                                       |
| Portfolio        | A filter bar, then a table of events with readiness columns                                                 | Portfolio                                            |
| Form             | One column, at most 40rem wide, the actions at the bottom                                                   | Create and edit pages                                |
| Public page      | No shell. The app name, one heading and a form column of at most 40rem                                      | Sign-in, magic link, invitation, organization choice |
| Settings         | The sub-navigation of the settings, then the settings page                                                  | „Mitglieder“, „Telegram“ in the settings             |
| Event page       | The event header with the key and the name, the sub-navigation of the event, then the sub-page              | „Übersicht“, „Mitglieder“, „Aufgaben“ of an event    |
| Documents        | A folder tree (wide) or a breadcrumb (narrow), then a file list                                             | „Dokumente“                                          |

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
