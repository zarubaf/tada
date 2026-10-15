// Where a card or a row of the „Arbeit“ page leads (ADR 0082): every kind opens in the side panel,
// named by the hash.
import { useCallback } from "react";
import { useSearch } from "../router/Router";
import { NEW_PARAM } from "./record/recordRoute";
import type { WorkItem } from "./workItem";

/**
 * The address of a record. `query` is the query string of the page without `?`; a link to the
 * panel keeps it, so that the view and the filters stay.
 */
export function recordHref(
  eventId: string,
  item: Pick<WorkItem, "readableId">,
  query = "",
): string {
  const event = `/events/${encodeURIComponent(eventId)}`;
  return `${event}/work${query === "" ? "" : `?${query}`}#${item.readableId}`;
}
/** `recordHref` for the current query of the page, without the key that opens the create state. */
export function useRecordHref(eventId: string) {
  const search = useSearch();
  const params = new URLSearchParams(search.toString());
  params.delete(NEW_PARAM);
  const query = params.toString();
  return useCallback(
    (item: Pick<WorkItem, "readableId">) => recordHref(eventId, item, query),
    [eventId, query],
  );
}
