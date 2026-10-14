// The events that the shell shows: the current one for the switcher and the title, and the list
// for the switcher menu and the command palette. Both use the events API as the events page does.
import { useCallback, useEffect, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { EVENT_KEY, readRemembered, writeRemembered } from "./remembered";

/**
 * The current event: the one of the route, else the one the member opened last (kept in the
 * browser). An event the server does not find is forgotten. It loads again when the organization
 * changes.
 */
export function useCurrentEvent(
  api: Api,
  routeEventId: string | undefined,
  organizationId: string | undefined,
): Event | undefined {
  const [remembered, setRemembered] = useState(() => readRemembered(EVENT_KEY));
  const [event, setEvent] = useState<Event>();
  const id = routeEventId ?? remembered;

  // biome-ignore lint/correctness/useExhaustiveDependencies: the organization is a trigger only
  useEffect(() => {
    if (id === undefined) {
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
          setRemembered(id);
          writeRemembered(EVENT_KEY, id);
        } else if (response.status === 404 || response.status === 403) {
          setEvent(undefined);
          setRemembered(undefined);
          writeRemembered(EVENT_KEY, undefined);
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, id, organizationId]);

  return event?.id === id ? event : undefined;
}

export type EventList =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "loaded"; events: Event[] };

/** The first page of the events of the member, loaded on the first call of `load`. */
export function useEventList(
  api: Api,
  organizationId: string | undefined,
): { list: EventList; load: () => void } {
  const [list, setList] = useState<EventList>({ kind: "idle" });

  // A list of another organization is no list of this one.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the organization is a trigger only
  useEffect(() => setList({ kind: "idle" }), [organizationId]);

  const load = useCallback(() => {
    if (list.kind === "loading" || list.kind === "loaded") {
      return;
    }
    setList({ kind: "loading" });
    api
      .GET("/api/v1/events", { params: { query: {} } })
      .then(({ data, error }) =>
        setList(
          data
            ? { kind: "loaded", events: data.items }
            : { kind: "failed", message: problemMessage(error) },
        ),
      )
      .catch(() => setList({ kind: "failed", message: problemMessage(undefined) }));
  }, [api, list.kind]);

  return { list, load };
}
