import { describe, expect, it } from "vitest";
import { milestoneWarningText } from "./milestoneWarnings";

const localId = (id: string) => (id === "a2" ? "ACT-002" : undefined);

describe("milestoneWarningText", () => {
  it("says that the date has passed", () => {
    expect(milestoneWarningText({ kind: "overdue" }, localId)).toBe("Datum überschritten");
  });

  it("names the action that is due after the milestone", () => {
    expect(milestoneWarningText({ kind: "late-action", action_id: "a2" }, localId)).toBe(
      "ACT-002 ist erst danach fällig",
    );
  });

  it("does not name an action that the list lacks", () => {
    expect(milestoneWarningText({ kind: "late-action", action_id: "zz" }, localId)).toBe(
      "Eine Aufgabe ist erst danach fällig",
    );
  });
});
