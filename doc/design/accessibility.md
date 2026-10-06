# Accessibility

ADR 0022 sets WCAG 2.2 AA as the minimum. This document gives the rules for each part of the UI and the test procedure.

## Structure

- Each page has one `h1`. Headings do not skip levels.
- Each page uses the landmarks `header`, `nav`, `main` and, if present, `aside` for the evidence panel.
- The first focusable element is a „Zum Inhalt springen“ link to `main`.
- The page title (`<title>`) names the page and the event: „Risiken · Sommerfest 2027 · tada“.

## Focus

- The focus ring is a 2 px outline in `--color-focus` with a 2 px offset, on `:focus-visible`.
- Focus is never hidden behind a sticky header or the bottom bar (WCAG 2.4.11). Use `scroll-padding`.
- After a route change, focus moves to the `h1` of the new page.
- When a dialog or a sheet opens, focus moves to its first field or its title. When it closes, focus returns to the element that opened it.
- When a member deletes or moves an item from a list, focus moves to the next item, or to the list if it is empty.

## Keyboard

| Key              | Action                                       |
| ---------------- | -------------------------------------------- |
| `Ctrl+K` or `⌘K` | Open the command menu                        |
| `?`              | Show all keyboard shortcuts                  |
| `/`              | Focus the search or filter field of the page |
| `J` and `K`      | Next and previous item in a list             |
| `Enter`          | Open the selected item                       |
| `Esc`            | Close the menu, sheet or dialog              |

- Single-character shortcuts work only when no text field has focus. A member can turn them off in the settings (WCAG 2.1.4).
- Lists and tables use one tab stop and arrow keys inside (roving focus, as React Aria Components provides).

## Forms

- Each field has a visible label above it. A placeholder is never the label.
- Required fields have the word „Pflichtfeld“ in the label, not only an asterisk.
- Help text is below the label and connects with `aria-describedby`.
- tada checks a field when the member leaves it and again on submit.
- On submit with errors:
  1. An error summary appears at the top of the form, with a link to each field.
  2. Focus moves to the summary.
  3. Each field shows its error below the input, in `--color-danger`, with the `alert-circle` icon.
- An error message names the problem and the fix: „Das Datum liegt vor dem Beginn des Anlasses. Wählen Sie ein Datum ab 12.06.2027.“
- tada never clears the input of a member after an error.

## Tables and lists

- Data tables use `table` semantics with a caption (visible or `visually-hidden`), column headers and `aria-sort` on sortable columns.
- Two-line list rows on narrow containers keep the column names as visible labels or as `visually-hidden` text.
- Selection uses checkboxes with labels that name the row: „ACT-042 auswählen“.

## Status and live regions

- Status changes that do not move focus (saved, sync failed, new proposal) go to one polite live region.
- Errors that block the task go to an assertive live region.
- A toast never contains the only way to do an action, and it stays until the member closes it if it contains an action.

## Color and contrast

- Use only the color pairs that [tokens.md](tokens.md) lists as checked.
- Text and icons that carry a meaning have a contrast of at least 4.5:1 and 3:1.
- Disabled controls also show the reason for the state, in a tooltip or in help text.

## Motion and preferences

- `prefers-reduced-motion: reduce`: no transforms; opacity changes take at most 120 ms.
- `prefers-contrast: more`: `--color-border-subtle` takes the value of `--color-border-control`, and `--color-text-muted` takes the value of `--color-text`.
- `forced-colors: active`: borders and focus rings use `CanvasText` and `Highlight`. Icons use `currentColor`.

## Language and locale

- The `html` element has the `lang` attribute of the member's locale, by default `de-CH`.
- Text in another language, for example an English supplier message in the evidence panel, has its own `lang` attribute.
- Long words wrap with `hyphens: auto`. IDs and URLs wrap with `overflow-wrap: anywhere`.
- Fluent messages contain full sentences. Code never joins sentence fragments.

Formats for `de-CH` (through `Intl` and `@internationalized/date`):

| Value                 | Format                        | Example             |
| --------------------- | ----------------------------- | ------------------- |
| Date                  | `dd.MM.yyyy`                  | 12.06.2027          |
| Date with weekday     | weekday, day, month           | „Sa, 12. Juni 2027“ |
| Time                  | 24 hours                      | 14:30               |
| Number                | apostrophe as group separator | 1’234.50            |
| Amount                | currency code first           | CHF 1’234.50        |
| First day of the week | Monday                        |                     |

- Relative times („vor 2 Stunden“) always have the exact time in a tooltip and in the accessible name.

## Test procedure

Automated (ADR 0024): axe-core on each route, in both themes, at 375 px and 1440 px.

Manual, for each new screen, recorded in the pull request:

1. Use the screen with the keyboard only. Check the focus order and that focus is always visible.
2. Use the screen with a screen reader:
   - macOS or iOS: VoiceOver with Safari.
   - Windows: NVDA with Firefox or Chrome.
   - Android: TalkBack with Chrome, for mobile views.
3. Zoom to 200 % and check that no content or function is lost.
4. Set the width to 320 px and check that the page needs no horizontal scrolling (WCAG 1.4.10).
5. Turn on `prefers-reduced-motion` and forced colors, and check the screen.
