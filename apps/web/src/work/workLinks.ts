// Where a card of the „Arbeit“ page leads. Until the side panel exists (ADR 0082), a record opens
// on the old page of its kind, which scrolls to the row that the hash names.
import type { RecordKind } from "../api/client";
import type { WorkItem } from "./workItem";

/** The old page of each kind. A risk has none: the risk register was never built. */
const PAGES: Record<RecordKind, string | undefined> = {
  action: "actions",
  commitment: "commitments",
  milestone: "milestones",
  decision: "decisions",
  risk: undefined,
};

/** The address of a record, or nothing for a kind without a page. */
export function recordHref(
  eventId: string,
  item: Pick<WorkItem, "kind" | "readableId">,
): string | undefined {
  const page = PAGES[item.kind];
  return page === undefined
    ? undefined
    : `/events/${encodeURIComponent(eventId)}/${page}#${item.readableId}`;
}
