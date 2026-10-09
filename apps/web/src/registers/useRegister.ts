// The loaded rows of a register with paging, shared by the pages of workstreams, actions,
// commitments, persons and institutions.
import { useCallback, useEffect, useRef, useState } from "react";
import { type Problem, problemMessage } from "../api/client";

export type RegisterState<T> =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; items: T[]; nextCursor: string | undefined };

type Answer<T> = Promise<{
  data?: { items: T[]; next_cursor?: string | null | undefined };
  error?: Problem | undefined;
}>;

export interface Register<T> {
  state: RegisterState<T>;
  setLoading: () => void;
  /** Loads the first page again. Resolves to true when it loaded. */
  reload: () => Promise<boolean>;
  /** Loads the next page. Resolves to the message of a failure, or to whether it was the last. */
  loadMore: () => Promise<{ failure: string } | { last: boolean }>;
  replace: (item: T) => void;
  append: (item: T) => void;
}

/** The rows of a register. `fetchPage` takes the cursor of the page, or none for the first. */
export function useRegister<T extends { id: string }>(
  fetchPage: (cursor: string | undefined) => Answer<T>,
): Register<T> {
  const [state, setState] = useState<RegisterState<T>>({ kind: "loading" });
  // The newest request wins: an older answer that arrives late is dropped.
  const latest = useRef(0);

  const reload = useCallback(async () => {
    const request = ++latest.current;
    let next: RegisterState<T>;
    try {
      const { data, error } = await fetchPage(undefined);
      next = data
        ? { kind: "loaded", items: data.items, nextCursor: data.next_cursor ?? undefined }
        : { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
    } catch {
      next = { kind: "failed", message: problemMessage(undefined), requestId: undefined };
    }
    if (request === latest.current) {
      setState(next);
    }
    return next.kind === "loaded";
  }, [fetchPage]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const loadMore = async () => {
    if (state.kind !== "loaded" || state.nextCursor === undefined) {
      return { last: true };
    }
    try {
      const { data, error } = await fetchPage(state.nextCursor);
      if (!data) {
        return { failure: problemMessage(error) };
      }
      const added = data.items;
      setState((current) =>
        current.kind === "loaded"
          ? {
              kind: "loaded",
              items: [...current.items, ...added],
              nextCursor: data.next_cursor ?? undefined,
            }
          : current,
      );
      return { last: data.next_cursor == null };
    } catch {
      return { failure: problemMessage(undefined) };
    }
  };

  const update = (change: (items: T[]) => T[]) =>
    setState((current) =>
      current.kind === "loaded" ? { ...current, items: change(current.items) } : current,
    );

  return {
    state,
    setLoading: () => setState({ kind: "loading" }),
    reload,
    loadMore,
    replace: (item) => update((items) => items.map((i) => (i.id === item.id ? item : i))),
    append: (item) => update((items) => [...items.filter((i) => i.id !== item.id), item]),
  };
}
