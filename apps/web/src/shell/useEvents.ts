// The events that the shell shows: the current one for the switcher and the title, and the list
// for the switcher menu and the command palette. Both use the events API as the events page does.
import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { eventKey, readRemembered, writeRemembered } from "./remembered";

function lastEventOf(organizationId: string | undefined) {
  return {
    organizationId,
    remembered: organizationId === undefined ? undefined : readRemembered(eventKey(organizationId)),
  };
}

/**
 * The current event: the one of the route, else the one the member opened last in this
 * organization (kept in the browser). The route event that the server does not find leaves the
 * remembered one alone; a remembered event that the server does not find is forgotten. Without an
 * organization (a page without the frame) it asks for nothing.
 */
export function useCurrentEvent(
  api: Api,
  routeEventId: string | undefined,
  organizationId: string | undefined,
): Event | undefined {
  const [state, setState] = useState(() => lastEventOf(organizationId));
  const [event, setEvent] = useState<Event>();
  // Another organization has another last event: adjust while rendering, not in an effect.
  if (state.organizationId !== organizationId) {
    setState(lastEventOf(organizationId));
  }
  const id = routeEventId ?? state.remembered;

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
          setState({ organizationId, remembered: id });
          writeRemembered(eventKey(organizationId), id);
        } else if (response.status === 404 || response.status === 403) {
          setEvent(undefined);
          // Only the remembered event is forgotten, not the one that a wrong link names.
          if (readRemembered(eventKey(organizationId)) === id) {
            setState({ organizationId, remembered: undefined });
            writeRemembered(eventKey(organizationId), undefined);
          }
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, id, organizationId]);

  return organizationId !== undefined && event?.id === id ? event : undefined;
}

export type EventList =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "loaded"; events: Event[] };

/**
 * The first page of the events of the member. `load` asks again each time; a list that is there
 * stays until the new one comes, so the menu does not flicker.
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
    api
      .GET("/api/v1/events", { params: { query: {} } })
      .then(({ data, error }) => {
        if (current()) {
          setList((old) =>
            data
              ? { kind: "loaded", events: data.items }
              : old.kind === "loaded"
                ? old
                : { kind: "failed", message: problemMessage(error) },
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
