# 0020. React Aria Components as the accessible base

- Status: Accepted
- Date: 2026-10-06

## Context

Accessible menus, dialogs, comboboxes, date pickers and tables are hard to build correctly.
tada needs dates and numbers in `de-CH` format, and keyboard use everywhere.
On 2026-10-06, the npm registry showed:

| Library                  | Latest | Release date | License    |
| ------------------------ | ------ | ------------ | ---------- |
| `react-aria-components`  | 1.21.1 | 2026-09-04   | Apache-2.0 |
| `@base-ui/react`         | 1.8.0  | 2026-09-04   | MIT        |
| `@radix-ui/react-dialog` | 1.2.0  | 2026-10-05   | MIT        |

All three have active releases.

## Decision

- The web client uses `react-aria-components` (RAC) as the unstyled base for interactive components.
- tada wraps each RAC component in its own component in `apps/web/src/ui/`. Screens use only these wrappers.
- Dates use `@internationalized/date` with the `de-CH` locale, which RAC uses internally.
- We add a primitive outside RAC only through an ADR.

## Consequences

- RAC brings tested keyboard, focus and screen reader behavior, and locale-aware date and number input.
- RAC has the largest set of components, including a date picker, a table with selection and a tree.
- The wrappers keep RAC replaceable, because screens never import RAC directly.
- RAC uses render props and data attributes. Contributors must learn this API.

## Alternatives

- Radix Primitives: good, but no date picker and less locale support.
- Base UI: modern and active, but fewer components for dates and tables, and a shorter release history after version 1.
- A complete styled kit such as Radix Themes or shadcn/ui: faster first screens, but the generic look that the product owner wants to avoid.
