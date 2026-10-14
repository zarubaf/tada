// Where a card or a row of the „Arbeit“ page leads (ADR 0082). A kind that has a panel opens in it,
// named by the hash. Until the other kinds have one, a record opens on the old page of its kind,
// which scrolls to the row that the hash names. When a kind gets its panel, set its row in `PANEL`
// to true; the old page of the kind then goes away with its page.
import { useCallback } from "react";
import type { RecordKind } from "../api/client";
import { useSearch } from "../router/Router";
import { NEW_PARAM } from "./record/recordRoute";
import type { WorkItem } from "./workItem";

/** True for a kind whose records open in the side panel. */
export const PANEL: Record<RecordKind, boolean> = {
  action: true,
  commitment: true,
  milestone: true,
  decision: true,
  risk: true,
};

/** True if the record opens in the side panel. */
export const opensInPanel = (item: Pick<WorkItem, "kind">): boolean => PANEL[item.kind];

/** The old page of each kind. A risk has none: it opens in the panel. */
const PAGES: Record<RecordKind, string | undefined> = {
  action: "actions",
  commitment: "commitments",
  milestone: "milestones",
  decision: "decisions",
  risk: undefined,
};

/**
 * The address of a record, or nothing for a kind without a page. `query` is the query string of
 * the page without `?`; a link to the panel keeps it, so that the view and the filters stay.
 */
export function recordHref(
  eventId: string,
  item: Pick<WorkItem, "kind" | "readableId">,
  query = "",
): string | undefined {
  const event = `/events/${encodeURIComponent(eventId)}`;
  if (PANEL[item.kind]) {
    return `${event}/work${query === "" ? "" : `?${query}`}#${item.readableId}`;
  }
  const page = PAGES[item.kind];
  return page === undefined ? undefined : `${event}/${page}#${item.readableId}`;
}

/** `recordHref` for the current query of the page, without the key that opens the create state. */
export function useRecordHref(eventId: string) {
  const search = useSearch();
  const params = new URLSearchParams(search.toString());
  params.delete(NEW_PARAM);
  const query = params.toString();
  return useCallback(
    (item: Pick<WorkItem, "kind" | "readableId">) => recordHref(eventId, item, query),
    [eventId, query],
  );
}
