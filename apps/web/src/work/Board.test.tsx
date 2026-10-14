import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { RecordKind } from "../api/client";
import { Router } from "../router/Router";
import * as r from "../test/workRecords";
import { Board } from "./Board";
import type { BoardColumn } from "./recordStatus";
import { toWorkItem, type WorkItem } from "./workItem";

const none = () => "";
const make = (kind: RecordKind, record: unknown): WorkItem =>
  toWorkItem(kind, record as never, none) as WorkItem;

// The table of ADR 0082: every kind and status with its column.
const PLACEMENT: [RecordKind, string, BoardColumn][] = [
  ["action", "open", "open"],
  ["action", "in-progress", "in-progress"],
  ["action", "blocked", "blocked"],
  ["action", "done", "done"],
  ["action", "canceled", "canceled"],
  ["commitment", "conditional", "open"],
  ["commitment", "firm", "open"],
  ["commitment", "fulfilled", "done"],
  ["commitment", "broken", "canceled"],
  ["commitment", "withdrawn", "canceled"],
  ["milestone", "open", "open"],
  ["milestone", "reached", "done"],
  ["milestone", "canceled", "canceled"],
  ["decision", "in-force", "open"],
  ["decision", "superseded", "canceled"],
  ["decision", "revoked", "canceled"],
  ["risk", "open", "open"],
  ["risk", "mitigated", "done"],
  ["risk", "occurred", "blocked"],
  ["risk", "closed", "done"],
];

const BUILD = {
  action: r.action,
  commitment: r.commitment,
  milestone: r.milestone,
  decision: r.decision,
  risk: r.risk,
};

function board(items: WorkItem[], narrow = false) {
  render(
    <Router>
      <Board
        items={items}
        eventId="e1"
        today="2030-05-20"
        workstreamName={(id) => (id === "w1" ? "Werbung" : undefined)}
        narrow={narrow}
      />
    </Router>,
  );
}

const column = (name: RegExp) => screen.getByRole("region", { name });

describe("Board, wide", () => {
  it.each(PLACEMENT)(
    "puts a %s in status %s into the column %s",
    async (kind, status, expected) => {
      const id = `X-${kind}-${status}`;
      board([make(kind, BUILD[kind](id, { status }))]);
      if (expected === "canceled") {
        await userEvent.click(screen.getByRole("button", { name: /Abgebrochen/ }));
      }
      const names: Record<BoardColumn, RegExp> = {
        open: /^Offen/,
        "in-progress": /^In Arbeit/,
        blocked: /^Blockiert/,
        done: /^Erledigt/,
        canceled: /^Abgebrochen/,
      };
      expect(within(column(names[expected])).getByText(id)).toBeInTheDocument();
      expect(screen.getAllByText(id)).toHaveLength(1);
    },
  );

  it("lists the columns in the order of the ADR, with the counts of the shown items", () => {
    board([
      make("action", r.action("ACT-001")),
      make("commitment", r.commitment("COM-001")),
      make("action", r.action("ACT-002", { status: "blocked" })),
    ]);
    const heads = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(heads).toEqual(["Offen2", "In Arbeit0", "Blockiert1", "Erledigt0"]);
  });

  it("collapses Abgebrochen with its count and opens it with a toggle", async () => {
    board([
      make("action", r.action("ACT-001", { status: "canceled" })),
      make("decision", r.decision("DEC-001", { status: "revoked" })),
    ]);
    const toggle = screen.getByRole("button", { name: /Abgebrochen\s*2/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("ACT-001")).not.toBeInTheDocument();

    await userEvent.click(toggle);
    expect(screen.getByText("ACT-001")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: /Abgebrochen/ })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Abgebrochen ausblenden" }));
    expect(screen.queryByText("ACT-001")).not.toBeInTheDocument();
  });

  it("sorts a column by due date, the undated last", () => {
    board([
      make("action", r.action("ACT-001")),
      make("action", r.action("ACT-002", { due_date: "2030-07-01" })),
      make("action", r.action("ACT-003", { due_date: "2030-06-01" })),
    ]);
    const ids = within(column(/^Offen/))
      .getAllByRole("link")
      .map((a) => a.getAttribute("href")?.split("#")[1]);
    expect(ids).toEqual(["ACT-003", "ACT-002", "ACT-001"]);
  });

  it("says so in an empty column", () => {
    board([make("action", r.action("ACT-001"))]);
    expect(within(column(/^In Arbeit/)).getByText("Keine Einträge")).toBeInTheDocument();
  });

  it("puts the workstream name on the card", () => {
    board([make("action", r.action("ACT-001", { workstream_id: "w1" }))]);
    expect(screen.getByText("Werbung")).toBeInTheDocument();
  });

  it("keeps the keyboard order column by column", async () => {
    board([
      make("action", r.action("ACT-001")),
      make("action", r.action("ACT-002")),
      make("action", r.action("ACT-003", { status: "in-progress" })),
    ]);
    await userEvent.tab();
    expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("link", { name: /ACT-002/ })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("link", { name: /ACT-003/ })).toHaveFocus();
  });
});

describe("Board, narrow", () => {
  const items = () => [
    make("action", r.action("ACT-001")),
    make("action", r.action("ACT-002", { status: "in-progress" })),
    make("action", r.action("ACT-003", { status: "canceled" })),
  ];

  it("shows one list with a status picker that names the counts", () => {
    board(items(), true);
    expect(screen.getByRole("button", { name: /Offen \(1\)/ })).toBeInTheDocument();
    expect(screen.getByText("ACT-001")).toBeInTheDocument();
    expect(screen.queryByText("ACT-002")).not.toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { level: 2 })).toHaveLength(0);
  });

  it("switches the list with the picker, also to Abgebrochen", async () => {
    board(items(), true);
    await userEvent.click(screen.getByRole("button", { name: /Status/ }));
    await userEvent.click(screen.getByRole("option", { name: "Abgebrochen (1)" }));
    expect(screen.getByText("ACT-003")).toBeInTheDocument();
    expect(screen.queryByText("ACT-001")).not.toBeInTheDocument();
  });

  it("says so when the chosen status has no item", async () => {
    board(items(), true);
    await userEvent.click(screen.getByRole("button", { name: /Status/ }));
    await userEvent.click(screen.getByRole("option", { name: "Blockiert (0)" }));
    expect(screen.getByText("Keine Einträge")).toBeInTheDocument();
  });
});
