import { useCallback, useMemo } from "react";
import { useSearch, useSetSearch } from "../router/Router";
import { BOARD_COLUMNS, type BoardColumn } from "./recordStatus";
import type { WorkItem } from "./workItem";
import type { WorkView } from "./workView";

/**
 * How the board shows the work, apart from the filters. It lives in the query string too, so that
 * a reload or a step back from a record opens the same view (ADR 0082).
 */
export interface BoardView {
  /** The status that the list on a phone shows; nothing means the first one that has records. */
  column: BoardColumn | undefined;
  /** True when the column „Abgebrochen“ is open. */
  canceled: boolean;
}

const COLUMN = "column";
const CANCELED = "canceled";

/** Reads the view from a query string. What it does not know, it ignores. */
export function parseBoardView(params: Pick<URLSearchParams, "get">): BoardView {
  const column = params.get(COLUMN);
  return {
    column: BOARD_COLUMNS.find((candidate) => candidate === column),
    canceled: params.get(CANCELED) === "1",
  };
}

export interface ColumnGroup {
  column: BoardColumn;
  items: WorkItem[];
}

/**
 * The status that the phone list shows. Without a choice it is the first status that has records,
 * so that a search that matches only done records does not open on an empty „Offen“.
 */
export function chosenGroup(groups: ColumnGroup[], view: BoardView): ColumnGroup | undefined {
  const chosen = view.column ?? (groups.find((g) => g.items.length > 0) ?? groups[0])?.column;
  return groups.find((candidate) => candidate.column === chosen);
}

/**
 * The records that the page shows right now. On the board that is the list of the chosen status on
 * a phone, else every column but a collapsed „Abgebrochen“. The table lists every record, on a
 * phone too. The timeline shows none that can be selected. The one answer for what is in sight, so
 * that the selection and „Alle auswählen“ agree with the screen.
 */
export function visibleItems(
  groups: ColumnGroup[],
  view: BoardView,
  narrow: boolean,
  workView: WorkView = "board",
): WorkItem[] {
  if (workView === "timeline") {
    return [];
  }
  if (workView === "table") {
    return groups.flatMap((g) => g.items);
  }
  return narrow
    ? (chosenGroup(groups, view)?.items ?? [])
    : groups.filter((g) => g.column !== "canceled" || view.canceled).flatMap((g) => g.items);
}

/** The view and a function that changes parts of it. A change replaces the history entry. */
export function useBoardView(): [BoardView, (change: Partial<BoardView>) => void] {
  const search = useSearch();
  const setSearch = useSetSearch();
  const view = useMemo(() => parseBoardView(search), [search]);
  const set = useCallback(
    (change: Partial<BoardView>) => {
      const update: Record<string, string | null> = {};
      if ("column" in change) {
        update[COLUMN] = change.column ?? null;
      }
      if ("canceled" in change) {
        update[CANCELED] = change.canceled ? "1" : null;
      }
      setSearch(update, { replace: true });
    },
    [setSearch],
  );
  return [view, set];
}
