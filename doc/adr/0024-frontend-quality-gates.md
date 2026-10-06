# 0024. Frontend quality gates

- Status: Proposed
- Date: 2026-10-06

## Context

The design system and the accessibility standard (ADRs 0018 to 0023) work only if CI checks them.
Agents produce UI code fast. A check must find drift before review.

## Decision

These checks run in `mise run check` and in CI:

1. TypeScript `strict`, Biome and Stylelint (ADR 0019).
2. Vitest with Testing Library for component behavior.
3. Playwright end-to-end tests against the built web client and a test backend.
4. `@axe-core/playwright` on each route, in light and dark theme, at 375 px and 1440 px. Each violation fails the build.
5. Playwright screenshot comparison for the component gallery and for key routes. The screenshots come from one pinned Linux container, so font rendering is stable.
6. A pseudo-locale test: Fluent messages with 40 % longer text and accented characters. The test fails if text overflows or truncates without a tooltip.
7. A bundle size budget for the initial JavaScript: 200 kB compressed.

The component gallery:

- The development build has a route `/_gallery` that shows each token and each component in each state, in both themes and both densities.
- The production build does not contain this route.
- We do not use Storybook. The gallery covers our need with no extra tool. We review this choice when the component count passes 60.

Manual checks for each new screen, recorded in the pull request:

- Keyboard-only use.
- One screen reader pass (VoiceOver or NVDA).
- 200 % zoom and 320 px width (WCAG reflow).

## Consequences

- Most regressions in contrast, labels, overflow and visuals fail CI.
- Screenshot tests need an update when a design changes on purpose. The pull request shows the image difference.
- CI time increases by a few minutes.

## Alternatives

- Storybook or Ladle: a strong tool, but one more build and dependency set for a small component count.
- No screenshot tests: cheaper, but visual drift goes unnoticed.
- A hosted visual service such as Chromatic: cost and an external service for a small team.
