import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { hasMessage } from "../i18n";
import {
  BOARD_COLUMNS,
  columnOf,
  RECORD_KINDS,
  STATUSES,
  statusPresentation,
} from "./recordStatus";

// The statuses of crates/api: the generated API types fail the build if a status is added there.
const EXPECTED: Record<RecordKind, string[]> = {
  action: ["open", "in-progress", "blocked", "done", "canceled"],
  commitment: ["conditional", "firm", "fulfilled", "broken", "withdrawn"],
  milestone: ["open", "reached", "canceled"],
  decision: ["in-force", "superseded", "revoked"],
  risk: ["open", "mitigated", "occurred", "closed"],
};

describe("the statuses of the record kinds", () => {
  it("are all those of the API", () => {
    expect(RECORD_KINDS).toEqual(Object.keys(EXPECTED));
    for (const kind of RECORD_KINDS) {
      expect(Object.keys(STATUSES[kind]).sort()).toEqual([...EXPECTED[kind]].sort());
    }
  });

  const all = RECORD_KINDS.flatMap((kind) =>
    EXPECTED[kind].map((status): [RecordKind, string] => [kind, status]),
  );

  it.each(all)("%s %s has exactly one board column", (kind, status) => {
    const column = columnOf(kind, status as never);
    expect(BOARD_COLUMNS.filter((candidate) => candidate === column)).toHaveLength(1);
  });

  it.each(all)("%s %s has one icon, one tone and a German label", (kind, status) => {
    const { glyph, tone, labelKey } = statusPresentation(kind, status as never);
    expect(glyph).toBeTruthy();
    expect(tone).toBeTruthy();
    expect(hasMessage(labelKey)).toBe(true);
  });
});

describe("columnOf (ADR 0082)", () => {
  it.each([
    ["action", "open", "open"],
    ["commitment", "conditional", "open"],
    ["commitment", "firm", "open"],
    ["milestone", "open", "open"],
    ["decision", "in-force", "open"],
    ["risk", "open", "open"],
    ["action", "in-progress", "in-progress"],
    ["action", "blocked", "blocked"],
    ["risk", "occurred", "blocked"],
    ["action", "done", "done"],
    ["commitment", "fulfilled", "done"],
    ["milestone", "reached", "done"],
    ["risk", "mitigated", "done"],
    ["risk", "closed", "done"],
    ["action", "canceled", "canceled"],
    ["commitment", "broken", "canceled"],
    ["commitment", "withdrawn", "canceled"],
    ["milestone", "canceled", "canceled"],
    ["decision", "superseded", "canceled"],
    ["decision", "revoked", "canceled"],
  ] as const)("puts %s %s in the column %s", (kind, status, column) => {
    expect(columnOf(kind, status as never)).toBe(column);
  });
});

describe("statusPresentation", () => {
  it("shows a conditional commitment in the warning tone", () => {
    expect(statusPresentation("commitment", "conditional").tone).toBe("warning");
  });
});
