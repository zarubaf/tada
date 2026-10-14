import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import * as r from "../test/workRecords";
import { itemKey, lockSelection, useWorkSelection } from "./useWorkSelection";
import { toWorkItem, type WorkItem } from "./workItem";

const make = (id: string, over = {}) =>
  toWorkItem("action", r.action(id, over) as never, () => "") as WorkItem;

describe("useWorkSelection", () => {
  it("starts empty again for another event", () => {
    const shown = [make("ACT-001")];
    const { result, rerender } = renderHook(({ event }) => useWorkSelection(event, shown, true), {
      initialProps: { event: "e1" },
    });
    act(() => result.current.set(shown, true));
    expect(result.current.items).toHaveLength(1);
    rerender({ event: "e2" });
    expect(result.current.items).toHaveLength(0);
    expect(result.current.selecting).toBe(false);
  });

  it("keeps the newest version of a selected record and drops one that is gone", () => {
    const first = make("ACT-001");
    const second = make("ACT-002");
    const { result, rerender } = renderHook(({ shown }) => useWorkSelection("e1", shown, true), {
      initialProps: { shown: [first, second] },
    });
    act(() => result.current.set([first, second], true));
    const newer = { ...first, version: 5 } as WorkItem;
    rerender({ shown: [newer] });
    expect(result.current.items.map((item) => item.version)).toEqual([5]);
    expect(result.current.isSelected(second)).toBe(false);
  });

  it("does not select what cannot be changed", () => {
    const locked = make("ACT-003", { can_change: false });
    const { result } = renderHook(() => useWorkSelection("e1", [locked], true));
    act(() => result.current.set([locked], true));
    expect(result.current.selecting).toBe(false);
    expect(result.current.canSelect(locked)).toBe(false);
  });

  it("holds the selection while it is locked", () => {
    const shown = [make("ACT-001"), make("ACT-002")];
    const { result } = renderHook(() => useWorkSelection("e1", shown, true));
    act(() => result.current.set(shown, true));
    const locked = lockSelection(result.current);
    act(() => {
      locked.clear();
      locked.set(shown, false);
      locked.keepOnly(new Set());
    });
    expect(locked.locked).toBe(true);
    expect(result.current.items).toHaveLength(2);
  });

  it("does not prune a record that leaves the list while it is locked, but once it is not", () => {
    const first = make("ACT-001");
    const second = make("ACT-002");
    const { result, rerender } = renderHook(
      ({ shown, locked }) => useWorkSelection("e1", shown, true, locked),
      { initialProps: { shown: [first, second], locked: false } },
    );
    act(() => result.current.set([first, second], true));
    rerender({ shown: [first], locked: true });
    expect(result.current.items.map(itemKey)).toEqual([itemKey(first), itemKey(second)]);
    rerender({ shown: [first], locked: false });
    expect(result.current.items.map(itemKey)).toEqual([itemKey(first)]);
    // The record is gone for good: it does not come back with the list.
    rerender({ shown: [first, second], locked: false });
    expect(result.current.items.map(itemKey)).toEqual([itemKey(first)]);
  });

  it("lists the selection in the order of the list, not of the clicks", () => {
    const shown = [make("ACT-001"), make("ACT-002"), make("ACT-003")];
    const { result } = renderHook(() => useWorkSelection("e1", shown, true));
    act(() => {
      result.current.set([shown[2] as WorkItem], true);
      result.current.set([shown[0] as WorkItem], true);
    });
    expect(result.current.items.map((i) => i.readableId)).toEqual(["ACT-001", "ACT-003"]);
  });
});
