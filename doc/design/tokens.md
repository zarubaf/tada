# Design tokens

[tokens.css](../../apps/web/src/styles/tokens.css) is the authority for the token names and values (ADR 0018).
The values in this document are for review. If a value here differs from tokens.css, tokens.css is correct; change both together.
Components use only these tokens. Stylelint rejects raw values (ADR 0019).

## Color

The tokens define all colors in OKLCH. The hex value is the sRGB result, for reference.
Neutral colors have hue 255 (a slightly cool gray). The accent is petrol, hue 220.
The design system uses no other hue except the three status hues: success 150, warning 65 to 85, danger 25.

| Token                    | Use                                      | Light                              | Dark                               |
| ------------------------ | ---------------------------------------- | ---------------------------------- | ---------------------------------- |
| `--color-bg-canvas`      | App background                           | `oklch(98.5% 0.002 255)` `#f9fafb` | `oklch(16.5% 0.006 255)` `#0d0e11` |
| `--color-bg-surface`     | Panels, tables, inputs                   | `oklch(100% 0 255)` `#ffffff`      | `oklch(20% 0.007 255)` `#141619`   |
| `--color-bg-raised`      | Menus, popovers, dialogs, sheets         | `oklch(100% 0 255)` `#ffffff`      | `oklch(23.5% 0.008 255)` `#1c1e22` |
| `--color-bg-sunken`      | Sidebar, table header, code              | `oklch(96.5% 0.004 255)` `#f2f4f6` | `oklch(14.5% 0.006 255)` `#090a0d` |
| `--color-bg-hover`       | Hover and selected row without focus     | `oklch(94.5% 0.005 255)` `#ebedf0` | `oklch(24.5% 0.008 255)` `#1e2124` |
| `--color-border-subtle`  | Dividers and panel borders               | `oklch(91.5% 0.005 255)` `#e1e3e6` | `oklch(28% 0.008 255)` `#26292d`   |
| `--color-border-control` | Input, checkbox and select borders (3:1) | `oklch(62% 0.012 255)` `#81878d`   | `oklch(53% 0.012 255)` `#676c73`   |
| `--color-text`           | Primary text                             | `oklch(22% 0.012 255)` `#171b20`   | `oklch(94% 0.006 255)` `#e8ebef`   |
| `--color-text-muted`     | Secondary text and placeholders          | `oklch(48% 0.014 255)` `#585e66`   | `oklch(72% 0.012 255)` `#a0a5ac`   |
| `--color-text-on-accent` | Text on accent buttons                   | `oklch(100% 0 255)` `#ffffff`      | `oklch(16.5% 0.006 255)` `#0d0e11` |
| `--color-accent`         | Primary button background                | `oklch(48% 0.082 220)` `#12687d`   | `oklch(74% 0.1 220)` `#58bad6`     |
| `--color-accent-hover`   | Primary button hover                     | `oklch(42% 0.072 220)` `#0d5668`   | `oklch(80% 0.09 220)` `#77cce6`    |
| `--color-accent-text`    | Links and selected labels                | `oklch(47% 0.08 220)` `#12657a`    | `oklch(76% 0.1 220)` `#5fc0dd`     |
| `--color-accent-subtle`  | Selected row, current navigation item    | `oklch(95.5% 0.018 220)` `#e4f3f9` | `oklch(27% 0.04 220)` `#0b2b34`    |
| `--color-focus`          | Focus ring                               | `oklch(52% 0.088 220)` `#18748c`   | `oklch(74% 0.12 220)` `#39bcdf`    |
| `--color-success`        | Success text and icons                   | `oklch(47% 0.11 150)` `#206b38`    | `oklch(76% 0.13 150)` `#6fc884`    |
| `--color-success-subtle` | Success background                       | `oklch(95.5% 0.03 150)` `#e3f6e6`  | `oklch(27% 0.04 150)` `#172c1c`    |
| `--color-warning`        | Warning text and icons                   | `oklch(50% 0.11 65)` `#8d5406`     | `oklch(80% 0.12 80)` `#e6b55d`     |
| `--color-warning-subtle` | Warning background                       | `oklch(96% 0.04 85)` `#fef0d4`     | `oklch(28% 0.045 75)` `#36250d`    |
| `--color-danger`         | Error text, icons, destructive buttons   | `oklch(50% 0.17 25)` `#b02a2d`     | `oklch(72% 0.15 25)` `#f47b74`     |
| `--color-danger-subtle`  | Error background                         | `oklch(95.5% 0.02 25)` `#feebe9`   | `oklch(28% 0.05 25)` `#3e1e1c`     |

Overlays use a scrim of `--color-scrim`: `oklch(16.5% 0.006 255 / 0.4)` in both themes.

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

| Token              | Size              | Line height | Use                                                                        |
| ------------------ | ----------------- | ----------- | -------------------------------------------------------------------------- |
| `--font-size-xs`   | 0.75rem (12 px)   | 1rem        | Badges, keyboard hints. Never for sentences.                               |
| `--font-size-sm`   | 0.8125rem (13 px) | 1.25rem     | Secondary text in rows, captions, metadata                                 |
| `--font-size-md`   | 0.875rem (14 px)  | 1.25rem     | Default UI text in compact density                                         |
| `--font-size-base` | 1rem (16 px)      | 1.5rem      | Default UI text in comfortable density, prose, all inputs on touch devices |
| `--font-size-lg`   | 1.125rem (18 px)  | 1.75rem     | Section heading (h3)                                                       |
| `--font-size-xl`   | 1.25rem (20 px)   | 1.75rem     | Panel and dialog title (h2)                                                |
| `--font-size-2xl`  | 1.5rem (24 px)    | 2rem        | Page title (h1)                                                            |
| `--font-size-3xl`  | 1.875rem (30 px)  | 2.25rem     | Event title on the event overview, wide layout only                        |

| Token                    | Value                                                |
| ------------------------ | ---------------------------------------------------- |
| `--font-family-sans`     | `"Mona Sans Variable", system-ui, sans-serif`        |
| `--font-family-mono`     | `"JetBrains Mono Variable", ui-monospace, monospace` |
| `--font-weight-regular`  | 400                                                  |
| `--font-weight-medium`   | 500 (labels, buttons, table headers)                 |
| `--font-weight-semibold` | 600 (headings)                                       |
| `--letter-spacing-tight` | -0.01em (sizes from 20 px)                           |

Rules:

- No weight above 600. No italics for emphasis; use `--font-weight-medium`.
- Headings use `text-wrap: balance`. Prose uses `text-wrap: pretty`.
- Numbers, dates, times and amounts use `font-variant-numeric: tabular-nums`.
- IDs such as `ACT-042`, hashes and code use `--font-family-mono` at `--font-size-sm`.

## Spacing

The spacing scale has a base of 4 px.

| Token         | Value |
| ------------- | ----- |
| `--space-0-5` | 2px   |
| `--space-1`   | 4px   |
| `--space-1-5` | 6px   |
| `--space-2`   | 8px   |
| `--space-3`   | 12px  |
| `--space-4`   | 16px  |
| `--space-5`   | 20px  |
| `--space-6`   | 24px  |
| `--space-8`   | 32px  |
| `--space-10`  | 40px  |
| `--space-12`  | 48px  |
| `--space-16`  | 64px  |

Rules:

- The space above a heading is larger than the space below it.
- Related items use `--space-1` to `--space-2`. Separate groups use `--space-4` to `--space-8`.

## Density and sizes

| Token                 | Compact          | Comfortable        |
| --------------------- | ---------------- | ------------------ |
| `--control-height-sm` | 24px             | 32px               |
| `--control-height-md` | 28px             | 40px               |
| `--control-height-lg` | 32px             | 44px               |
| `--row-height`        | 32px             | 44px               |
| `--font-size-ui`      | `--font-size-md` | `--font-size-base` |

- The default is `compact` with a fine pointer (`pointer: fine`) and `comfortable` with a coarse pointer.
- A member can choose the density in the settings.
- Each pointer target is at least 24 × 24 px in compact and 44 × 44 px in comfortable density, including its padding.

## Radius

| Token           | Value  | Use                                                 |
| --------------- | ------ | --------------------------------------------------- |
| `--radius-sm`   | 4px    | Badges, checkboxes, tags, keyboard hints            |
| `--radius-md`   | 6px    | Buttons, inputs, menu items, rows with a background |
| `--radius-lg`   | 8px    | Panels, popovers, menus                             |
| `--radius-xl`   | 12px   | Dialogs and the top corners of bottom sheets        |
| `--radius-full` | 9999px | Avatars and switches only                           |

Rule: an element inside a container has a radius equal to or smaller than the container radius minus the padding, and at least 2 px.

## Elevation

| Token        | Light                                                                           | Dark                             | Use                                                      |
| ------------ | ------------------------------------------------------------------------------- | -------------------------------- | -------------------------------------------------------- |
| `--shadow-0` | none                                                                            | none                             | Everything in the page flow; use `--color-border-subtle` |
| `--shadow-1` | `0 1px 2px oklch(22% 0.012 255 / 0.06), 0 4px 12px oklch(22% 0.012 255 / 0.10)` | `0 4px 12px oklch(0% 0 0 / 0.4)` | Menus, popovers, tooltips                                |
| `--shadow-2` | `0 8px 32px oklch(22% 0.012 255 / 0.18)`                                        | `0 8px 32px oklch(0% 0 0 / 0.5)` | Dialogs and sheets                                       |

Elements with a shadow also have a 1 px border in `--color-border-subtle` and the background `--color-bg-raised`.

## Motion

| Token             | Value                          | Use                         |
| ----------------- | ------------------------------ | --------------------------- |
| `--duration-fast` | 120ms                          | Hover, press, color changes |
| `--duration-base` | 180ms                          | Menus, popovers, tooltips   |
| `--duration-slow` | 240ms                          | Sheets and dialogs          |
| `--ease-out`      | `cubic-bezier(0.2, 0, 0, 1)`   | Elements that appear        |
| `--ease-in-out`   | `cubic-bezier(0.4, 0, 0.2, 1)` | Elements that move or close |

- Only `opacity`, `transform`, `background-color`, `border-color` and `color` change in an animation.
- With `prefers-reduced-motion: reduce`, transforms stop, and opacity changes take at most 120 ms.

## Z-index

| Token        | Value | Use                               |
| ------------ | ----- | --------------------------------- |
| `--z-sticky` | 10    | Sticky table headers and toolbars |
| `--z-shell`  | 20    | Sidebar and bottom bar            |
| `--z-toast`  | 60    | Toasts                            |

Dialogs, sheets, popovers, menus and tooltips use the browser top layer (`<dialog>` or the Popover API through React Aria Components). They need no z-index.

## Breakpoints

| Token         | Value           | Use                                      |
| ------------- | --------------- | ---------------------------------------- |
| `--bp-medium` | 40rem (640 px)  | App shell: sidebar instead of bottom bar |
| `--bp-wide`   | 80rem (1280 px) | App shell: docked evidence panel         |

CSS custom properties do not work in media queries, so the CSS uses the literal values with a comment that names the token.
Components use container queries instead (see [layout-and-responsiveness.md](layout-and-responsiveness.md)).
