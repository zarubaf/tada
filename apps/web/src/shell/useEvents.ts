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

/** The newest event that is not closed, or none. */
export function mostRecentOpen(events: readonly Event[]): Event | undefined {
  let newest: Event | undefined;
  for (const event of events) {
    if (event.status !== "closed" && (!newest || event.created_at > newest.created_at)) {
      newest = event;
    }
  }
  return newest;
}

/**
 * The scoped event, the one that the event items of the shell act on (ADR 0084): the event of
 * the route, else the one the member opened last in this organization (kept in the browser),
 * else the most recent open event, else none. The route event that the server does not find
 * leaves the remembered one alone; a remembered event that the server does not find is
 * forgotten. Without an organization (a page without the frame) it asks for nothing.
 */
export function useScopedEvent(
  api: Api,
  routeEventId: string | undefined,
  organizationId: string | undefined,
  events: { list: EventList; load: () => void },
): Event | undefined {
  const [state, setState] = useState(() => lastEventOf(organizationId));
  const [event, setEvent] = useState<Event>();
  // Another organization has another last event: adjust while rendering, not in an effect.
  if (state.organizationId !== organizationId) {
    setState(lastEventOf(organizationId));
  }
  const id = routeEventId ?? state.remembered;
  const needsFallback = organizationId !== undefined && (id === undefined || state.missing === id);
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
    return undefined;
  }
  if (event !== undefined && event.id === id) {
    return event;
  }
  return needsFallback && events.list.kind === "loaded"
    ? mostRecentOpen(events.list.events)
    : undefined;
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
