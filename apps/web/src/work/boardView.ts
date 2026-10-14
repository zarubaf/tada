import { useCallback, useMemo } from "react";
import { useSearch, useSetSearch } from "../router/Router";
import { BOARD_COLUMNS, type BoardColumn } from "./recordStatus";

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
