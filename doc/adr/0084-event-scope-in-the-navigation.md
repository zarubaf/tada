# 0084. Event scope in the navigation

- Status: Accepted
- Date: 2026-10-17
- Amends: [0082](0082-redesign-of-the-web-client.md)

## Context

The product owner uses the web client and finds the navigation inconsistent.
Three things work against each other:

- The event switcher shows the remembered event on every page.
- The sidebar shows „Arbeit“, „Zeitplan“ and „Dokumente“ only while the path names an event, so items appear and disappear.
- „Anlässe“ is a menu item, the title of the switcher menu and the target of „Alle Anlässe“.
  Three controls lead to one list.

ADR 0082 set these rules.
The product owner wants one strategy: one way to reach each goal and no item that comes and goes.

## Decision

We split the destinations into two scopes and give each scope fixed items.

### Scopes

- The organization scope has „Eingang“, „Meine Arbeit“, „Kontakte“, „Einstellungen“ and „Datenschutz“.
- The event scope has „Übersicht“, „Arbeit“, „Zeitplan“ and „Dokumente“.
- The scoped event is the event in the route, else the remembered event, else the most recent open event, else none.
  One hook returns it.
  The switcher, the menu, the title of the top bar, the bottom bar and the command menu all read that hook.

### Switcher

- The event switcher is the only control that chooses an event.
  Its menu lists the events, then „Alle Anlässe“ (opens `/events`), then „Neuer Anlass“.
- On an event page, a switch keeps the page: `/events/A/work` goes to `/events/B/work`, and `/events/A/documents` goes to `/events/B/documents`.
  A view parameter such as `?view=timeline` stays.
  Other query parameters (filters) and the hash drop.
  A document page goes to the documents of B.
- On an organization page, a switch opens „Übersicht“ of the new event (`/events/B`).
- On a phone the switcher sits in the top bar, not only in the drawer.
  Each layout has one switcher.

### Menu

The menu has the same items in every state.
No item appears or disappears.
From top to bottom:

1. The organization switcher (only with more than one membership).
2. „Eingang“, „Meine Arbeit“ and „Kontakte“.
3. The event switcher as the heading of the event group.
4. „Übersicht“ (current only on an exact match of `/events/:id`), „Arbeit“, „Zeitplan“ and „Dokumente“.
5. At the foot: „Einstellungen“, „Datenschutz“ and „Versionen“.

- The menu has no „Anlässe“ item and no rule that lets an item yield paths to another item.
  No item is current on `/events`.
- With no scoped event, the four event items are disabled with the hint „Zuerst einen Anlass wählen“.
  The empty list of events offers an action to create an event.
- The bottom bar is fixed: „Eingang“, „Meine Arbeit“, „Arbeit“ (of the scoped event) and „Mehr“.
  „Mehr“ is fixed: „Übersicht“, „Dokumente“, „Kontakte“, „Einstellungen“, „Datenschutz“ and „Versionen“.
- The command menu lists the destinations of the scoped event under „Zum Ziel“.
  Its list of events stays as a search shortcut, and a choice there follows the switch rule above.
- The page title of `/events` is „Alle Anlässe“ in the top bar and as the `h1`, so it matches the entry of the switcher.
- The addresses do not change, and the legacy redirects stay.
  Back is the Back button of the browser.
  There are no breadcrumbs.

This ADR replaces the first two points of „Structure“ in ADR 0082 (the sidebar items and the sidebar head).
The other decisions of ADR 0082 stay in force.

## Consequences

- A member finds each goal in one place, and the menu looks the same on every page.
- A member who works in one event can change to another event without losing the page.
- The member sees at all times which event the event items act on.
- A page outside an event, for example „Kontakte“, still shows the event group.
  The group takes space that a menu without it would not need.
- A member with no event sees four disabled items until the member creates or chooses an event.
- The code must read the scoped event from one hook.
  We remove the path yield of „Anlässe“ from the destinations.
  The rules `onlyWith` and `unlessWith` stay for „Zeitplan“.
- The tests and the screenshot baselines of the shell change.

## Alternatives

- Keep „Anlässe“ as an item and drop the switcher: switching events then needs a detour through the list, and the page does not stay.
- Hide the event items outside an event: items appear and disappear, which is the problem we solve.
- Show a tree of events in the sidebar: it grows with the number of events and crowds the foot on a phone.
- Use tabs on the event page: they repeat the menu and give two ways to reach the same page.
