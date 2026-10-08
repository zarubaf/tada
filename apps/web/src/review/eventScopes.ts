import { useEffect, useMemo, useState } from "react";
import type { Api, Event } from "../api/client";
import { t } from "../i18n";

/** What the inbox calls a changeset and in which zone it shows times. */
export interface Scopes {
  /** The key and the name of the event, or „Organisation“ for a changeset without an event. */
  title: (eventId: string | null | undefined) => string;
  /** The IANA time zone of the event. Without an event it is the zone of the browser. */
  timeZone: (eventId: string | null | undefined) => string;
}

const BROWSER_ZONE = new Intl.DateTimeFormat().resolvedOptions().timeZone;

/**
 * Loads the events of the member once. A changeset names only the ID of its event, so the inbox
 * looks the key, the name and the time zone up here. A failed load leaves the generic names.
 */
export function useEventScopes(api: Api): Scopes {
  const [events, setEvents] = useState<Map<string, Event>>(new Map());
  useEffect(() => {
    let current = true;
    void api
      .GET("/api/v1/events", { params: { query: { limit: 200 } } })
      .then(({ data }) => {
        if (current && data) {
          setEvents(new Map(data.items.map((event) => [event.id, event])));
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api]);

  return useMemo(
    () => ({
      title: (eventId) => {
        if (!eventId) {
          return t("inbox-scope-organization");
        }
        const event = events.get(eventId);
        return event ? `${event.key} ${event.name}` : t("inbox-event-unknown");
      },
      timeZone: (eventId) => (eventId && events.get(eventId)?.time_zone) || BROWSER_ZONE,
    }),
    [events],
  );
}
