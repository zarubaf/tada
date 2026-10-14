import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { recordHref } from "./workLinks";

const item = (kind: RecordKind, readableId: string) => ({ kind, readableId });

describe("recordHref", () => {
  it("opens an action in the panel of the Arbeit page", () => {
    expect(recordHref("e1", item("action", "ACT-012"))).toBe("/events/e1/work#ACT-012");
  });

  it("opens a milestone in the panel of the Arbeit page", () => {
    expect(recordHref("e1", item("milestone", "MIL-001"))).toBe("/events/e1/work#MIL-001");
  });

  it("opens a decision in the panel of the Arbeit page", () => {
    expect(recordHref("e1", item("decision", "DEC-004"))).toBe("/events/e1/work#DEC-004");
  });

  it("keeps the query of the page, so that the view and the filters stay", () => {
    expect(recordHref("e1", item("action", "ACT-012"), "view=table&type=action")).toBe(
      "/events/e1/work?view=table&type=action#ACT-012",
    );
  });

  it("opens a commitment in the panel, like an action", () => {
    expect(recordHref("e1", item("commitment", "COM-003"), "view=table")).toBe(
      "/events/e1/work?view=table#COM-003",
    );
  });

  it("opens a risk in the panel, as it has no page of its own", () => {
    expect(recordHref("e1", item("risk", "RSK-001"), "view=table")).toBe(
      "/events/e1/work?view=table#RSK-001",
    );
  });

  it("escapes the event ID", () => {
    expect(recordHref("a b", item("action", "ACT-001"))).toBe("/events/a%20b/work#ACT-001");
  });
});
