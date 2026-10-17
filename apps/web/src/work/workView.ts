import { useCallback, useMemo } from "react";
import { useSearch, useSetSearch } from "../router/Router";
import { parseSort, SORT_PARAMS, type Sort, serializeSort } from "./workFilters";

/** The views of the „Arbeit“ page (ADR 0082). The board is the default. */
export const WORK_VIEWS = ["board", "table", "timeline"] as const;
export type WorkView = (typeof WORK_VIEWS)[number];

const VIEW = "view";
/** The view that has no key in the address. */
export const DEFAULT_WORK_VIEW: WorkView = "board";

/** Reads the view from a query string. What it does not know, it shows as the board. */
export function parseWorkView(params: Pick<URLSearchParams, "get">): WorkView {
  const view = params.get(VIEW);
  return WORK_VIEWS.find((candidate) => candidate === view) ?? DEFAULT_WORK_VIEW;
}

/**
 * The view of the page, kept in the query key `view`; the board has no key. A change makes a new
 * history entry, because a view is a place to go back to; the filters and the sort stay.
 */
export function useWorkView(): [WorkView, (next: WorkView) => void] {
  const search = useSearch();
  const setSearch = useSetSearch();
  const view = useMemo(() => parseWorkView(search), [search]);
  const set = useCallback(
    (next: WorkView) => setSearch({ [VIEW]: next === DEFAULT_WORK_VIEW ? null : next }),
    [setSearch],
  );
  return [view, set];
}

/** How the table sorts, in the query keys `sort` and `dir`. A change replaces the history entry. */
export function useWorkSort(): [Sort, (next: Sort) => void] {
  const search = useSearch();
  const setSearch = useSetSearch();
  const sort = useMemo(() => parseSort(search), [search]);
  const set = useCallback(
    (next: Sort) => {
      const cleared = Object.fromEntries(SORT_PARAMS.map((key) => [key, null]));
      setSearch({ ...cleared, ...Object.fromEntries(serializeSort(next)) }, { replace: true });
    },
    [setSearch],
  );
  return [sort, set];
}
