import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { Router } from "../router/Router";
import * as r from "../test/workRecords";
import { WorkCard } from "./WorkCard";
import { toWorkItem, type WorkItem } from "./workItem";

const names = (id: string) => (id.endsWith("b1") ? "Anna Muster" : "");

function card(
  kind: RecordKind,
  record: unknown,
  props: { workstream?: string; today?: string } = {},
) {
  const item = toWorkItem(kind, record as never, names) as WorkItem;
  render(
    <Router>
      <ul>
        <WorkCard
          item={item}
          eventId="e1"
          today={props.today ?? "2030-05-20"}
          workstreamName={props.workstream}
        />
      </ul>
    </Router>,
  );
  return screen.getByRole("listitem");
}

describe("WorkCard", () => {
  it("is one link to the old page of the kind, named by id, status, title, date, area and owner", () => {
    const li = card(
      "action",
      r.action("ACT-012", {
        title: "Sponsorenliste nachführen",
        status: "in-progress",
        due_date: "2030-05-30",
        workstream_id: "w1",
      }),
      { workstream: "Werbung" },
    );
    const link = within(li).getByRole("link");
    expect(link).toHaveAttribute("href", "/events/e1/actions#ACT-012");
    expect(within(li).getAllByRole("link")).toHaveLength(1);
    for (const part of [
      "Aufgabe",
      "ACT-012",
      "in Arbeit",
      "Sponsorenliste nachführen",
      "30.05.2030",
      "Bereich:\\s*Werbung",
      "Verantwortlich: Anna Muster",
    ]) {
      expect(link).toHaveAccessibleName(new RegExp(part));
    }
  });

  it("shows the real status of the kind as text, not by color alone", () => {
    const li = card("commitment", r.commitment("COM-001", { status: "conditional" }));
    expect(within(li).getByText("bedingt")).toBeVisible();
  });

  it("flags a past due date as overdue on an open record", () => {
    const li = card("action", r.action("ACT-001", { due_date: "2030-05-01" }));
    expect(within(li).getByText(/überfällig/)).toBeInTheDocument();
  });

  it.each(["done", "canceled"])("does not flag a %s action as overdue", (status) => {
    const li = card("action", r.action("ACT-001", { due_date: "2030-05-01", status }));
    expect(within(li).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("shows the decided-on date of a decision, never overdue", () => {
    const li = card("decision", r.decision("DEC-001", { decided_on: "2030-03-12" }));
    expect(within(li).getByText("12.03.2030")).toBeInTheDocument();
    expect(within(li).getByRole("link")).toHaveAccessibleName(/Entschieden am/);
    expect(within(li).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("shows the date of a milestone", () => {
    const li = card("milestone", r.milestone("MIL-001", { date: "2030-07-01" }));
    expect(within(li).getByText("01.07.2030")).toBeInTheDocument();
    expect(within(li).getByRole("link")).toHaveAttribute("href", "/events/e1/milestones#MIL-001");
  });

  it("shows no date on a risk, and a risk is no link yet", () => {
    const li = card("risk", r.risk("RSK-001", { status: "occurred" }));
    expect(within(li).queryByRole("link")).not.toBeInTheDocument();
    expect(within(li).queryByRole("time")).not.toBeInTheDocument();
    expect(li).toHaveTextContent("RSK-001");
    expect(li).toHaveTextContent("eingetreten");
  });

  it("shows the tombstone for a redacted record", () => {
    const li = card("action", r.action("ACT-001", { title: "", redacted: true }));
    expect(within(li).getByText("entfernt")).toBeInTheDocument();
  });

  it("omits the area chip when the record has none", () => {
    const li = card("action", r.action("ACT-001"));
    expect(within(li).queryByText(/Bereich/)).not.toBeInTheDocument();
  });

  it("keeps the full title in the name; the clamp is only paint", () => {
    const title = "Ein sehr langer Titel ".repeat(10).trim();
    const li = card("action", r.action("ACT-001", { title }));
    expect(within(li).getByRole("link")).toHaveAccessibleName(new RegExp(title));
  });
});
