// The events that the shell shows: the scoped one for the switcher and the title, and the list
// for the switcher menu and the command palette. Both use the events API as the events page does.
import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { LARGEST_PAGE, loadAll } from "../registers/loadAll";
import { eventKey, readRemembered, writeRemembered } from "./remembered";

function lastEventOf(organizationId: string | undefined) {
  return {
    organizationId,
    remembered: organizationId === undefined ? undefined : readRemembered(eventKey(organizationId)),
    // The event that the server did not find: the hook then falls back to the list.
    missing: undefined as string | undefined,
  };
}

/** The newest event that is not closed, or none. `created_at` is the recency key; the ID breaks a tie. */
export function mostRecentOpen(events: readonly Event[]): Event | undefined {
  let newest: Event | undefined;
  for (const event of events) {
    if (
      event.status !== "closed" &&
      (!newest ||
        event.created_at > newest.created_at ||
        (event.created_at === newest.created_at && event.id > newest.id))
    ) {
      newest = event;
    }
  }
  return newest;
}

/** The scoped event, the ID that the menu acts on, and whether the answer is still on its way. */
export interface ScopedEvent {
  event: Event | undefined;
  eventId: string | undefined;
  pending: boolean;
}

/**
 * The scoped event, the one that the event items of the shell act on (ADR 0084): the event of
 * the route, else the one the member opened last in this organization (kept in the browser),
 * else the most recent open event, else none. The route event that the server does not find
 * leaves the remembered one alone; a remembered event that the server does not find is
 * forgotten. Without an organization (a page without the frame) it asks for nothing.
 *
 * `pending` is true while the answer is still on its way, so that the menu does not flash its
 * "no event" state; `eventId` is then the event that the menu acts on in the meantime: the one
 * that the hook asks for.
 */
export function useScopedEvent(
  api: Api,
  routeEventId: string | undefined,
  organizationId: string | undefined,
  events: { list: EventList; load: () => void },
): ScopedEvent {
  const [state, setState] = useState(() => lastEventOf(organizationId));
  const [event, setEvent] = useState<Event>();
  // Another organization has another last event: adjust while rendering, not in an effect.
  if (state.organizationId !== organizationId) {
    setState(lastEventOf(organizationId));
  }
  const id = routeEventId ?? state.remembered;
  // A "not found" belongs to one ID: another ID, or a return to this one, asks again.
  if (state.missing !== undefined && state.missing !== id) {
    setState({ ...state, missing: undefined });
  }
  // The most recent open event stands in for a missing remembered event, never for a wrong link.
  const needsFallback =
    organizationId !== undefined &&
    (id === undefined || (routeEventId === undefined && state.missing === id));
  const { load } = events;

  useEffect(() => {
    if (id === undefined || organizationId === undefined) {
      return;
    }
    let current = true;
    api
      .GET("/api/v1/events/{event_id}", { params: { path: { event_id: id } } })
      .then(({ data, response }) => {
        if (!current) {
          return;
        }
        if (data) {
          setEvent(data);
          setState({ organizationId, remembered: id, missing: undefined });
          writeRemembered(eventKey(organizationId), id);
        } else if (response.status === 404 || response.status === 403) {
          setEvent(undefined);
          // Only the remembered event is forgotten, not the one that a wrong link names.
          const forget = readRemembered(eventKey(organizationId)) === id;
          if (forget) {
            writeRemembered(eventKey(organizationId), undefined);
          }
          setState((old) => ({
            organizationId,
            remembered: forget ? undefined : old.remembered,
            missing: id,
          }));
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, id, organizationId]);

  useEffect(() => {
    if (needsFallback) {
      load();
    }
  }, [needsFallback, load]);

  if (organizationId === undefined) {
    return { event: undefined, eventId: undefined, pending: false };
  }
  const resolved = event !== undefined && event.id === id ? event : undefined;
  // A route event that the server does not find has no scoped event: the page says so.
  const fallback =
    resolved === undefined && needsFallback && events.list.kind === "loaded"
      ? mostRecentOpen(events.list.events)
      : undefined;
  const scoped = resolved ?? fallback;
  const waitsForEvent = id !== undefined && resolved === undefined && state.missing !== id;
  const waitsForList =
    needsFallback && (events.list.kind === "idle" || events.list.kind === "loading");
  const pending = scoped === undefined && (waitsForEvent || waitsForList);
  return {
    event: scoped,
    eventId: scoped?.id ?? (pending ? id : undefined),
    pending,
  };
}

export type EventList =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "loaded"; events: Event[] };

/**
 * Every page of the events of the member, so that the switcher and the palette miss none. `load`
 * asks again each time; a list that is there stays until the new one comes, so the menu does not
 * flicker.
 */
export function useEventList(
  api: Api,
  organizationId: string | undefined,
): { list: EventList; load: () => void } {
  const [list, setList] = useState<EventList>({ kind: "idle" });
  const asking = useRef(false);
  // Counts the organizations; an answer of an older one is dropped.
  const generation = useRef(0);

  // A list of another organization is no list of this one, and neither is a request that still runs.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the organization is a trigger only
  useEffect(() => {
    generation.current += 1;
    asking.current = false;
    setList({ kind: "idle" });
  }, [organizationId]);

  const load = useCallback(() => {
    if (organizationId === undefined || asking.current) {
      return;
    }
    asking.current = true;
    const mine = generation.current;
    const current = () => generation.current === mine;
    setList((old) => (old.kind === "loaded" ? old : { kind: "loading" }));
    loadAll((cursor) =>
      api.GET("/api/v1/events", {
        params: { query: { limit: LARGEST_PAGE, ...(cursor && { cursor }) } },
      }),
    )
      .then((result) => {
        if (current()) {
          setList((old) =>
            "items" in result
              ? { kind: "loaded", events: result.items }
              : old.kind === "loaded"
                ? old
                : { kind: "failed", message: problemMessage(result.error) },
          );
        }
      })
      .catch(() => {
        if (current()) {
          setList((old) =>
            old.kind === "loaded" ? old : { kind: "failed", message: problemMessage(undefined) },
          );
        }
      })
      .finally(() => {
        if (current()) {
          asking.current = false;
        }
      });
  }, [api, organizationId]);

  return { list, load };
}
