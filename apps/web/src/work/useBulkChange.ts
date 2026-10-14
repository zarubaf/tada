// Runs one change on many records: different records in parallel (the commands queue the commands
// of one record), with a progress, and a list of what failed. The failed records can be tried again
// alone. A conflict loads its kind again, so that the next try uses the version of the server.
// The hook belongs to the page, not to the bar: a run goes on when the bar leaves.
import { useCallback, useEffect, useRef, useState } from "react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import { applyChange, type BulkChange, unchanged } from "./bulkChange";
import { itemKey } from "./useWorkSelection";
import type { CommandResult, FailureReason, WorkCommands } from "./workCommands";
import type { WorkItem } from "./workItem";

export interface BulkFailure {
  key: string;
  readableId: string;
  reason: FailureReason;
  /** The German message of the server, for a reason that has no short word. */
  message: string;
}

export interface BulkState {
  /** Set while the changes run. */
  progress?: { done: number; total: number };
  /** What failed in the last run. Empty when nothing failed. */
  failures: BulkFailure[];
  /** The number of records in the last run. */
  total: number;
  /** True while the kinds with a conflict load again: a retry would use an old version. */
  reloading: boolean;
}

export interface BulkOutcome {
  /** The number of records changed since the change began, retries included. */
  changed: number;
  failedKeys: ReadonlySet<string>;
}

const IDLE: BulkState = { failures: [], total: 0, reloading: false };

/** What `run` did: it began, or it changed nothing (no record needs it, or a run is not over). */
export type RunOutcome = "started" | "unchanged";

export function useBulkChange({
  scope,
  commands,
  refresh,
  items,
  onFinished,
}: {
  /** What the run belongs to, for example the event: the result of a run for another scope is dropped. */
  scope: string;
  commands: WorkCommands;
  /** Loads one kind again without a loading state; the lists keep their cards if it fails. */
  refresh: (kind: RecordKind) => Promise<boolean>;
  /** All records, as they are now: a retry reads the version after a reload from here. */
  items: readonly WorkItem[];
  onFinished: (outcome: BulkOutcome) => void;
}) {
  const [state, setState] = useState<BulkState>(IDLE);
  const latest = useRef({ items, onFinished });
  latest.current = { items, onFinished };
  // The run that is not over, with its reload. A new scope ends it for the hook: its result is dropped.
  const active = useRef<object | undefined>(undefined);
  const change = useRef<{ change: BulkChange; changed: number } | undefined>(undefined);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the scope is the trigger
  useEffect(() => {
    active.current = undefined;
    change.current = undefined;
    setState(IDLE);
  }, [scope]);

  const execute = useCallback(
    async (targets: readonly WorkItem[], first: boolean) => {
      const current = change.current;
      if (!current || active.current) {
        return;
      }
      const token = {};
      active.current = token;
      const stale = () => active.current !== token;
      // A retry keeps the failures on screen, so that its button keeps focus until the answer.
      setState((s) => ({
        progress: { done: 0, total: targets.length },
        failures: first ? [] : s.failures,
        total: targets.length,
        reloading: false,
      }));
      try {
        const results = await Promise.all(
          targets.map(async (item) => {
            const result = await applyChange(commands, item, current.change).catch(
              (): CommandResult<unknown> => ({
                ok: false,
                failure: { reason: "other", message: t("work-bulk-reason-unknown") },
              }),
            );
            if (!stale()) {
              setState((s) =>
                s.progress ? { ...s, progress: { ...s.progress, done: s.progress.done + 1 } } : s,
              );
            }
            return { item, result };
          }),
        );
        if (stale()) {
          return;
        }
        const failures: BulkFailure[] = [];
        const reload = new Set<RecordKind>();
        for (const { item, result } of results) {
          if (result.ok) {
            current.changed += 1;
          } else {
            failures.push({
              key: itemKey(item),
              readableId: item.readableId,
              reason: result.failure.reason,
              message: result.failure.message,
            });
            if (result.failure.reason === "conflict" || result.failure.reason === "not-found") {
              reload.add(item.kind);
            }
          }
        }
        setState({ failures, total: targets.length, reloading: reload.size > 0 });
        latest.current.onFinished({
          changed: current.changed,
          failedKeys: new Set(failures.map((f) => f.key)),
        });
        if (reload.size > 0) {
          await Promise.allSettled([...reload].map((kind) => refresh(kind)));
          if (!stale()) {
            setState((s) => ({ ...s, reloading: false }));
          }
        }
      } finally {
        // The reload is part of the run: no new run starts on versions that are about to change.
        if (!stale()) {
          active.current = undefined;
        }
      }
    },
    [commands, refresh],
  );

  /** Applies a new change to the records that do not have the value yet. */
  const run = useCallback(
    (targets: readonly WorkItem[], next: BulkChange): RunOutcome => {
      // The bar refuses a new change while one runs, so this is only a guard for the hook.
      if (active.current) {
        return "unchanged";
      }
      const todo = targets.filter((item) => !unchanged(item, next));
      if (todo.length === 0) {
        return "unchanged";
      }
      change.current = { change: next, changed: 0 };
      void execute(todo, true);
      return "started";
    },
    [execute],
  );

  /**
   * Tries the failed records again, with the version that the list has now. Only records in
   * `inSight` (keys of the selected records) are tried: the bar never acts on a card out of sight.
   */
  const retry = useCallback(
    (inSight: ReadonlySet<string>) => {
      const keys = new Set(state.failures.map((f) => f.key).filter((key) => inSight.has(key)));
      void execute(
        latest.current.items.filter((item) => keys.has(itemKey(item))),
        false,
      );
    },
    [execute, state.failures],
  );

  /** Forgets the last run, for example when its selection is gone. A run that goes on is not touched. */
  const reset = useCallback(() => {
    if (!active.current) {
      change.current = undefined;
      setState(IDLE);
    }
  }, []);

  return { state, run, retry, reset };
}

/** What the page and the bar share of a bulk change. */
export type BulkChanger = ReturnType<typeof useBulkChange>;
