// The selection of the „Arbeit“ page: a set of record keys (kind and ID) that lives in the page and
// not in the address (ADR 0082). A record that leaves the list that shows, or that the member may
// not change, leaves the selection too, so the bulk bar never acts on a record that is out of sight.
import { type KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RecordKind } from "../api/client";
import type { WorkItem } from "./workItem";
import { canEdit } from "./workPermissions";

/** The key of a record in the selection. An ID is unique within its kind only. */
export function itemKey(item: { kind: RecordKind; id: string }): string {
  return `${item.kind}:${item.id}`;
}

export interface WorkSelection {
  /** True if a selection exists: every card then shows its checkbox. */
  selecting: boolean;
  /**
   * The selected records, as the list shows them now (with their newest version), in the order of
   * the list: on the board that is column by column, not the order of the clicks.
   */
  items: WorkItem[];
  /** True while a bulk change runs: the selection holds still and the boxes are off. */
  locked: boolean;
  /** True if the member may select the record: it can be changed. */
  canSelect: (item: WorkItem) => boolean;
  isSelected: (item: WorkItem) => boolean;
  /** Selects or deselects the records that can be selected; the others are ignored. */
  set: (items: readonly WorkItem[], selected: boolean) => void;
  /** Keeps only the records with these keys, for the records that failed to change. */
  keepOnly: (keys: ReadonlySet<string>) => void;
  clear: () => void;
}

/**
 * @param visible The records in sight: what the filters let through and the board shows now.
 * @param writable False for a closed event (`canEdit`).
 * @param locked True while a bulk change runs: the selection is not pruned and shows the records
 *   of the moment it was locked, so that the result of the run meets the same records.
 */
export function useWorkSelection(
  eventId: string,
  visible: readonly WorkItem[],
  writable: boolean,
  locked = false,
): WorkSelection {
  const [keys, setKeys] = useState<ReadonlySet<string>>(new Set());
  const live = useMemo(
    () => new Map(visible.filter((item) => canEdit(item, writable)).map((i) => [itemKey(i), i])),
    [visible, writable],
  );
  const held = useRef(live);
  if (!locked) {
    held.current = live;
  }
  const selectable = locked ? held.current : live;
  const items = useMemo(
    () => [...selectable].filter(([key]) => keys.has(key)).map(([, item]) => item),
    [selectable, keys],
  );

  // What left the list leaves the selection, so it does not come back with the filter.
  useEffect(() => {
    if (!locked && items.length !== keys.size) {
      setKeys(new Set(items.map(itemKey)));
    }
  }, [items, keys, locked]);

  // Another event is another selection.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the event is the trigger
  useEffect(() => setKeys(new Set()), [eventId]);

  const set = useCallback((targets: readonly WorkItem[], selected: boolean) => {
    setKeys((current) => {
      const next = new Set(current);
      for (const item of targets) {
        next[selected ? "add" : "delete"](itemKey(item));
      }
      return next;
    });
  }, []);
  const keepOnly = useCallback(
    (keep: ReadonlySet<string>) =>
      setKeys((current) => new Set([...current].filter((k) => keep.has(k)))),
    [],
  );
  const clear = useCallback(() => setKeys(new Set()), []);

  return useMemo(
    () => ({
      selecting: items.length > 0,
      locked,
      items,
      canSelect: (item) => selectable.has(itemKey(item)),
      isSelected: (item) => keys.has(itemKey(item)) && selectable.has(itemKey(item)),
      set: (targets, selected) =>
        set(
          targets.filter((i) => selectable.has(itemKey(i))),
          selected,
        ),
      keepOnly,
      clear,
    }),
    [items, selectable, keys, locked, set, keepOnly, clear],
  );
}

const NOTHING = () => {};

/** The same selection with its changes off, for as long as a bulk change runs on it. */
export function lockSelection(selection: WorkSelection): WorkSelection {
  return { ...selection, locked: true, set: NOTHING, keepOnly: NOTHING, clear: NOTHING };
}

/**
 * The keys of a selection while focus is inside the list: Ctrl+A or Cmd+A selects what shows, and
 * Escape clears the selection. Ctrl+A is the browser's unless it selects something here.
 *
 * @param inSight The records that show now, as `visibleItems` answers.
 */
export function selectionKeys(selection: WorkSelection | undefined, inSight: () => WorkItem[]) {
  return (event: KeyboardEvent) => {
    if (!selection || event.defaultPrevented) {
      return;
    }
    if (event.key === "Escape" && selection.selecting) {
      selection.clear();
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      const selectable = inSight().filter(selection.canSelect);
      if (!selection.locked && selectable.length > 0) {
        event.preventDefault();
        selection.set(selectable, true);
      }
    }
  };
}
