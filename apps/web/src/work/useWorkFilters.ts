import { useCallback, useMemo } from "react";
import { useSearch, useSetSearch } from "../router/Router";
import { FILTER_KEYS, parseFilters, serializeFilters, type WorkFilters } from "./workFilters";

/**
 * The filters of the „Arbeit“ page, kept in the query string so that a URL opens the same view
 * (ADR 0082). A change replaces the history entry: a filter is not a place to go back to. The keys
 * of the view and any other key stay as they are.
 */
export function useWorkFilters(): [WorkFilters, (next: WorkFilters) => void] {
  const search = useSearch();
  const setSearch = useSetSearch();
  const filters = useMemo(() => parseFilters(search), [search]);
  const set = useCallback(
    (next: WorkFilters) => {
      const cleared = Object.fromEntries(FILTER_KEYS.map((key) => [key, null]));
      setSearch({ ...cleared, ...Object.fromEntries(serializeFilters(next)) }, { replace: true });
    },
    [setSearch],
  );
  return [filters, set];
}
