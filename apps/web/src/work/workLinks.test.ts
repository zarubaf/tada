import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { recordHref } from "./workLinks";

const item = (kind: RecordKind, readableId: string) => ({ kind, readableId });

describe("recordHref", () => {
  it("opens an action in the panel of the Arbeit page", () => {
    expect(recordHref("e1", item("action", "ACT-012"))).toBe("/events/e1/work#ACT-012");
  });

  it("keeps the query of the page, so that the view and the filters stay", () => {
    expect(recordHref("e1", item("action", "ACT-012"), "view=table&type=action")).toBe(
      "/events/e1/work?view=table&type=action#ACT-012",
    );
  });

  it.each([
    ["commitment", "COM-003", "/events/e1/commitments#COM-003"],
    ["milestone", "MIL-001", "/events/e1/milestones#MIL-001"],
    ["decision", "DEC-004", "/events/e1/decisions#DEC-004"],
  ] as const)("still links a %s to the old page of its kind", (kind, id, href) => {
    expect(recordHref("e1", item(kind, id), "view=table")).toBe(href);
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
