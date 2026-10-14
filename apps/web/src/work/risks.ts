// The risks of an event. A risk has a rating and a mitigation but no date (ADR 0081); the client
// reads them the way it reads the other kinds.
import type { Api } from "../api/client";
import { LARGEST_PAGE } from "../registers/loadAll";

/** One page of the risks, the worst first. The cursor comes from the page before. */
export function listRisks(api: Api, eventId: string, cursor?: string) {
  return api.GET("/api/v1/events/{event_id}/risks", {
    params: {
      path: { event_id: eventId },
      query: { limit: LARGEST_PAGE, ...(cursor && { cursor }) },
    },
  });
}

/** One risk with its evidence. */
export function getRisk(api: Api, eventId: string, id: string) {
  return api.GET("/api/v1/events/{event_id}/risks/{id}", {
    params: { path: { event_id: eventId, id } },
  });
}
