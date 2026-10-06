# 0005. React and Vite web client with German UI

- Status: Proposed
- Date: 2026-10-06

## Context

The web client must use the same API as the other channels (ADR 0017).
Members in Zurich and Dübendorf use German.
Mobile participation matters more than a decorative dashboard.

## Decision

- The web client is a React single-page application. Vite builds it.
- The client calls the server only through the generated API client.
- The HTTP server serves the built files. There is no separate web server process.
- All user-facing text uses Fluent message files in one `locales/` folder. The web client, Telegram messages and emails share these files.
- The web client uses `@fluent/bundle`; the backend uses `fluent-rs`.
- German (`de-CH`) is the first locale.
- The layout starts with the mobile view.

## Consequences

- The UI cannot bypass the domain commands.
- An English locale later needs translations only, not code changes.
- One mechanism and one set of files for the browser and the backend.
- No server-side rendering. The app is internal and behind a sign-in, so search engines do not matter.

## Alternatives

- Next.js: server actions and server components can call the domain without the API contract.
- A UI with no i18n: a later change touches each string.
- i18next in the web and a different library in Rust: two formats for the same text.
