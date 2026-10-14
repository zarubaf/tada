// The order in which the „Arbeit“ page shows its records, for the previous and next buttons of the
// side panel (ADR 0082): the board column by column, the table in its sort. The timeline shows no
// list of records to step through.
import { type BoardView, visibleItems } from "./boardView";
import { groupByColumn, type Sort, sortBy } from "./workFilters";
import type { WorkItem } from "./workItem";
import type { WorkView } from "./workView";

export function displayOrder({
  items,
  view,
  boardView,
  narrow,
  sort,
  areaOf,
}: {
  /** The records that the filters let through. */
  items: WorkItem[];
  view: WorkView;
  boardView: BoardView;
  narrow: boolean;
  sort: Sort;
  areaOf: (item: WorkItem) => string | undefined;
}): WorkItem[] {
  if (view === "timeline") {
    return [];
  }
  return view === "table"
    ? sortBy(items, sort, areaOf)
    : visibleItems(groupByColumn(items), boardView, narrow, "board");
}

/**
 * The readable IDs before and after a record in `order`, and its place there (counting from one);
 * none for a record outside of it.
 */
export function neighbors(
  order: readonly Pick<WorkItem, "readableId">[],
  readableId: string,
): {
  previous: string | undefined;
  next: string | undefined;
  place: { index: number; total: number } | undefined;
} {
  const index = order.findIndex((item) => item.readableId === readableId);
  return index < 0
    ? { previous: undefined, next: undefined, place: undefined }
    : {
        previous: order[index - 1]?.readableId,
        next: order[index + 1]?.readableId,
        place: { index: index + 1, total: order.length },
      };
}
