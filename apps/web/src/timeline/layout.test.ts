import { describe, expect, it } from "vitest";
import type { Action, Fact, Milestone, Workstream } from "../api/client";
import { axisLabels, eventWindow, isEmpty, layout, weekGroups } from "./layout";

// Only invented data. 2030-05-06 is a Monday.

function action(id: string, fields: Partial<Action> = {}): Action {
  return {
    id,
    local_id: `ACT-${id}`,
    event_id: "e1",
    title: `Aufgabe ${id}`,
    redacted: false,
    owner_user_id: "u1",
    status: "open",
    version: 1,
    evidence: [],
    can_change: true,
    next_statuses: [],
    ...fields,
  };
}

function milestone(id: string, date: string): Milestone {
  return {
    id,
    local_id: `MIL-${id}`,
    event_id: "e1",
    title: `Meilenstein ${id}`,
    redacted: false,
    date,
    owner_user_id: "u1",
    status: "open",
    warnings: [],
    version: 1,
    evidence: [],
    can_change: true,
    next_statuses: [],
  };
}

const ground: Workstream = {
  id: "w1",
  event_id: "e1",
  name: "Bodenbetrieb",
  redacted: false,
  lead_user_id: "u1",
  status: "active",
  version: 1,
};

const base = {
  actions: [],
  milestones: [],
  workstreams: [] as Workstream[],
  window: null,
  today: "2030-05-08",
};

describe("layout", () => {
  it("bar_for_start_and_due", () => {
    const result = layout({
      ...base,
      actions: [action("1", { start_date: "2030-05-07", due_date: "2030-05-10" })],
    });
    expect(result.weeks).toEqual(["2030-05-06"]);
    expect(result.lanes[0]?.items).toEqual([
      expect.objectContaining({ kind: "bar", from: 1, to: 5 }),
    ]);
  });

  it("point_for_due_only", () => {
    const result = layout({ ...base, actions: [action("1", { due_date: "2030-05-09" })] });
    expect(result.lanes[0]?.items).toEqual([expect.objectContaining({ kind: "point", at: 3 })]);
  });

  it("start_without_due_counts_as_undated", () => {
    const result = layout({
      ...base,
      actions: [
        action("1", { start_date: "2030-05-07" }),
        action("2"),
        action("3", { due_date: "2030-05-09" }),
      ],
    });
    expect(result.undated).toBe(2);
    const placed = result.lanes.flatMap((lane) => lane.items.map((item) => item.action.id));
    expect(placed).toEqual(["3"]);
  });

  it("lane_for_no_workstream_is_last", () => {
    const festival: Workstream = { ...ground, id: "w2", name: "Festwirtschaft" };
    const result = layout({
      ...base,
      workstreams: [ground, festival],
      actions: [
        action("1", { due_date: "2030-05-09" }),
        action("2", { due_date: "2030-05-09", workstream_id: "w2" }),
      ],
    });
    expect(result.lanes.map((lane) => lane.workstream?.id ?? null)).toEqual(["w1", "w2", null]);
    expect(result.lanes[2]?.items.map((item) => item.action.id)).toEqual(["1"]);
  });

  it("omits the lane without a workstream when it is empty and a closed workstream without items", () => {
    const closed: Workstream = { ...ground, id: "w2", status: "closed" };
    const result = layout({
      ...base,
      workstreams: [ground, closed],
      actions: [action("1", { due_date: "2030-05-09", workstream_id: "w1" })],
    });
    expect(result.lanes.map((lane) => lane.workstream?.id ?? null)).toEqual(["w1"]);
  });

  it("axis_spans_actions_milestones_and_window", () => {
    const actions = [
      action("1", { start_date: "2030-05-08", due_date: "2030-05-20" }),
      action("2", { due_date: "2030-05-29" }),
    ];
    // The window ends after the last due date: the axis runs to the end of the window.
    const long = layout({ ...base, actions, window: { start: "2030-05-15", end: "2030-06-10" } });
    expect(long.weeks).toEqual([
      "2030-05-06",
      "2030-05-13",
      "2030-05-20",
      "2030-05-27",
      "2030-06-03",
      "2030-06-10",
    ]);
    expect(long.window).toEqual({ from: 9, to: 36 });
    // The last due date is after the window: the axis runs to the last due date.
    const short = layout({ ...base, actions, window: { start: "2030-05-15", end: "2030-05-16" } });
    expect(short.weeks).toEqual(["2030-05-06", "2030-05-13", "2030-05-20", "2030-05-27"]);
    // The window starts before the first action: the axis starts with the window.
    const early = layout({ ...base, actions, window: { start: "2030-04-30", end: "2030-05-02" } });
    expect(early.weeks[0]).toBe("2030-04-29");
    // A milestone after the last due date stays on the axis.
    const marked = layout({ ...base, actions, milestones: [milestone("1", "2030-06-04")] });
    expect(marked.weeks.at(-1)).toBe("2030-06-03");
    expect(marked.milestones).toEqual([expect.objectContaining({ at: 29 })]);
    // A milestone after the last due date and after the end of the window extends the axis.
    const wrapUp = layout({
      ...base,
      actions,
      window: { start: "2030-05-15", end: "2030-05-16" },
      milestones: [milestone("1", "2030-07-02")],
    });
    expect(wrapUp.weeks.at(-1)).toBe("2030-07-01");
    expect(wrapUp.milestones).toEqual([expect.objectContaining({ at: 57 })]);
    // A milestone before the first action starts the axis.
    const early2 = layout({ ...base, actions, milestones: [milestone("1", "2030-04-02")] });
    expect(early2.weeks[0]).toBe("2030-04-01");
  });

  it("places today on the axis only when it is inside", () => {
    const actions = [action("1", { due_date: "2030-05-09" })];
    expect(layout({ ...base, actions }).today).toBe(2);
    expect(layout({ ...base, actions, today: "2031-01-01" }).today).toBeNull();
  });

  it("layout_of_empty_event", () => {
    // No dated action, no milestone and no window: no axis, only the count of undated actions.
    const result = layout({
      ...base,
      actions: [action("1"), action("2", { start_date: "2030-05-07" })],
    });
    expect(result).toEqual({
      weeks: [],
      lanes: [],
      milestones: [],
      window: null,
      today: null,
      undated: 2,
    });
  });

  it("layout_with_milestones_only", () => {
    const result = layout({
      ...base,
      actions: [action("1")],
      milestones: [milestone("1", "2030-05-15")],
      window: { start: "2030-05-06", end: "2030-05-31" },
    });
    expect(result.weeks).toEqual(["2030-05-06", "2030-05-13", "2030-05-20", "2030-05-27"]);
    expect(result.lanes).toEqual([]);
    expect(result.milestones).toEqual([expect.objectContaining({ at: 9 })]);
    expect(result.window).toEqual({ from: 0, to: 26 });
    expect(result.undated).toBe(1);
  });

  it("shows the window alone as an axis with a band", () => {
    const result = layout({ ...base, window: { start: "2030-05-15", end: "2030-06-10" } });
    expect(result.weeks).toHaveLength(5);
    expect(result.window).toEqual({ from: 2, to: 29 });
    expect(result.lanes).toEqual([]);
  });

  it("layout_without_window", () => {
    const result = layout({
      ...base,
      actions: [action("1", { start_date: "2030-05-07", due_date: "2030-05-21" })],
    });
    expect(result.window).toBeNull();
    expect(result.weeks).toEqual(["2030-05-06", "2030-05-13", "2030-05-20"]);
  });

  it("spans several years in whole weeks", () => {
    const result = layout({
      ...base,
      actions: [action("1", { start_date: "2030-05-06", due_date: "2033-05-01" })],
    });
    expect(result.weeks).toHaveLength(156);
    expect(result.weeks.at(-1)).toBe("2033-04-25");
    expect(result.lanes[0]?.items[0]).toMatchObject({ from: 0, to: 1092 });
  });
});

describe("a span of centuries", () => {
  // A due date typed as the year 2300 must not freeze the page.
  const far = layout({
    ...base,
    actions: [
      action("1", { start_date: "2030-05-06", due_date: "2300-05-01" }),
      ...Array.from({ length: 200 }, (_, index) =>
        action(`d${index}`, { due_date: `2030-06-${String((index % 28) + 1).padStart(2, "0")}` }),
      ),
    ],
    milestones: [milestone("1", "2300-05-02")],
  });

  it("lays out in whole weeks", () => {
    expect(far.weeks.length).toBeGreaterThan(14_000);
    expect(far.weeks[0]).toBe("2030-05-06");
  });

  it("groups by week without a pass over the weeks for each entry", () => {
    const started = performance.now();
    const groups = weekGroups(far);
    expect(performance.now() - started).toBeLessThan(250);
    expect(groups.at(-1)?.week).toBe("2300-04-30");
    expect(groups.reduce((sum, group) => sum + group.entries.length, 0)).toBe(202);
  });

  it("labels at most about forty weeks, at the start of a month", () => {
    const labels = axisLabels(far.weeks);
    expect(labels.length).toBeLessThanOrEqual(45);
    expect(labels.length).toBeGreaterThan(1);
    for (const label of labels) {
      expect(label.date.endsWith("-01") || Number(label.date.slice(8)) <= 7).toBe(true);
    }
  });
});

describe("axisLabels", () => {
  it("labels the first week and the first week of each month, with the year at a new year", () => {
    const result = layout({
      ...base,
      actions: [action("1", { start_date: "2030-12-16", due_date: "2031-02-10" })],
    });
    const labels = axisLabels(result.weeks);
    expect(labels.map((label) => [label.date, label.withYear])).toEqual([
      ["2030-12-16", true],
      ["2031-01-06", true],
      ["2031-02-03", false],
    ]);
    expect(labels[0]?.at).toBe(0);
    expect(labels[1]?.at).toBe(21);
  });

  it("gives none for an axis without weeks", () => {
    expect(axisLabels([])).toEqual([]);
  });
});

describe("isEmpty", () => {
  it("is true only without dated actions, milestones and window", () => {
    expect(isEmpty({ ...base, actions: [action("1")] })).toBe(true);
    expect(isEmpty({ ...base, actions: [action("1", { due_date: "2030-05-09" })] })).toBe(false);
    expect(isEmpty({ ...base, milestones: [milestone("1", "2030-05-09")] })).toBe(false);
    expect(isEmpty({ ...base, window: { start: "2030-05-09", end: "2030-05-10" } })).toBe(false);
  });
});

describe("weekGroups", () => {
  it("groups the actions by the week of the due date and the milestones by their date", () => {
    const result = layout({
      ...base,
      actions: [
        action("1", { start_date: "2030-05-07", due_date: "2030-05-14" }),
        action("2", { due_date: "2030-05-08" }),
      ],
      milestones: [milestone("1", "2030-05-12")],
    });
    const groups = weekGroups(result);
    expect(groups.map((group) => group.week)).toEqual(["2030-05-06", "2030-05-13"]);
    expect(
      groups[0]?.entries.map((entry) =>
        entry.kind === "milestone" ? entry.milestone.id : entry.item.action.id,
      ),
    ).toEqual(["2", "1"]);
    expect(groups[1]?.entries.map((entry) => entry.kind)).toEqual(["action"]);
  });
});

describe("eventWindow", () => {
  function fact(field_key: string, state: Fact["state"], start: string, end: string): Fact {
    return {
      id: field_key,
      field_id: field_key,
      field_key,
      state,
      redacted: false,
      version: 1,
      evidence: [],
      accepted_by: { kind: "member", id: "u1", channel: "web" },
      accepted_at: "2030-03-01T13:12:00Z",
      ...(state === "unknown"
        ? {}
        : { value: { type: "date-window", start, end, granularity: "day" } }),
    } as Fact;
  }

  it("prefers the accepted exact dates", () => {
    const facts = [
      fact("date_window", "accepted", "2030-05-01", "2030-06-30"),
      fact("exact_dates", "accepted", "2030-05-18", "2030-05-19"),
    ];
    expect(eventWindow(facts)).toMatchObject({
      fact: facts[1],
      start: "2030-05-18",
      end: "2030-05-19",
      field_key: "exact_dates",
      state: "accepted",
    });
  });

  it("takes the date window when it is an assumption and the exact dates are not accepted", () => {
    const facts = [
      fact("exact_dates", "assumption", "2030-05-18", "2030-05-19"),
      fact("date_window", "assumption", "2030-05-01", "2030-06-30"),
    ];
    expect(eventWindow(facts)).toMatchObject({ field_key: "date_window", state: "assumption" });
  });

  it("gives no window when both are unknown or redacted", () => {
    expect(eventWindow([fact("date_window", "unknown", "", "")])).toBeNull();
    const redacted = {
      ...fact("date_window", "accepted", "", ""),
      redacted: true,
      value: undefined,
    };
    expect(eventWindow([redacted])).toBeNull();
    expect(eventWindow([])).toBeNull();
  });
});
