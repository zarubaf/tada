# 0021. Self-hosted typography and one icon set

- Status: Accepted
- Date: 2026-10-06

## Context

German UI text has long words, for example „Veranstaltungsbewilligung“.
Tables need tabular figures.
Members' data must not go to a font CDN.
We measured the width of one German UI sample (113 characters) in five open-source fonts, at the same size:

| Font          | Width   | x-height | Tabular figures       |
| ------------- | ------- | -------- | --------------------- |
| Inter         | 56.1 em | 0.546    | yes                   |
| Geist         | 54.8 em | 0.530    | yes                   |
| IBM Plex Sans | 53.6 em | 0.516    | not in the web subset |
| Mona Sans     | 52.8 em | 0.517    | yes                   |
| Source Sans 3 | 46.3 em | 0.478    | not in the web subset |

Phosphor icons had no release since May 2025. Tabler icons release each month.

## Decision

- The UI font is Mona Sans (variable, weights 200–900, SIL Open Font License 1.1).
- The monospace font is JetBrains Mono (variable, SIL Open Font License 1.1). It is only for IDs, hashes and code.
- The web client self-hosts both fonts from the `@fontsource-variable` packages, with the `latin` and `latin-ext` subsets and `font-display: swap`.
- No font comes from a CDN.
- Numbers in tables, dates and amounts use `font-variant-numeric: tabular-nums`.
- The icon set is Tabler Icons (`@tabler/icons-react`, MIT), outline style, stroke width 1.5, sizes 16 px and 20 px.
- Icons never replace a text label for an action, except in a toolbar with a visible tooltip and an accessible name.
- No emoji in the UI.

## Consequences

- Mona Sans is about 6 % narrower than Inter, so German labels wrap less.
- One UI family keeps the type system simple; weight and size create the hierarchy.
- Mona Sans `latin` is 40 kB (WOFF2). The browser loads `latin-ext` (16 kB) and JetBrains Mono (40 kB) only when a page uses their characters.

## Alternatives

- Inter: the most legible x-height, but the widest, and the default of many generated UIs.
- Geist: good, but strongly linked to one vendor's look.
- System fonts: no download, but different metrics on each platform make layouts less predictable.
- Phosphor icons: good design, but no release for 17 months.
- Lucide: active, but the default icon set of many generated UIs.
