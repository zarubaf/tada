# 0005. React and Vite web client with German UI

- Status: Accepted
- Date: 2026-10-06

## Context

The web client must use the same API as the other channels (ADR 0017).
Members in Zurich and Dübendorf use German.
Mobile participation matters more than a decorative dashboard.

## Decision

- The web client is a React single-page application. Vite builds it.
- The client calls the server only through the generated API client.
- The HTTP server serves the built files. There is no separate web server process.
- All UI text uses i18n keys. German (`de-CH`) is the first locale.
- The layout starts with the mobile view.

## Consequences

- The UI cannot bypass the domain commands.
- An English locale later needs translations only, not code changes.
- No server-side rendering. The app is internal and behind a sign-in, so search engines do not matter.

## Alternatives

- Next.js: server actions and server components can call the domain without the API contract.
- A UI with no i18n: a later change touches each string.
