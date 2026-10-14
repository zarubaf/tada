import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { Router } from "../router/Router";
import * as r from "../test/workRecords";
import { parseBoardView, useBoardView, visibleItems } from "./boardView";
import { groupByColumn } from "./workFilters";
import { toWorkItem, type WorkItem } from "./workItem";

const wrapper = ({ children }: { children: ReactNode }) => <Router>{children}</Router>;

afterEach(() => window.history.replaceState(null, "", "/"));

describe("parseBoardView", () => {
  it("reads the status of the phone list and the open Abgebrochen", () => {
    expect(parseBoardView(new URLSearchParams("column=done&canceled=1"))).toEqual({
      column: "done",
      canceled: true,
    });
  });

  it("has defaults, and ignores what it does not know", () => {
    expect(parseBoardView(new URLSearchParams())).toEqual({ column: undefined, canceled: false });
    expect(parseBoardView(new URLSearchParams("column=nope&canceled=yes"))).toEqual({
      column: undefined,
      canceled: false,
    });
  });
});

describe("useBoardView", () => {
  it("writes the view into the query string with replace and keeps the other keys", () => {
    window.history.replaceState(null, "", "/events/e1/work?type=risk#x");
    const { result } = renderHook(() => useBoardView(), { wrapper });
    const before = window.history.length;
    act(() => result.current[1]({ column: "blocked" }));
    expect(window.location.search).toBe("?type=risk&column=blocked");
    expect(window.location.hash).toBe("#x");
    act(() => result.current[1]({ canceled: true }));
    expect(new URLSearchParams(window.location.search).get("canceled")).toBe("1");
    expect(result.current[0]).toEqual({ column: "blocked", canceled: true });
    expect(window.history.length).toBe(before);
  });

  it("leaves a key out at its default", () => {
    window.history.replaceState(null, "", "/?column=done&canceled=1");
    const { result } = renderHook(() => useBoardView(), { wrapper });
    act(() => result.current[1]({ column: undefined, canceled: false }));
    expect(window.location.search).toBe("");
  });
});

describe("visibleItems", () => {
  const make = (id: string, status: string) =>
    toWorkItem("action", r.action(id, { status }) as never, () => "") as WorkItem;
  const groups = groupByColumn([
    make("ACT-001", "open"),
    make("ACT-002", "in-progress"),
    make("ACT-003", "done"),
    make("ACT-004", "canceled"),
  ]);
  const ids = (items: WorkItem[]) => items.map((item) => item.readableId);

  it("shows every column but the closed Abgebrochen on a wide layout", () => {
    expect(ids(visibleItems(groups, { column: undefined, canceled: false }, false))).toEqual([
      "ACT-001",
      "ACT-002",
      "ACT-003",
    ]);
    expect(ids(visibleItems(groups, { column: undefined, canceled: true }, false))).toHaveLength(4);
  });

  it("shows the chosen status on a phone, and the first one with records without a choice", () => {
    expect(ids(visibleItems(groups, { column: "done", canceled: false }, true))).toEqual([
      "ACT-003",
    ]);
    expect(ids(visibleItems(groups, { column: undefined, canceled: false }, true))).toEqual([
      "ACT-001",
    ]);
  });

  it("shows every record in the table, folded or not, wide or on a phone", () => {
    const view = { column: "done", canceled: false } as const;
    expect(ids(visibleItems(groups, view, false, "table"))).toHaveLength(4);
    expect(ids(visibleItems(groups, view, true, "table"))).toHaveLength(4);
  });

  it("shows no record in the timeline, so nothing stays selected there", () => {
    const view = { column: undefined, canceled: true };
    expect(visibleItems(groups, view, false, "timeline")).toEqual([]);
  });
});
