# Design tokens

[tokens.css](../../apps/web/src/styles/tokens.css) is the authority for the token names and values (ADR 0018).
This document gives the uses, the rules and the reasons.
The values are only in tokens.css. The breakpoints are the exception (see below).
Components use only these tokens. Stylelint rejects raw values (ADR 0019).

## Color

The tokens define all colors in OKLCH.
Neutral colors have hue 255 (a slightly cool gray). The accent is petrol, hue 220.
The design system uses no other hue except the three status hues: success 150, warning 65 to 85, danger 25.

| Token                    | Use                                      |
| ------------------------ | ---------------------------------------- |
| `--color-bg-canvas`      | App background                           |
| `--color-bg-surface`     | Panels, tables, inputs                   |
| `--color-bg-raised`      | Menus, popovers, dialogs, sheets         |
| `--color-bg-sunken`      | Sidebar, table header, code              |
| `--color-bg-hover`       | Hover and selected row without focus     |
| `--color-border-subtle`  | Dividers and panel borders               |
| `--color-border-control` | Input, checkbox and select borders (3:1) |
| `--color-text`           | Primary text                             |
| `--color-text-muted`     | Secondary text and placeholders          |
| `--color-text-on-accent` | Text on accent buttons                   |
| `--color-accent`         | Primary button background                |
| `--color-accent-hover`   | Primary button hover                     |
| `--color-accent-text`    | Links and selected labels                |
| `--color-accent-subtle`  | Selected row, current navigation item    |
| `--color-focus`          | Focus ring                               |
| `--color-success`        | Success text and icons                   |
| `--color-success-subtle` | Success background                       |
| `--color-warning`        | Warning text and icons                   |
| `--color-warning-subtle` | Warning background                       |
| `--color-danger`         | Error text, icons, destructive buttons   |
| `--color-danger-subtle`  | Error background                         |

Overlays use a scrim of `--color-scrim`, the same in both themes.

### Checked contrast pairs

A script computed these ratios with the WCAG 2.2 formula on 2026-10-06.
A change of a color token must keep each pair at or above its minimum.

| Foreground       | Background       | Minimum | Light   | Dark    |
| ---------------- | ---------------- | ------- | ------- | ------- |
| `text`           | `bg-canvas`      | 4.5:1   | 16.58:1 | 16.16:1 |
| `text`           | `bg-sunken`      | 4.5:1   | 15.64:1 | 16.60:1 |
| `text-muted`     | `bg-canvas`      | 4.5:1   | 6.26:1  | 7.77:1  |
| `text-muted`     | `bg-sunken`      | 4.5:1   | 5.90:1  | 7.98:1  |
| `text-muted`     | `bg-hover`       | 4.5:1   | 5.56:1  | 6.54:1  |
| `text-on-accent` | `accent`         | 4.5:1   | 6.36:1  | 8.60:1  |
| `text-on-accent` | `accent-hover`   | 4.5:1   | 8.26:1  | 10.58:1 |
| `accent-text`    | `bg-canvas`      | 4.5:1   | 6.36:1  | 9.23:1  |
| `accent-text`    | `accent-subtle`  | 4.5:1   | 5.85:1  | 7.15:1  |
| `border-control` | `bg-surface`     | 3:1     | 3.64:1  | 3.43:1  |
| `border-control` | `bg-canvas`      | 3:1     | 3.49:1  | 3.65:1  |
| `focus`          | `bg-canvas`      | 3:1     | 5.12:1  | 8.64:1  |
| `focus`          | `bg-surface`     | 3:1     | 5.35:1  | 8.12:1  |
| `success`        | `bg-surface`     | 4.5:1   | 6.48:1  | 8.87:1  |
| `success`        | `success-subtle` | 4.5:1   | 5.74:1  | 7.28:1  |
| `warning`        | `bg-surface`     | 4.5:1   | 6.18:1  | 9.59:1  |
| `warning`        | `warning-subtle` | 4.5:1   | 5.50:1  | 7.77:1  |
| `danger`         | `bg-surface`     | 4.5:1   | 6.55:1  | 6.84:1  |
| `danger`         | `danger-subtle`  | 4.5:1   | 5.71:1  | 5.62:1  |
| `danger`         | `bg-hover`       | 4.5:1   | 5.58:1  | 6.13:1  |
| `text`           | `accent-subtle`  | 4.5:1   | 15.25:1 | 12.51:1 |
| `text`           | `bg-raised`      | 4.5:1   | 17.31:1 | 13.98:1 |
| `text-muted`     | `bg-raised`      | 4.5:1   | 6.53:1  | 6.72:1  |
| `border-control` | `bg-raised`      | 3:1     | 3.64:1  | 3.16:1  |

### State of knowledge

Each value in tada shows its state of knowledge.
The state always has a label and an icon. Color and line style only support them.

| State      | Label (de-CH) | Icon (Tabler)    | Visual treatment                                                                                              |
| ---------- | ------------- | ---------------- | ------------------------------------------------------------------------------------------------------------- |
| accepted   | „Bestätigt“   | `circle-check`   | Normal text. The label shows only on hover, in detail views and in the evidence panel.                        |
| proposed   | „Vorschlag“   | `circle-dashed`  | 1 px dashed border in `--color-accent`, background `--color-accent-subtle`, label always visible.             |
| assumption | „Annahme“     | `circle-dotted`  | 1 px dotted underline in `--color-text-muted`, label always visible.                                          |
| unknown    | „Unbekannt“   | `help-circle`    | The word „Unbekannt“ in `--color-text-muted`. tada never shows an empty field or a dash for an unknown value. |
| conflict   | „Konflikt“    | `alert-triangle` | Background `--color-danger-subtle`, text and icon in `--color-danger`, label always visible.                  |

## Typography

Each font size has a line height token with the same suffix, for example `--line-height-md`.

| Token              | Use                                                                        |
| ------------------ | -------------------------------------------------------------------------- |
| `--font-size-xs`   | Badges, keyboard hints. Never for sentences.                               |
| `--font-size-sm`   | Secondary text in rows, captions, metadata                                 |
| `--font-size-md`   | Default UI text in compact density                                         |
| `--font-size-base` | Default UI text in comfortable density, prose, all inputs on touch devices |
| `--font-size-lg`   | Section heading (h3)                                                       |
| `--font-size-xl`   | Panel and dialog title (h2)                                                |
| `--font-size-2xl`  | Page title (h1)                                                            |
| `--font-size-3xl`  | Event title on the event overview, wide layout only                        |

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

Rules:

- The space above a heading is larger than the space below it.
- Related items use `--space-1` to `--space-2`. Separate groups use `--space-4` to `--space-8`.

## Density and sizes

| Token                 | Use                                                                                     |
| --------------------- | --------------------------------------------------------------------------------------- |
| `--control-height-sm` | Small buttons and inputs in toolbars                                                    |
| `--control-height-md` | Buttons and inputs                                                                      |
| `--control-height-lg` | Primary actions on mobile pages                                                         |
| `--row-height`        | Table and list rows                                                                     |
| `--font-size-ui`      | UI text: `--font-size-md` in compact density, `--font-size-base` in comfortable density |

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

| Token        | Use                               |
| ------------ | --------------------------------- |
| `--z-sticky` | Sticky table headers and toolbars |
| `--z-shell`  | Sidebar and bottom bar            |
| `--z-toast`  | Toasts                            |

Dialogs, sheets, popovers, menus and tooltips use the browser top layer (`<dialog>` or the Popover API through React Aria Components). They need no z-index.

## Breakpoints

| Token         | Value           | Use                                      |
| ------------- | --------------- | ---------------------------------------- |
| `--bp-medium` | 40rem (640 px)  | App shell: sidebar instead of bottom bar |
| `--bp-wide`   | 80rem (1280 px) | App shell: docked evidence panel         |

CSS custom properties do not work in media queries, so the CSS uses the literal values with a comment that names the token.
Components use container queries instead (see [layout-and-responsiveness.md](layout-and-responsiveness.md)).
