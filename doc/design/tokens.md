# Design tokens

[tokens.css](../../apps/web/src/styles/tokens.css) is the authority for the token names and values (ADR 0018).
This document gives the uses, the rules and the reasons.
The values are only in tokens.css. The breakpoints are the exception (see below).
Components use only these tokens. Stylelint rejects raw values (ADR 0019).

## Color

The tokens define all colors in OKLCH.
Neutral colors have hue 265 (a slightly cool gray). The accent is one blue, hue 262 ([ADR 0082](../adr/0082-redesign-of-the-web-client.md)).
The design system uses no other hue except the three status hues (success 150, warning 65 to 85, danger 25) and the hue of a proposal (300).
The accent marks primary actions, selection, links and focus only.

| Token                     | Use                                      |
| ------------------------- | ---------------------------------------- |
| `--color-bg-canvas`       | App background                           |
| `--color-bg-surface`      | Panels, tables, inputs                   |
| `--color-bg-raised`       | Menus, popovers, dialogs, sheets         |
| `--color-bg-sunken`       | Sidebar, table header, code              |
| `--color-bg-hover`        | Hover and selected row without focus     |
| `--color-border-subtle`   | Dividers and panel borders               |
| `--color-border-control`  | Input, checkbox and select borders (3:1) |
| `--color-text`            | Primary text                             |
| `--color-text-muted`      | Secondary text and placeholders          |
| `--color-text-on-accent`  | Text on accent buttons                   |
| `--color-accent`          | Primary button background                |
| `--color-accent-hover`    | Primary button hover                     |
| `--color-accent-text`     | Links and selected labels                |
| `--color-accent-subtle`   | Selected row, current navigation item    |
| `--color-focus`           | Focus ring                               |
| `--color-success`         | Success text and icons                   |
| `--color-success-subtle`  | Success background                       |
| `--color-warning`         | Warning text and icons                   |
| `--color-warning-subtle`  | Warning background                       |
| `--color-danger`          | Error text, icons, destructive buttons   |
| `--color-danger-subtle`   | Error background                         |
| `--color-proposed`        | Border, text and icon of a proposal      |
| `--color-proposed-subtle` | Background of a proposal                 |

Overlays use a scrim of `--color-scrim`, the same in both themes.

### Themes

The dark and the light theme define the same token names.
The default follows `prefers-color-scheme`. The attribute `data-theme="light"` or `data-theme="dark"` on `<html>` overrides it.
Without the attribute, the system decides.
The member chooses system, light or dark with the theme toggle (see [components.md](components.md)).
The choice is stored in the browser (`localStorage`, key `tada-theme`).
The file `theme-init.js` applies it before the first paint, so the page does not flash.
The page works without the stored choice.
Status never shows by color alone, so the hues need no separate color-blind variant.

### Checked contrast pairs

The test [contrast.test.ts](../../apps/web/src/styles/contrast.test.ts) computes these ratios from tokens.css with the WCAG 2.2 formula, in both themes.
A change of a color token must keep each pair at or above its minimum. The values below are from 2026-10-14.
The text pairs include `--color-text-muted` on every surface, because muted text is used on all of them.
The `accent` pairs are for the accent as a UI indicator: the fill of a checked box or switch and a selected border.
The `avatar-1` to `avatar-6` tokens are the fixed palette of the avatar: a neutral and five hues. Initials use `--color-text` on them.

| Foreground       | Background        | Minimum | Light   | Dark    |
| ---------------- | ----------------- | ------- | ------- | ------- |
| `text`           | `bg-canvas`       | 4.5:1   | 16.58:1 | 16.59:1 |
| `text`           | `bg-surface`      | 4.5:1   | 17.07:1 | 15.60:1 |
| `text`           | `bg-raised`       | 4.5:1   | 17.22:1 | 15.04:1 |
| `text`           | `bg-sunken`       | 4.5:1   | 15.97:1 | 17.51:1 |
| `text`           | `bg-hover`        | 4.5:1   | 15.19:1 | 14.04:1 |
| `text`           | `accent-subtle`   | 4.5:1   | 14.92:1 | 14.31:1 |
| `text-muted`     | `bg-canvas`       | 4.5:1   | 6.83:1  | 7.51:1  |
| `text-muted`     | `bg-surface`      | 4.5:1   | 7.03:1  | 7.06:1  |
| `text-muted`     | `bg-raised`       | 4.5:1   | 7.09:1  | 6.81:1  |
| `text-muted`     | `bg-sunken`       | 4.5:1   | 6.57:1  | 7.93:1  |
| `text-muted`     | `bg-hover`        | 4.5:1   | 6.25:1  | 6.36:1  |
| `text-muted`     | `accent-subtle`   | 4.5:1   | 6.14:1  | 6.48:1  |
| `text-on-accent` | `accent`          | 4.5:1   | 5.71:1  | 5.22:1  |
| `text-on-accent` | `accent-hover`    | 4.5:1   | 7.44:1  | 6.49:1  |
| `accent-text`    | `bg-canvas`       | 4.5:1   | 6.52:1  | 9.20:1  |
| `accent-text`    | `bg-surface`      | 4.5:1   | 6.71:1  | 8.64:1  |
| `accent-text`    | `bg-raised`       | 4.5:1   | 6.77:1  | 8.33:1  |
| `accent-text`    | `bg-sunken`       | 4.5:1   | 6.28:1  | 9.71:1  |
| `accent-text`    | `accent-subtle`   | 4.5:1   | 5.87:1  | 7.93:1  |
| `border-control` | `bg-canvas`       | 3:1     | 3.49:1  | 3.84:1  |
| `border-control` | `bg-surface`      | 3:1     | 3.59:1  | 3.61:1  |
| `border-control` | `bg-raised`       | 3:1     | 3.62:1  | 3.48:1  |
| `border-control` | `bg-sunken`       | 3:1     | 3.36:1  | 4.05:1  |
| `border-control` | `bg-hover`        | 3:1     | 3.19:1  | 3.25:1  |
| `border-control` | `accent-subtle`   | 3:1     | 3.14:1  | 3.31:1  |
| `accent`         | `bg-canvas`       | 3:1     | 5.47:1  | 3.57:1  |
| `accent`         | `bg-surface`      | 3:1     | 5.63:1  | 3.36:1  |
| `accent`         | `bg-raised`       | 3:1     | 5.68:1  | 3.24:1  |
| `accent`         | `bg-sunken`       | 3:1     | 5.27:1  | 3.77:1  |
| `accent`         | `bg-hover`        | 3:1     | 5.01:1  | 3.02:1  |
| `accent`         | `accent-subtle`   | 3:1     | 4.92:1  | 3.08:1  |
| `focus`          | `bg-canvas`       | 3:1     | 5.47:1  | 7.88:1  |
| `focus`          | `bg-surface`      | 3:1     | 5.63:1  | 7.40:1  |
| `focus`          | `bg-raised`       | 3:1     | 5.68:1  | 7.14:1  |
| `focus`          | `bg-sunken`       | 3:1     | 5.27:1  | 8.31:1  |
| `success`        | `bg-canvas`       | 4.5:1   | 5.92:1  | 8.88:1  |
| `success`        | `bg-surface`      | 4.5:1   | 6.09:1  | 8.35:1  |
| `success`        | `bg-raised`       | 4.5:1   | 6.15:1  | 8.05:1  |
| `success`        | `bg-sunken`       | 4.5:1   | 5.70:1  | 9.37:1  |
| `success`        | `bg-hover`        | 4.5:1   | 5.42:1  | 7.52:1  |
| `success`        | `success-subtle`  | 4.5:1   | 5.48:1  | 7.07:1  |
| `text`           | `success-subtle`  | 4.5:1   | 15.35:1 | 13.20:1 |
| `text-muted`     | `success-subtle`  | 4.5:1   | 6.32:1  | 5.98:1  |
| `warning`        | `bg-canvas`       | 4.5:1   | 5.93:1  | 9.77:1  |
| `warning`        | `bg-surface`      | 4.5:1   | 6.10:1  | 9.19:1  |
| `warning`        | `bg-raised`       | 4.5:1   | 6.15:1  | 8.86:1  |
| `warning`        | `bg-sunken`       | 4.5:1   | 5.71:1  | 10.32:1 |
| `warning`        | `bg-hover`        | 4.5:1   | 5.43:1  | 8.27:1  |
| `warning`        | `warning-subtle`  | 4.5:1   | 5.51:1  | 7.70:1  |
| `text`           | `warning-subtle`  | 4.5:1   | 15.41:1 | 13.07:1 |
| `text-muted`     | `warning-subtle`  | 4.5:1   | 6.34:1  | 5.92:1  |
| `danger`         | `bg-canvas`       | 4.5:1   | 6.31:1  | 7.52:1  |
| `danger`         | `bg-surface`      | 4.5:1   | 6.50:1  | 7.07:1  |
| `danger`         | `bg-raised`       | 4.5:1   | 6.55:1  | 6.82:1  |
| `danger`         | `bg-sunken`       | 4.5:1   | 6.08:1  | 7.94:1  |
| `danger`         | `bg-hover`        | 4.5:1   | 5.78:1  | 6.37:1  |
| `danger`         | `danger-subtle`   | 4.5:1   | 5.71:1  | 6.02:1  |
| `text`           | `danger-subtle`   | 4.5:1   | 15.01:1 | 13.29:1 |
| `text-muted`     | `danger-subtle`   | 4.5:1   | 6.18:1  | 6.02:1  |
| `proposed`       | `bg-canvas`       | 4.5:1   | 6.80:1  | 9.29:1  |
| `proposed`       | `bg-surface`      | 4.5:1   | 7.00:1  | 8.73:1  |
| `proposed`       | `bg-raised`       | 4.5:1   | 7.06:1  | 8.42:1  |
| `proposed`       | `bg-sunken`       | 4.5:1   | 6.55:1  | 9.80:1  |
| `proposed`       | `bg-hover`        | 4.5:1   | 6.23:1  | 7.86:1  |
| `proposed`       | `proposed-subtle` | 4.5:1   | 6.17:1  | 7.38:1  |
| `text`           | `proposed-subtle` | 4.5:1   | 15.05:1 | 13.19:1 |
| `text-muted`     | `proposed-subtle` | 4.5:1   | 6.20:1  | 5.97:1  |
| `text`           | `avatar-1`        | 4.5:1   | 12.85:1 | 9.67:1  |
| `text`           | `avatar-2`        | 4.5:1   | 11.82:1 | 9.98:1  |
| `text`           | `avatar-3`        | 4.5:1   | 12.43:1 | 9.72:1  |
| `text`           | `avatar-4`        | 4.5:1   | 12.37:1 | 9.37:1  |
| `text`           | `avatar-5`        | 4.5:1   | 12.28:1 | 9.35:1  |
| `text`           | `avatar-6`        | 4.5:1   | 11.88:1 | 9.93:1  |

### State of knowledge

Each value in tada shows its state of knowledge.
The state always has a label and an icon. Color and line style only support them.

| State      | Label (de-CH) | Icon (Tabler)    | Visual treatment                                                                                              |
| ---------- | ------------- | ---------------- | ------------------------------------------------------------------------------------------------------------- |
| accepted   | „Bestätigt“   | `circle-check`   | Normal text. The label shows only on hover, in detail views and in the evidence panel.                        |
| proposed   | „Vorschlag“   | `circle-dashed`  | 1 px dashed border in `--color-proposed`, background `--color-proposed-subtle`, label always visible.         |
| assumption | „Annahme“     | `circle-dotted`  | 1 px dotted underline in `--color-text-muted`, label always visible.                                          |
| unknown    | „Unbekannt“   | `help-circle`    | The word „Unbekannt“ in `--color-text-muted`. tada never shows an empty field or a dash for an unknown value. |
| conflict   | „Konflikt“    | `alert-triangle` | Background `--color-danger-subtle`, text and icon in `--color-danger`, label always visible.                  |

## Typography

Each font size has a line height token with the same suffix, for example `--line-height-md`.

| Token              | Use                                                                               |
| ------------------ | --------------------------------------------------------------------------------- |
| `--font-size-xs`   | 12 px. Chips, meta text, keyboard hints. No text is smaller. Never for sentences. |
| `--font-size-sm`   | 13 px. Dense rows and cards, secondary text, captions, metadata                   |
| `--font-size-md`   | 14 px. Default UI text and panel text in compact density                          |
| `--font-size-base` | Default UI text in comfortable density, prose, all inputs on touch devices        |
| `--font-size-lg`   | Section heading (h3)                                                              |
| `--font-size-xl`   | Panel and dialog title (h2)                                                       |
| `--font-size-2xl`  | Page title (h1)                                                                   |
| `--font-size-3xl`  | Event title on the event overview, wide layout only                               |

| Token                    | Use                                                  |
| ------------------------ | ---------------------------------------------------- |
| `--font-family-sans`     | All UI text: Mona Sans (ADR 0021)                    |
| `--font-family-mono`     | IDs, hashes and code only: JetBrains Mono (ADR 0021) |
| `--font-weight-regular`  | Body text                                            |
| `--font-weight-medium`   | Labels, buttons, table headers and emphasis          |
| `--font-weight-semibold` | Headings                                             |
| `--letter-spacing-tight` | Sizes from `--font-size-xl`                          |

Rules:

- No weight above 600. No italics for emphasis; use `--font-weight-medium`.
- Headings use `text-wrap: balance`. Prose uses `text-wrap: pretty`.
- Numbers, dates, times and amounts use `font-variant-numeric: tabular-nums`.
- IDs such as `ACT-042`, hashes and code use `--font-family-mono` at `--font-size-sm`.

## Spacing

The spacing scale has a base of 4 px.
The number in the name times 4 px gives the size, for example `--space-4` and `--space-0-5`.
`--space-2-5` (10 px) is the one step between 8 and 12 px: the inset of a card and the side padding of a type filter option.

Rules:

- The space above a heading is larger than the space below it.
- Related items use `--space-1` to `--space-2`. Separate groups use `--space-4` to `--space-8`.

## Density and sizes

| Token                    | Use                                                                                                                                                              |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--control-height-sm`    | Small buttons, tabs and inputs in toolbars (28 px compact)                                                                                                       |
| `--control-height-md`    | Buttons and inputs (32 px compact, 44 px comfortable)                                                                                                            |
| `--control-height-lg`    | Primary actions on mobile pages (36 px compact)                                                                                                                  |
| `--row-height`           | Table and list rows (28 px compact)                                                                                                                              |
| `--size-panel-width`     | Width of the side panel: 480 px, as an overlay and as a docked column                                                                                            |
| `--topbar-height`        | Height of the top bar of the shell (48 px)                                                                                                                       |
| `--sidebar-width`        | Width of the docked sidebar of the shell (232 px)                                                                                                                |
| `--size-tile`            | The tile with the initials of an event in the event switcher (28 px)                                                                                             |
| `--size-toolbar`         | The height of the toolbar row of a page and of the bulk bar (44 px)                                                                                              |
| `--size-chip-height`     | Chips (20 px)                                                                                                                                                    |
| `--size-hit-target`      | The height of the hit area of an interactive chip (0 with a fine pointer, 44 px with a coarse one)                                                               |
| `--size-column-head`     | The height of the head of a board column and the width of the rail of a collapsed one (40 px, 44 px with a coarse pointer)                                       |
| `--size-chip-hit-margin` | The space that an interactive chip keeps above and below, so that the hit areas of wrapped chips do not overlap (0 with a fine pointer, 12 px with a coarse one) |
| `--font-size-ui`         | UI text: `--font-size-md` in compact density, `--font-size-base` in comfortable density                                                                          |
| `--font-size-dense`      | Text of dense rows and cards: `--font-size-sm` in compact, `--font-size-base` in comfortable density                                                             |

- In compact density, body text and panel text are 14 px, and dense rows and cards are 13 px.
  In comfortable density, body text and inputs are 16 px, because iOS zooms into inputs below 16 px.
  Chips and meta text are never below 12 px (ADR 0082 amends ADR 0018).
- The default is `compact` with a fine pointer (`pointer: fine`) and `comfortable` with a coarse pointer.
- A member can choose the density in the settings.
- The comfortable density has larger controls and rows than the compact density.
- Each pointer target is at least 24 × 24 px in compact and 44 × 44 px in comfortable density, including its padding.

## Radius

| Token           | Use                                                 |
| --------------- | --------------------------------------------------- |
| `--radius-sm`   | Badges, checkboxes, tags, keyboard hints            |
| `--radius-md`   | Buttons, inputs, menu items, rows with a background |
| `--radius-lg`   | Panels, popovers, menus                             |
| `--radius-xl`   | Dialogs and the top corners of bottom sheets        |
| `--radius-full` | Avatars and switches only                           |

Rule: an element inside a container has a radius equal to or smaller than the container radius minus the padding, and at least 2 px.

## Elevation

| Token        | Use                                                      |
| ------------ | -------------------------------------------------------- |
| `--shadow-0` | Everything in the page flow; use `--color-border-subtle` |
| `--shadow-1` | Menus, popovers, tooltips                                |
| `--shadow-2` | Dialogs and sheets                                       |

Shadows are on overlays only. There are no gradients, no glow and no blur.
Elements with a shadow also have a 1 px border in `--color-border-subtle` and the background `--color-bg-raised`.

## Motion

| Token             | Use                         |
| ----------------- | --------------------------- |
| `--duration-fast` | Hover, press, color changes |
| `--duration-base` | Menus, popovers, tooltips   |
| `--duration-slow` | Sheets and dialogs          |
| `--ease-out`      | Elements that appear        |
| `--ease-in-out`   | Elements that move or close |

- Only `opacity`, `transform`, `background-color`, `border-color` and `color` change in an animation.
- With `prefers-reduced-motion: reduce`, transforms stop, and opacity changes take at most 120 ms.

## Z-index

| Token        | Use                                                  |
| ------------ | ---------------------------------------------------- |
| `--z-sticky` | Sticky table headers and toolbars                    |
| `--z-shell`  | Sidebar and bottom bar                               |
| `--z-panel`  | The side panel, above the top bar and the bottom bar |
| `--z-toast`  | Toasts                                               |

Dialogs, sheets, popovers, menus and tooltips use the browser top layer (`<dialog>` or the Popover API through React Aria Components). They need no z-index.

## Breakpoints

| Token         | Value           | Use                                      |
| ------------- | --------------- | ---------------------------------------- |
| `--bp-medium` | 40rem (640 px)  | App shell: sidebar instead of bottom bar |
| `--bp-wide`   | 80rem (1280 px) | App shell: docked evidence panel         |

CSS custom properties do not work in media queries, so the CSS uses the literal values with a comment that names the token.
Components use container queries instead (see [layout-and-responsiveness.md](layout-and-responsiveness.md)).
