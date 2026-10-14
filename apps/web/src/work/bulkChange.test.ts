import { describe, expect, it } from "vitest";
import type { Api } from "../api/client";
import * as r from "../test/workRecords";
import { statusChoice, supports, unchanged } from "./bulkChange";
import { createWorkCommands } from "./workCommands";
import { toWorkItem, type WorkItem } from "./workItem";

const commands = createWorkCommands({} as Api, "e1");
const item = (kind: "action" | "commitment" | "decision" | "risk", record: unknown) =>
  toWorkItem(kind, record as never, () => "") as WorkItem;

const open = item(
  "action",
  r.action("ACT-001", { next_statuses: ["in-progress", "done", "canceled"] }),
);
const busy = item(
  "action",
  r.action("ACT-002", { status: "in-progress", next_statuses: ["open", "done", "canceled"] }),
);
const stuck = item("action", r.action("ACT-003", { status: "canceled", next_statuses: [] }));
const promise = item("commitment", r.commitment("COM-001", { next_statuses: ["fulfilled"] }));
const decision = item("decision", r.decision("DEC-001"));

describe("statusChoice", () => {
  it("offers the statuses that every record may take next, in the order of the first", () => {
    expect(statusChoice(commands, [open, busy])).toEqual({
      allowed: true,
      kind: "action",
      statuses: ["done", "canceled"],
    });
  });

  it("is blocked for records of several kinds", () => {
    expect(statusChoice(commands, [open, promise])).toEqual({ allowed: false, reason: "mixed" });
  });

  it("is blocked for decisions, which have no status command", () => {
    expect(statusChoice(commands, [decision])).toEqual({ allowed: false, reason: "no-command" });
  });

  it("is blocked when no status is common", () => {
    expect(statusChoice(commands, [open, stuck])).toEqual({
      allowed: false,
      reason: "none-common",
    });
  });
});

describe("supports", () => {
  it("asks the commands, so a command that the API lacks is not offered", () => {
    expect(supports(commands, [open, decision], "owner")).toBe(true);
    expect(supports(commands, [open, decision], "area")).toBe(true);
    expect(supports(commands, [open, decision], "status")).toBe(false);
  });
});

describe("unchanged", () => {
  it("knows a record that has the value already", () => {
    expect(unchanged(open, { field: "owner", userId: open.owner.id })).toBe(true);
    expect(unchanged(open, { field: "owner", userId: "other" })).toBe(false);
    expect(unchanged(open, { field: "area", workstreamId: null })).toBe(true);
    expect(unchanged(open, { field: "area", workstreamId: "w1" })).toBe(false);
    expect(unchanged(open, { field: "status", status: "done" })).toBe(false);
  });
});
