import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/client";
import { dependentsOf, deselect, select } from "./selection";

function proposal(id: string, dependsOn: string[] = [], status: Proposal["status"] = "open") {
  return {
    id,
    depends_on: dependsOn,
    status,
    stale: false,
    overdue: false,
    routed_to_me: true,
    can_review: true,
    reason: "",
    evidence: [],
    operation: { kind: "deprecate-field", event_id: "e", field_id: "f" },
  } satisfies Proposal;
}

// field <- fact <- note, and an unrelated question.
const proposals = [
  proposal("field"),
  proposal("fact", ["field"]),
  proposal("note", ["fact"]),
  proposal("question"),
];

describe("select", () => {
  it("selects the dependencies of a proposal, all the way down", () => {
    const result = select(proposals, new Set(), "note");
    expect([...result.selected].sort()).toEqual(["fact", "field", "note"]);
    expect(result.added.sort()).toEqual(["fact", "field"]);
  });

  it("reports a shared dependency once", () => {
    // top needs left and base, left needs base.
    const diamond = [
      proposal("base"),
      proposal("left", ["base"]),
      proposal("top", ["base", "left"]),
    ];
    const result = select(diamond, new Set(), "top");
    expect([...result.selected].sort()).toEqual(["base", "left", "top"]);
    expect(result.added.sort()).toEqual(["base", "left"]);
  });

  it("does not report a dependency that was selected before", () => {
    const result = select(proposals, new Set(["field"]), "fact");
    expect([...result.selected].sort()).toEqual(["fact", "field"]);
    expect(result.added).toEqual([]);
  });

  it("leaves a dependency out that is not open any more", () => {
    const done = [proposal("field", [], "accepted"), proposal("fact", ["field"])];
    const result = select(done, new Set(), "fact");
    expect([...result.selected]).toEqual(["fact"]);
  });

  it("does not select a proposal that is not open", () => {
    const done = [proposal("field", [], "rejected")];
    expect(select(done, new Set(), "field").selected.size).toBe(0);
  });
});

describe("deselect", () => {
  it("also deselects the selected proposals that depend on it", () => {
    const all = new Set(["field", "fact", "note", "question"]);
    expect([...deselect(proposals, all, "fact")].sort()).toEqual(["field", "question"]);
  });

  it("keeps the dependencies of the proposal", () => {
    const all = new Set(["field", "fact"]);
    expect([...deselect(proposals, all, "fact")]).toEqual(["field"]);
  });
});

describe("dependentsOf", () => {
  it("lists the open proposals that need a proposal, all the way up", () => {
    expect(dependentsOf(proposals, "field").sort()).toEqual(["fact", "note"]);
  });

  it("leaves out a proposal that is not open", () => {
    const closed = [proposal("field"), proposal("fact", ["field"], "rejected")];
    expect(dependentsOf(closed, "field")).toEqual([]);
  });
});
