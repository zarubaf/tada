import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import type { BoardColumn } from "./recordStatus";
import {
  countsFor,
  filterWork,
  groupByColumn,
  NO_FILTERS,
  parseFilters,
  serializeFilters,
  sortWork,
  type WorkFilters,
} from "./workFilters";
import type { WorkItem } from "./workItem";

const ctx = { me: "u1", today: "2030-05-15" }; // a Wednesday

let n = 0;
function item(
  kind: RecordKind,
  over: {
    readableId?: string;
    title?: string;
    column?: BoardColumn;
    owner?: string;
    date?: string;
    workstreamId?: string;
  } = {},
): WorkItem {
  n += 1;
  return {
    kind,
    id: `id${n}`,
    readableId: over.readableId ?? `X-${String(n).padStart(3, "0")}`,
    title: over.title ?? `Titel ${n}`,
    status: "open",
    column: over.column ?? "open",
    owner: { id: over.owner ?? "u2", name: "" },
    date: over.date,
    workstreamId: over.workstreamId,
    version: 1,
    redacted: false,
    record: {},
  } as unknown as WorkItem;
}

const filters = (over: Partial<WorkFilters>): WorkFilters => ({ ...NO_FILTERS, ...over });
const ids = (items: WorkItem[]) => items.map((i) => i.readableId);

describe("filterWork", () => {
  const a = item("action", { readableId: "ACT-001", owner: "u1", date: "2030-05-10" });
  const b = item("action", {
    readableId: "ACT-002",
    column: "done",
    date: "2030-05-10",
    workstreamId: "w1",
  });
  const c = item("commitment", { readableId: "COM-001", date: "2030-05-17" });
  const d = item("decision", { readableId: "DEC-001", date: "2030-01-01", owner: "u3" });
  const e = item("risk", { readableId: "RSK-001", title: "Regen am Fest" });
  const f = item("milestone", { readableId: "MIL-001", date: "2030-05-20", column: "canceled" });
  const all = [a, b, c, d, e, f];

  it("keeps everything without filters", () => {
    expect(filterWork(all, NO_FILTERS, ctx)).toEqual(all);
  });

  it("filters by kind and by column", () => {
    expect(ids(filterWork(all, filters({ kinds: ["action", "risk"] }), ctx))).toEqual([
      "ACT-001",
      "ACT-002",
      "RSK-001",
    ]);
    expect(ids(filterWork(all, filters({ columns: ["done", "canceled"] }), ctx))).toEqual([
      "ACT-002",
      "MIL-001",
    ]);
  });

  it("filters by owner, with mine as the signed-in member", () => {
    expect(ids(filterWork(all, filters({ owners: ["mine"] }), ctx))).toEqual(["ACT-001"]);
    expect(ids(filterWork(all, filters({ owners: ["mine", "u3"] }), ctx))).toEqual([
      "ACT-001",
      "DEC-001",
    ]);
  });

  it("filters by workstream", () => {
    expect(ids(filterWork(all, filters({ workstreams: ["w1"] }), ctx))).toEqual(["ACT-002"]);
  });

  it("filters overdue: before today, still open, and no decision", () => {
    expect(ids(filterWork(all, filters({ due: "overdue" }), ctx))).toEqual(["ACT-001"]);
  });

  it("filters this week: from today to Sunday, still open", () => {
    expect(ids(filterWork(all, filters({ due: "week" }), ctx))).toEqual(["COM-001"]);
    const sunday = item("action", { readableId: "ACT-009", date: "2030-05-19" });
    const monday = item("action", { readableId: "ACT-010", date: "2030-05-20" });
    expect(ids(filterWork([c, sunday, monday], filters({ due: "week" }), ctx))).toEqual([
      "COM-001",
      "ACT-009",
    ]);
  });

  it("filters the records without a due date", () => {
    expect(ids(filterWork(all, filters({ due: "none" }), ctx))).toEqual(["DEC-001", "RSK-001"]);
  });

  it("finds a readable id typed without the dash", () => {
    expect(ids(filterWork(all, filters({ text: "act001" }), ctx))).toEqual(["ACT-001"]);
    expect(ids(filterWork(all, filters({ text: "ACT 002" }), ctx))).toEqual(["ACT-002"]);
  });

  it("searches the title and the readable id, ignoring case and accents", () => {
    expect(ids(filterWork(all, filters({ text: "regen" }), ctx))).toEqual(["RSK-001"]);
    expect(ids(filterWork(all, filters({ text: " act-00 " }), ctx))).toEqual([
      "ACT-001",
      "ACT-002",
    ]);
    const accent = item("action", { title: "Café öffnen" });
    expect(filterWork([accent], filters({ text: "CAFE OFFNEN" }), ctx)).toEqual([accent]);
  });

  it("combines the filters", () => {
    expect(ids(filterWork(all, filters({ kinds: ["action"], columns: ["open"] }), ctx))).toEqual([
      "ACT-001",
    ]);
  });
});

describe("sortWork", () => {
  it("sorts by date, the records without one last, then by readable id", () => {
    const x = item("action", { readableId: "ACT-010", date: "2030-01-02" });
    const y = item("action", { readableId: "ACT-002", date: "2030-01-02" });
    const z = item("risk", { readableId: "RSK-001" });
    const w = item("action", { readableId: "ACT-001", date: "2030-03-01" });
    const v = item("action", { readableId: "ACT-003" });
    expect(ids(sortWork([z, w, x, v, y]))).toEqual([
      "ACT-002",
      "ACT-010",
      "ACT-001",
      "ACT-003",
      "RSK-001",
    ]);
  });

  it("does not change its input", () => {
    const input = [item("risk", { readableId: "B" }), item("risk", { readableId: "A" })];
    sortWork(input);
    expect(ids(input)).toEqual(["B", "A"]);
  });
});

describe("groupByColumn", () => {
  it("has the columns of ADR 0082 in order, sorted inside", () => {
    const x = item("action", { readableId: "ACT-002" });
    const y = item("action", { readableId: "ACT-001" });
    const z = item("risk", { column: "blocked", readableId: "RSK-001" });
    const groups = groupByColumn([x, y, z]);
    expect(groups.map((g) => g.column)).toEqual([
      "open",
      "in-progress",
      "blocked",
      "done",
      "canceled",
    ]);
    expect(groups.map((g) => ids(g.items))).toEqual([
      ["ACT-001", "ACT-002"],
      [],
      ["RSK-001"],
      [],
      [],
    ]);
  });
});

describe("countsFor", () => {
  const a = item("action", { column: "open", owner: "u1" });
  const b = item("action", { column: "done", owner: "u1" });
  const c = item("risk", { column: "open", owner: "u1" });
  const d = item("risk", { column: "open", owner: "u2" });
  const all = [a, b, c, d];

  it("counts the kinds with all filters but the kind filter", () => {
    const { kinds } = countsFor(all, filters({ kinds: ["risk"], owners: ["mine"] }), ctx);
    expect(kinds).toEqual({ action: 2, commitment: 0, milestone: 0, decision: 0, risk: 1 });
  });

  it("counts the columns with all filters", () => {
    const { columns } = countsFor(all, filters({ kinds: ["risk"], owners: ["mine"] }), ctx);
    expect(columns).toEqual({ open: 1, "in-progress": 0, blocked: 0, done: 0, canceled: 0 });
  });

  it("counts everything without filters", () => {
    const { kinds, columns } = countsFor(all, NO_FILTERS, ctx);
    expect(kinds.action + kinds.risk).toBe(4);
    expect(columns.open).toBe(3);
  });
});

describe("the query string", () => {
  const full: WorkFilters = {
    kinds: ["action", "risk"],
    columns: ["blocked", "done"],
    owners: ["mine", "0199b8e0-0000-7000-8000-0000000000b2"],
    workstreams: ["w1"],
    due: "overdue",
    text: "hangar fest",
  };

  it("round-trips the filters", () => {
    const query = serializeFilters(full).toString();
    expect(parseFilters(new URLSearchParams(query))).toEqual(full);
  });

  it("reads a repeated parameter and a comma list alike, and writes it back", () => {
    const repeated = parseFilters(new URLSearchParams("type=action&type=risk&owner=a,b&owner=c"));
    expect(repeated).toMatchObject({ kinds: ["action", "risk"], owners: ["a", "b", "c"] });
    expect(parseFilters(serializeFilters(repeated))).toEqual(repeated);
  });

  it("leaves the defaults out", () => {
    expect(serializeFilters(NO_FILTERS).toString()).toBe("");
    expect(parseFilters(new URLSearchParams())).toEqual(NO_FILTERS);
  });

  it("ignores unknown and malformed values and other keys", () => {
    const params = new URLSearchParams(
      "type=action,wish,action&column=nope&owner=a b,ok&due=never&q=&view=timeline&stream=,",
    );
    expect(parseFilters(params)).toEqual({ ...NO_FILTERS, kinds: ["action"], owners: ["ok"] });
  });

  it("puts the kinds and columns in the order of the tables", () => {
    const parsed = parseFilters(new URLSearchParams("type=risk,action&column=done,open"));
    expect(parsed.kinds).toEqual(["action", "risk"]);
    expect(parsed.columns).toEqual(["open", "done"]);
  });
});
