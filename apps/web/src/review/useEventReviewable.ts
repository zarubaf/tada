import { useEffect, useState } from "react";
import type { Api } from "../api/client";

/**
 * Whether the member can review proposals of the event of a changeset. A changeset carries only the
 * ID of its event, so the status comes from the event itself. Until the event loads, its status is
 * unknown (`known` is false) and nobody reviews. A changeset without an event stays reviewable. If the event does not
 * load, the server decides: it refuses a write into a closed event with `event-closed`.
 */
export function useEventReviewable(
  api: Api,
  eventId: string | null | undefined,
): { known: boolean; reviewable: boolean; closed: boolean } {
  const [loaded, setLoaded] = useState<{ eventId: string; closed: boolean | undefined }>();

  useEffect(() => {
    if (!eventId) {
      return;
    }
    let current = true;
    api
      .GET("/api/v1/events/{event_id}", { params: { path: { event_id: eventId } } })
      .then(({ data }) => current && setLoaded({ eventId, closed: data?.status === "closed" }))
      .catch(() => current && setLoaded({ eventId, closed: undefined }));
    return () => {
      current = false;
    };
  }, [api, eventId]);

  if (!eventId) {
    return { known: true, reviewable: true, closed: false };
  }
  const known = loaded?.eventId === eventId ? loaded : undefined;
  return {
    known: known !== undefined,
    reviewable: known !== undefined && known.closed !== true,
    closed: known?.closed === true,
  };
}
