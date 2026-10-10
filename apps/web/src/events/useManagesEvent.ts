import { useEffect, useState } from "react";
import type { Api } from "../api/client";

/**
 * True if the member manages the event, for example to approve documents or to close it. Only an
 * event manager reads the event memberships (ADR 0052), and an organization owner or admin acts as
 * event manager, so a list that loads means the right to manage.
 * A failure hides the control; the server checks the right again.
 */
export function useManagesEvent(api: Api, eventId: string): boolean {
  const [can, setCan] = useState(false);
  useEffect(() => {
    let current = true;
    api
      .GET("/api/v1/events/{event_id}/memberships", { params: { path: { event_id: eventId } } })
      .then(({ data }) => current && setCan(data !== undefined))
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, eventId]);
  return can;
}
