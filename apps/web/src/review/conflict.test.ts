import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/client";
import { conflictOf } from "./conflict";

const FIELD = "0199b8e0-0000-7000-8000-0000000000f1";
const EVENT = "0199b8e0-0000-7000-8000-000000000001";

function setFact(
  id: string,
  expectedVersion: number | null,
  currentVersion: number | undefined,
  extra: Partial<Proposal> = {},
): Proposal {
  return {
    id,
    depends_on: [],
    reason: "",
    evidence: [],
    status: "open",
    stale: false,
    overdue: false,
    routed_to_me: true,
    operation: {
      kind: "set-fact",
      event_id: EVENT,
      field_id: FIELD,
      state: "accepted",
      expected_version: expectedVersion,
    },
    current:
      currentVersion === undefined
        ? null
        : {
            fact_id: "0199b8e0-0000-7000-8000-0000000000f2",
            version: currentVersion,
            state: "accepted",
          },
    ...extra,
  };
}

describe("conflictOf", () => {
  it("finds no conflict when the expected version is the current one", () => {
    const proposal = setFact("a", 2, 2);
    expect(conflictOf(proposal, [proposal])).toBeUndefined();
  });

  it("finds no conflict when the fact must not exist and does not", () => {
    const proposal = setFact("a", null, undefined);
    expect(conflictOf(proposal, [proposal])).toBeUndefined();
  });

  it("finds a conflict when the fact changed after the proposal", () => {
    const proposal = setFact("a", 2, 3);
    expect(conflictOf(proposal, [proposal])).toBe("fact-changed");
  });

  it("finds a conflict when the fact appeared although the proposal expects none", () => {
    const proposal = setFact("a", null, 1);
    expect(conflictOf(proposal, [proposal])).toBe("fact-changed");
  });

  it("uses the status and the reason of the server", () => {
    const proposal = setFact("a", 2, 2, { status: "conflict", conflict_reason: "target-changed" });
    expect(conflictOf(proposal, [proposal])).toBe("target-changed");
  });

  it("names a conflict of the status without a reason", () => {
    const proposal = setFact("a", 2, 2, { status: "conflict" });
    expect(conflictOf(proposal, [proposal])).toBe("target-changed");
  });

  it("blocks a proposal whose dependency conflicts", () => {
    const base = setFact("base", 2, 3);
    const dependent = setFact("dependent", 1, 1, { depends_on: ["base"] });
    expect(conflictOf(dependent, [base, dependent])).toBe("dependency");
  });

  it("does not look at a proposal that is not open", () => {
    const proposal = setFact("a", 2, 3, { status: "accepted" });
    expect(conflictOf(proposal, [proposal])).toBeUndefined();
  });
});
