# 0022. Accessibility standard: WCAG 2.2 AA

- Status: Accepted
- Date: 2026-10-06

## Context

Members, volunteers and suppliers use tada on phones and desktops, some with a screen reader, a keyboard or a magnifier.
The W3C published WCAG 2.2 in October 2023.
EN 301 549 (the European standard for ICT accessibility) and the Swiss standard eCH-0059 for federal websites build on WCAG AA.
A private club is not bound by eCH-0059 or by the Swiss disability act (BehiG) for its website. A future customer in the public sector can be.
We did not check the latest versions of EN 301 549 and eCH-0059 for this ADR.

## Decision

- WCAG 2.2 level AA is the minimum for each screen. A screen that fails it does not ship.
- Keyboard first:
  - Each function works with a keyboard alone, in a logical order.
  - Focus is always visible: a 2 px ring in `--color-focus` with a 2 px offset.
  - A "Skip to content" link starts each page.
  - Frequent actions have keyboard shortcuts. A shortcut list opens with `?`.
  - Shortcuts with a single character can be turned off (WCAG 2.1.4).
- Pointer targets are at least 24 × 24 CSS px (WCAG 2.5.8). On coarse pointers, tada uses 44 × 44 px.
- Contrast: text at least 4.5:1, large text and UI component boundaries at least 3:1. [doc/design/tokens.md](../design/tokens.md) lists the checked pairs.
- tada never uses color alone to carry a meaning. A status has an icon or a text label as well.
- Motion respects `prefers-reduced-motion`: movement stops, opacity changes stay at 0 ms to 120 ms.
- `prefers-contrast: more` makes borders and muted text stronger.
- Windows High Contrast (`forced-colors: active`) works: focus rings and selection use system colors.
- Language: the `html` element has `lang="de-CH"` (or the locale of the member). Text in another language has its own `lang` attribute.
- Long German words wrap with `hyphens: auto`. IDs and URLs wrap with `overflow-wrap: anywhere`.
- All text comes from Fluent messages (ADR 0005). Text in images is not allowed.
- Errors name the field, the problem and the fix. tada announces them through a live region.
- Session timeouts warn before they end and let the member extend the session (WCAG 2.2.1).

## Consequences

- tada meets the accessibility level that public customers can need, without a later rework.
- Automated checks (ADR 0024) find about one third of the problems. Manual keyboard and screen reader checks stay necessary for each new screen.

## Alternatives

- WCAG 2.1 AA: the level of EN 301 549 v3.2.1, but it misses the 2.2 criteria for focus appearance, target size and dragging.
- WCAG AAA: not possible for all content, for example for uploaded documents.
