// The work records of an event for the „Arbeit“ page: every page of the five kinds, loaded at the
// same time, each kind with its own state (ADR 0082). A kind that fails shows a partial error with
// a retry, and the other kinds still show. The people and the workstreams come from the directory
// of the event, so the page loads them once.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Api, type Problem, problemMessage, type RecordKind } from "../api/client";
import { loadAll } from "../registers/loadAll";
import { type Directory, type DirectoryState, query, useDirectory } from "./directory";
import { RECORD_KINDS } from "./recordStatus";
import { listRisks } from "./risks";
import { createWorkCommands, type WorkCommands } from "./workCommands";
import { type RecordOf, toWorkItem, type WorkItem } from "./workItem";

export type KindState<R> =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; records: R[] };

export type KindStates = { [K in RecordKind]: KindState<RecordOf[K]> };

export interface Work {
  directory: DirectoryState;
  kinds: KindStates;
  /** The records of the kinds that loaded, mapped. Unsorted: see `sortWork`. */
  items: WorkItem[];
  commands: WorkCommands;
  /** Loads the directory and the five kinds again. */
  reload: () => Promise<void>;
  /** Loads one kind again, for the retry of its banner. Resolves to true when it loaded. */
  reloadKind: (kind: RecordKind) => Promise<boolean>;
  /** Loads one kind again without the loading state, after a command. */
  refresh: (kind: RecordKind) => Promise<boolean>;
  /** Puts the record that a command returned into the list, without a request. */
  replace: <K extends RecordKind>(kind: K, record: RecordOf[K]) => void;
}

type Answer = Promise<{
  data?: { items: { id: string }[]; next_cursor?: string | null | undefined };
  error?: Problem | undefined;
}>;

/** The list call of each kind. A call gives one page. */
const LISTS: Record<RecordKind, (api: Api, eventId: string, cursor: string | undefined) => Answer> =
  {
    action: (api, event_id, cursor) =>
      api.GET("/api/v1/events/{event_id}/actions", {
        params: { path: { event_id }, query: query(cursor) },
      }),
    commitment: (api, event_id, cursor) =>
      api.GET("/api/v1/events/{event_id}/commitments", {
        params: { path: { event_id }, query: query(cursor) },
      }),
    milestone: (api, event_id, cursor) =>
      api.GET("/api/v1/events/{event_id}/milestones", {
        params: { path: { event_id }, query: query(cursor) },
      }),
    decision: (api, event_id, cursor) =>
      api.GET("/api/v1/events/{event_id}/decisions", {
        params: { path: { event_id }, query: query(cursor) },
      }),
    risk: (api, eventId, cursor) => listRisks(api, eventId, cursor),
  };

interface Slot {
  /** The event of the answer: a page that moved to another event must not show it. */
  eventId: string;
  state: KindState<unknown>;
}

const LOADING: KindState<never> = { kind: "loading" };

/** A record appears once, also when a list moves under the paging (a risk that changes its rating). */
function once<T extends { id: string }>(records: T[]): T[] {
  return [...new Map(records.map((record) => [record.id, record])).values()];
}

export function useWork(api: Api, eventId: string): Work {
  const { state: directory, reload: reloadDirectory } = useDirectory(api, eventId);
  const [slots, setSlots] = useState<Partial<Record<RecordKind, Slot>>>({});
  // The newest request of a kind wins: the answer of an older load, or of another event, is dropped.
  const latest = useRef<Record<RecordKind, number>>({
    action: 0,
    commitment: 0,
    milestone: 0,
    decision: 0,
    risk: 0,
  });

  const load = useCallback(
    async (kind: RecordKind, silent: boolean) => {
      const request = ++latest.current[kind];
      const current = () => request === latest.current[kind];
      const put = (state: KindState<unknown>) =>
        setSlots((slots) => ({ ...slots, [kind]: { eventId, state } }));
      if (!silent) {
        put(LOADING);
      }
      let result: Awaited<ReturnType<typeof loadAll<{ id: string }>>>;
      try {
        result = await loadAll((cursor) => LISTS[kind](api, eventId, cursor));
      } catch {
        result = { error: undefined };
      }
      if (!current()) {
        return false;
      }
      if ("items" in result) {
        put({ kind: "loaded", records: once(result.items) });
        return true;
      }
      // A refresh that fails keeps the list on screen; the caller sees the false.
      if (!silent) {
        put({
          kind: "failed",
          message: problemMessage(result.error),
          requestId: result.error?.request_id,
        });
      }
      return false;
    },
    [api, eventId],
  );

  useEffect(() => {
    // The five kinds start together; none waits for another.
    for (const kind of RECORD_KINDS) {
      void load(kind, false);
    }
  }, [load]);

  const reload = useCallback(async () => {
    await Promise.all([reloadDirectory(), ...RECORD_KINDS.map((kind) => load(kind, false))]);
  }, [load, reloadDirectory]);
  const reloadKind = useCallback((kind: RecordKind) => load(kind, false), [load]);
  const refresh = useCallback((kind: RecordKind) => load(kind, true), [load]);

  const replace = useCallback(
    <K extends RecordKind>(kind: K, record: RecordOf[K]) =>
      setSlots((slots) => {
        const slot = slots[kind];
        if (slot?.eventId !== eventId || slot.state.kind !== "loaded") {
          return slots;
        }
        const records = (slot.state.records as RecordOf[K][]).map((r) =>
          r.id === record.id ? record : r,
        );
        return { ...slots, [kind]: { eventId, state: { kind: "loaded", records } } };
      }),
    [eventId],
  );

  const commands = useMemo(() => createWorkCommands(api, eventId), [api, eventId]);

  const kinds = useMemo(
    () =>
      Object.fromEntries(
        RECORD_KINDS.map((kind) => {
          const slot = slots[kind];
          return [kind, slot && slot.eventId === eventId ? slot.state : LOADING];
        }),
      ) as KindStates,
    [slots, eventId],
  );

  const items = useMemo(() => {
    const nameOf = nameFunction(directory);
    return RECORD_KINDS.flatMap((kind) => {
      const state = kinds[kind];
      return state.kind === "loaded"
        ? (state.records as RecordOf[typeof kind][]).map(
            (record) => toWorkItem(kind, record as never, nameOf) as WorkItem,
          )
        : [];
    });
  }, [kinds, directory]);

  return { directory, kinds, items, commands, reload, reloadKind, refresh, replace };
}

/** The names show as soon as the directory has loaded; before, the owner has no name. */
function nameFunction(directory: DirectoryState): Directory["nameOf"] {
  return directory.kind === "loaded" ? directory.directory.nameOf : () => "";
}
