# 0023. Responsive layout: one app, two primary contexts

- Status: Accepted
- Date: 2026-10-06

## Context

Volunteers and workstream leads use phones, often outside, for short tasks: answer a reminder, check an assignment, review a proposal.
The PM and event leads use desktops for the portfolio, registers and documents.
A separate mobile app or a separate mobile site doubles the work.

## Decision

- One web client serves all screen sizes. It is mobile-first.
- The app shell has three layouts, chosen by the viewport width:

| Name   | Width                     | Navigation                 | Evidence panel               |
| ------ | ------------------------- | -------------------------- | ---------------------------- |
| narrow | below 40rem (640 px)      | bottom bar with four items | full-screen sheet            |
| medium | 40rem to 80rem            | collapsible sidebar        | overlay sheet from the right |
| wide   | 80rem (1280 px) and above | sidebar                    | docked column on the right   |

- Components adapt to their container with container queries, not to the viewport.
- Tables become lists of rows with two lines on narrow containers. A table never scrolls sideways on a phone, except a wide data table that the member opens on purpose.
- The most frequent mobile tasks (My Work, the Review Inbox and assignment answers) need no more than two taps from the start screen.
- The layout uses `100dvh`, not `100vh`, and respects the safe area insets.
- Text measure for prose is at most 70 characters. Data views can be wider.
- tada does not use fluid type. The type scale is fixed in rem (ADR 0018).

## Consequences

- One code base, one set of components.
- Each screen needs a review at three widths: 375 px, 1024 px and 1440 px.

## Alternatives

- A native mobile app: higher cost, app store work, and no benefit for the planned tasks.
- Desktop first: the mobile views become an afterthought, but volunteers use phones.
