import { describe, expect, it } from "vitest";
import type { Action, Commitment, Decision, Milestone, Risk } from "../api/client";
import { toWorkItem } from "./workItem";

const nameOf = (id: string) => (id === "u1" ? "Anna Muster" : "");
const base = { event_id: "e1", owner_user_id: "u1", redacted: false, version: 3, evidence: [] };

const action = {
  ...base,
  id: "a1",
  local_id: "ACT-012",
  title: "Flyer drucken",
  workstream_id: "w1",
  due_date: "2030-05-18",
  status: "in-progress",
  can_change: true,
  next_statuses: [],
} as unknown as Action;

const commitment = {
  ...base,
  id: "c1",
  local_id: "COM-002",
  text: "Hangar stellen",
  status: "firm",
  due_date: null,
  workstream_id: null,
  can_change: true,
  can_make_firm: false,
  next_statuses: [],
} as unknown as Commitment;

const milestone = {
  ...base,
  id: "m1",
  local_id: "MIL-001",
  title: "Anmeldeschluss",
  date: "2030-06-01",
  status: "reached",
  warnings: [],
  can_change: false,
  next_statuses: [],
} as unknown as Milestone;

const decision = {
  ...base,
  id: "d1",
  local_id: "DEC-004",
  wording: "Das Fest findet im Hangar statt.",
  decided_on: "2030-03-02",
  status: "superseded",
  can_change: false,
  can_end: false,
} as unknown as Decision;

const risk = {
  ...base,
  id: "r1",
  local_id: "RSK-003",
  title: "Regen",
  status: "occurred",
  likelihood: "high",
  impact: "medium",
  can_change: true,
  next_statuses: [],
} as unknown as Risk;

describe("toWorkItem", () => {
  it("maps an action", () => {
    expect(toWorkItem("action", action, nameOf)).toEqual({
      kind: "action",
      id: "a1",
      readableId: "ACT-012",
      title: "Flyer drucken",
      status: "in-progress",
      column: "in-progress",
      owner: { id: "u1", name: "Anna Muster" },
      date: "2030-05-18",
      workstreamId: "w1",
      version: 3,
      redacted: false,
      record: action,
    });
  });

  it("maps a commitment with its text and without a date", () => {
    const item = toWorkItem("commitment", commitment, nameOf);
    expect(item).toMatchObject({ title: "Hangar stellen", column: "open", status: "firm" });
    expect(item.date).toBeUndefined();
    expect(item.workstreamId).toBeUndefined();
  });

  it("maps a milestone with its date", () => {
    expect(toWorkItem("milestone", milestone, nameOf)).toMatchObject({
      date: "2030-06-01",
      column: "done",
    });
  });

  it("maps a decision with its wording and the day it was decided", () => {
    expect(toWorkItem("decision", decision, nameOf)).toMatchObject({
      title: "Das Fest findet im Hangar statt.",
      date: "2030-03-02",
      column: "canceled",
      status: "superseded",
    });
  });

  it("maps a risk without a date", () => {
    const item = toWorkItem("risk", risk, nameOf);
    expect(item).toMatchObject({ title: "Regen", column: "blocked", status: "occurred" });
    expect(item.date).toBeUndefined();
  });

  it("keeps the redaction and an unknown owner", () => {
    const redacted = { ...action, redacted: true, title: "", owner_user_id: "u9" };
    expect(toWorkItem("action", redacted, nameOf)).toMatchObject({
      redacted: true,
      title: "",
      owner: { id: "u9", name: "" },
    });
  });
});
