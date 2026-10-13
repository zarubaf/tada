// The loaded rows of a register with paging, shared by the pages of workstreams, actions,
// commitments, persons and institutions.
import { useCallback, useEffect, useRef, useState } from "react";
import { type Problem, problemMessage } from "../api/client";

/**
 * The rows on screen. Each load of the first page starts a new generation; the loaded list carries
 * the generation of the load that gave it, so a next page can tell which list it extends.
 */
export type RegisterState<T> =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; items: T[]; nextCursor: string | undefined; generation: number };

/**
 * What a request for the next page did.
 * - `appended`: the page is in the list; `last` when no page follows.
 * - `failed`: the server refused or did not answer; the list is unchanged.
 * - `stale`: the request belongs to a list that is no longer the newest (a reload started), or the
 *   call came while the list had no next page or one was already on its way. Nothing changed and
 *   nothing is to be said.
 */
export type PageOutcome =
  | { kind: "appended"; last: boolean }
  | { kind: "failed"; message: string }
  | { kind: "stale" };

type Answer<T> = Promise<{
  data?: { items: T[]; next_cursor?: string | null | undefined };
  error?: Problem | undefined;
}>;

export interface Register<T> {
  state: RegisterState<T>;
  /** A next page of the newest list is on its way. */
  loadingMore: boolean;
  setLoading: () => void;
  /** Loads the first page again. Resolves to true when it loaded. */
  reload: () => Promise<boolean>;
  /** Loads the next page of the list on screen. */
  loadMore: () => Promise<PageOutcome>;
  replace: (item: T) => void;
  append: (item: T) => void;
}

const STALE: PageOutcome = { kind: "stale" };

/** The rows of a register. `fetchPage` takes the cursor of the page, or none for the first. */
export function useRegister<T extends { id: string }>(
  fetchPage: (cursor: string | undefined) => Answer<T>,
): Register<T> {
  const [state, setState] = useState<RegisterState<T>>({ kind: "loading" });
  const [loadingMore, setLoadingMore] = useState(false);
  // The generation of the newest load of the first page. An answer of an older one is dropped.
  const latest = useRef(0);
  // The generation and the next cursor of the list on screen, read at the time of a call (a caller
  // may hold the state of an earlier render).
  const shown = useRef<{ generation: number; nextCursor: string | undefined }>(undefined);
  // The generation that has a request for the next page on its way: one at a time.
  const busy = useRef<number>(undefined);

  const show = useCallback((next: RegisterState<T>) => {
    shown.current =
      next.kind === "loaded"
        ? { generation: next.generation, nextCursor: next.nextCursor }
        : undefined;
    setState(next);
  }, []);

  const reload = useCallback(async () => {
    const generation = ++latest.current;
    // A next page on its way now belongs to an old list.
    setLoadingMore(false);
    let next: RegisterState<T>;
    try {
      const { data, error } = await fetchPage(undefined);
      next = data
        ? {
            kind: "loaded",
            items: data.items,
            nextCursor: data.next_cursor ?? undefined,
            generation,
          }
        : { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
    } catch {
      next = { kind: "failed", message: problemMessage(undefined), requestId: undefined };
    }
    if (generation === latest.current) {
      show(next);
    }
    return next.kind === "loaded";
  }, [fetchPage, show]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const loadMore = async (): Promise<PageOutcome> => {
    const list = shown.current;
    if (
      list === undefined ||
      list.nextCursor === undefined ||
      list.generation !== latest.current ||
      busy.current === latest.current
    ) {
      return STALE;
    }
    const { generation, nextCursor: cursor } = list;
    busy.current = generation;
    setLoadingMore(true);
    let answer: Awaited<Answer<T>> | undefined;
    try {
      answer = await fetchPage(cursor);
    } catch {
      answer = undefined;
    }
    // A reload started meanwhile: the page belongs to a list that is no longer on screen, and a
    // request of the new list may be on its way.
    if (generation !== latest.current) {
      return STALE;
    }
    busy.current = undefined;
    setLoadingMore(false);
    const data = answer?.data;
    if (!data) {
      return { kind: "failed", message: problemMessage(answer?.error) };
    }
    // A server that repeats the cursor would serve the same page forever: the list ends there.
    const last = data.next_cursor == null || data.next_cursor === cursor;
    const nextCursor = last ? undefined : (data.next_cursor ?? undefined);
    shown.current = { generation, nextCursor };
    setState((current) => {
      if (current.kind !== "loaded") {
        return current;
      }
      // A row appears once, even when the server serves it again.
      const known = new Set(current.items.map((row) => row.id));
      const added = data.items.filter((row) => !known.has(row.id));
      return { ...current, items: [...current.items, ...added], nextCursor };
    });
    return { kind: "appended", last };
  };

  const update = (change: (items: T[]) => T[]) =>
    setState((current) =>
      current.kind === "loaded" ? { ...current, items: change(current.items) } : current,
    );

  return {
    state,
    loadingMore,
    setLoading: () => show({ kind: "loading" }),
    reload,
    loadMore,
    replace: (item) => update((items) => items.map((i) => (i.id === item.id ? item : i))),
    append: (item) => update((items) => [...items.filter((i) => i.id !== item.id), item]),
  };
}
