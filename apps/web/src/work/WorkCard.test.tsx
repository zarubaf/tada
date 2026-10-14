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
  props: { workstream?: string; today?: string; inColumn?: boolean } = {},
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
          inColumn={props.inColumn ?? true}
        />
      </ul>
    </Router>,
  );
  return screen.getByRole("listitem");
}

describe("WorkCard", () => {
  it("is one link to the record, named by its id and title", () => {
    const li = card(
      "action",
      r.action("ACT-012", { title: "Sponsorenliste nachführen", status: "in-progress" }),
    );
    const link = within(li).getByRole("link");
    expect(link).toHaveAttribute("href", "/events/e1/work#ACT-012");
    expect(within(li).getAllByRole("link")).toHaveLength(1);
    expect(link).toHaveAccessibleName("ACT-012 Sponsorenliste nachführen");
  });

  it("describes the kind, status, owner, date and area apart from the name, in words", () => {
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
    expect(link).toHaveAccessibleDescription(
      "Aufgabe in Arbeit Verantwortlich: Anna Muster Fällig 30.05.2030 Bereich: Werbung",
    );
    // The name stays short: none of these is part of it.
    expect(link.getAttribute("aria-labelledby")?.split(" ")).toHaveLength(2);
  });

  it("names a risk by its ID and title, as a link to the panel, and describes it by what it has", () => {
    const li = card(
      "risk",
      r.risk("RSK-001", { status: "occurred", title: "Regen am Fest", owner_user_id: "x" }),
    );
    const link = within(li).getByRole("link", { name: "RSK-001 Regen am Fest" });
    expect(link).toHaveAttribute("href", "/events/e1/work#RSK-001");
    expect(link).toHaveAccessibleDescription(
      "Risiko eingetreten Wirkung hoch Wahrscheinlich mittel",
    );
  });

  it("shows the real status of the kind as text, not by color alone", () => {
    const li = card("commitment", r.commitment("COM-001", { status: "conditional" }));
    expect(within(li).getByText("bedingt")).toBeVisible();
  });

  it("shows a plain status as an icon before the title, with the status as its name", () => {
    const li = card(
      "action",
      r.action("ACT-012", { title: "Sponsorenliste", status: "in-progress" }),
    );
    const status = within(li).getByText("in Arbeit");
    expect(status.compareDocumentPosition(within(li).getByText("Sponsorenliste"))).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    // The status is in the description, and it is not a chip.
    expect(within(li).getByRole("link")).toHaveAccessibleDescription(/in Arbeit/);
    expect(status.closest("[class*=chips]")).toBeNull();
  });

  it("gives every status its label as a chip where no column names it, as in the list of a phone", () => {
    const li = card("action", r.action("ACT-012", { status: "in-progress" }), { inColumn: false });
    expect(within(li).getByText("in Arbeit").closest("[class*=chips]")).not.toBeNull();
    expect(within(li).getByRole("link")).toHaveAccessibleDescription(/in Arbeit/);
  });

  it("gives a status that its column does not say a chip, and the icon stays decoration", () => {
    const li = card("commitment", r.commitment("COM-001", { status: "conditional" }));
    expect(within(li).getByText("bedingt")).toBeVisible();
    expect(within(li).queryByRole("img")).not.toBeInTheDocument();
    expect(within(li).getByRole("link")).toHaveAccessibleDescription(/bedingt/);
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
    expect(within(li).getByRole("link")).toHaveAccessibleDescription(/Entschieden am 12.03.2030/);
    expect(within(li).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("shows the date of a milestone", () => {
    const li = card("milestone", r.milestone("MIL-001", { date: "2030-07-01" }));
    expect(within(li).getByText("01.07.2030")).toBeInTheDocument();
    expect(within(li).getByRole("link")).toHaveAttribute("href", "/events/e1/milestones#MIL-001");
  });

  it("shows the rating of a risk as two chips in words, which the link also describes", () => {
    const li = card("risk", r.risk("RSK-001", { impact: "high", likelihood: "medium" }));
    expect(within(li).getByText("Wirkung hoch")).toBeVisible();
    expect(within(li).getByText("Wahrscheinlich mittel")).toBeVisible();
    expect(within(li).getByRole("link")).toHaveAccessibleDescription(
      /Risiko offen .*Wirkung hoch Wahrscheinlich mittel$/,
    );
  });

  it("shows no rating on a kind that has none", () => {
    const li = card("action", r.action("ACT-001"));
    expect(within(li).queryByText(/^Wirkung/)).not.toBeInTheDocument();
  });

  it("shows no date on a risk", () => {
    const li = card("risk", r.risk("RSK-001", { status: "occurred" }));
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
    expect(within(li).getByRole("link")).toHaveAccessibleName(`ACT-001 ${title}`);
  });
});
