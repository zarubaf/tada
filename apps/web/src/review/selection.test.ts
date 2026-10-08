import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/client";
import { deselect, select } from "./selection";

function proposal(id: string, dependsOn: string[] = [], status: Proposal["status"] = "open") {
  return {
    id,
    depends_on: dependsOn,
    status,
    stale: false,
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
