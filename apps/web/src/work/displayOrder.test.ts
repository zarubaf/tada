import { describe, expect, it } from "vitest";
import * as r from "../test/workRecords";
import { displayOrder, neighbors } from "./displayOrder";
import { toWorkItem, type WorkItem } from "./workItem";

const nameOf = () => "";
const act = (id: string, over: Record<string, unknown>) =>
  toWorkItem("action", r.action(id, over) as never, nameOf) as WorkItem;

const items = [
  act("ACT-001", { status: "done", due_date: "2030-01-01" }),
  act("ACT-002", { status: "open", due_date: "2030-03-01" }),
  act("ACT-003", { status: "open", due_date: "2030-02-01" }),
  act("ACT-004", { status: "canceled" }),
  act("ACT-005", { status: "in-progress", due_date: "2030-05-01" }),
];
const ids = (list: WorkItem[]) => list.map((item) => item.readableId);
const noView = { column: undefined, canceled: false };
const area = () => undefined;

describe("displayOrder", () => {
  it("follows the board column by column, each column by due date, without a collapsed column", () => {
    const order = displayOrder({
      items,
      view: "board",
      boardView: noView,
      narrow: false,
      sort: { key: "due", dir: "asc" },
      areaOf: area,
    });
    expect(ids(order)).toEqual(["ACT-003", "ACT-002", "ACT-005", "ACT-001"]);
  });

  it("includes the canceled column once it is open", () => {
    const order = displayOrder({
      items,
      view: "board",
      boardView: { column: undefined, canceled: true },
      narrow: false,
      sort: { key: "due", dir: "asc" },
      areaOf: area,
    });
    expect(ids(order)).toEqual(["ACT-003", "ACT-002", "ACT-005", "ACT-001", "ACT-004"]);
  });

  it("shows only the chosen status on a phone", () => {
    const order = displayOrder({
      items,
      view: "board",
      boardView: { column: "open", canceled: false },
      narrow: true,
      sort: { key: "due", dir: "asc" },
      areaOf: area,
    });
    expect(ids(order)).toEqual(["ACT-003", "ACT-002"]);
  });

  it("follows the sort of the table, also a descending one", () => {
    const order = displayOrder({
      items,
      view: "table",
      boardView: noView,
      narrow: false,
      sort: { key: "id", dir: "desc" },
      areaOf: area,
    });
    expect(ids(order)).toEqual(["ACT-005", "ACT-004", "ACT-003", "ACT-002", "ACT-001"]);
  });

  it("has no order in the timeline", () => {
    expect(
      displayOrder({
        items,
        view: "timeline",
        boardView: noView,
        narrow: false,
        sort: { key: "due", dir: "asc" },
        areaOf: area,
      }),
    ).toEqual([]);
  });
});

describe("neighbors", () => {
  const order = items;
  it("gives the records before and after", () => {
    expect(neighbors(order, "ACT-002")).toEqual({
      previous: "ACT-001",
      next: "ACT-003",
    });
  });

  it("has no previous record at the start and no next one at the end", () => {
    expect(neighbors(order, "ACT-001")).toEqual({ previous: undefined, next: "ACT-002" });
    expect(neighbors(order, "ACT-005")).toEqual({ previous: "ACT-004", next: undefined });
  });

  it("has none for a record that is not in the order", () => {
    expect(neighbors(order, "ACT-099")).toEqual({ previous: undefined, next: undefined });
  });
});
