# 0019. Styling with CSS custom properties and CSS Modules

- Status: Accepted
- Date: 2026-10-06

## Context

The web client needs a styling method that enforces the tokens of ADR 0018.
Modern CSS has cascade layers, container queries, `:focus-visible`, `color-mix()` and `oklch()`. All target browsers support them.
React Aria Components (ADR 0020) exposes component states as data attributes, for example `[data-pressed]`.
Agents write Tailwind fluently, but arbitrary values such as `p-[13px]` bypass the tokens, and Biome has no rule against them.

## Decision

- Tokens are CSS custom properties in `tokens.css`.
- Components use CSS Modules (`*.module.css`). Vite supports them without a plugin.
- Global CSS has three cascade layers in this order: `reset`, `tokens`, `components`.
- Stylelint with `stylelint-declaration-strict-value` fails CI when a color, spacing, radius, font size, shadow, duration or z-index property uses a raw value instead of a token.
- Component states use the data attributes of React Aria Components, for example `.button[data-pressed]`.
- Responsive component layouts use container queries. Media queries are only for the app shell and user preferences.
- No CSS-in-JS runtime.

## Consequences

- The CSS is standard and outlives any framework.
- A raw value fails CI, so drift from the tokens is visible.
- Developers write more CSS lines than with utility classes.

## Alternatives

- Tailwind CSS v4: fast to write, but raw values bypass the tokens, and markup becomes long class lists.
- vanilla-extract: type-safe tokens, but a build plugin and a second language for styles.
- CSS-in-JS with a runtime: run-time cost, and a poor fit with React Server Components should we ever need them.
