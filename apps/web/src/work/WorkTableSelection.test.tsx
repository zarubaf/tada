import { fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, json, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
];

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

function open(search = "?view=table", setup: Setup & { narrow?: boolean } = {}) {
  const { narrow = false, ...rest } = setup;
  vi.stubGlobal("matchMedia", () => ({
    matches: narrow,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
  return renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work${search}`,
      lists: r.sampleWork(),
      workstreams: STREAMS,
      answers: {
        "GET /profile": () => json(200, { facts: [], proposals: [], open_questions: [] }),
      },
      ...rest,
    },
  );
}

const table = () => screen.findByRole("table", { name: "Arbeit als Tabelle" });
const all = () => screen.getByRole("checkbox", { name: "Alle auswählen" });
const box = (id: string, title: string) =>
  screen.getByRole("checkbox", { name: `${id} ${title} auswählen` });
const rowOf = (id: string, title: string) => box(id, title).closest("tr") as HTMLElement;
const BAR = { name: "Aktionen für die Auswahl" };
const bar = () => screen.getByRole("toolbar", BAR);
const noBar = () => expect(screen.queryByRole("toolbar", BAR)).not.toBeInTheDocument();
const tab = (name: string) => screen.getByRole("tab", { name });
/** The names of the checked record boxes; the select-all box is not a record. */
const selected = () =>
  screen
    .queryAllByRole("checkbox", { checked: true })
    .map((c) => (c.closest("label")?.textContent ?? "").replace("✓", ""))
    .filter((name) => /[A-Z]{3}-\d{3} /.test(name));

describe("the selection in the table", () => {
  it("gives each row a checkbox that is named by its ID and title and is not in the link", async () => {
    open();
    await table();
    const checkbox = box("ACT-001", "Plakat gestalten");
    expect(checkbox).not.toBeChecked();
    const link = screen.getByRole("link", { name: "Plakat gestalten" });
    expect(link).not.toContainElement(checkbox);
    expect(rowOf("ACT-001", "Plakat gestalten")).toContainElement(link);
  });

  it("puts „Alle auswählen“ in the header cell of the first column", async () => {
    open();
    const grid = await table();
    const header = within(grid).getAllByRole("columnheader")[0] as HTMLElement;
    expect(within(header).getByRole("checkbox", { name: "Alle auswählen" })).toBe(all());
  });

  it("selects every row with the header box, and none with a second press", async () => {
    open();
    await table();
    await userEvent.click(all());
    // The table shows the canceled record too.
    expect(selected()).toHaveLength(10);
    expect(all()).toBeChecked();
    expect(within(bar()).getByText("10 ausgewählt")).toBeInTheDocument();
    await userEvent.click(all());
    expect(selected()).toHaveLength(0);
    noBar();
  });

  it("shows the header box as mixed when some rows are selected", async () => {
    open();
    await table();
    expect(all()).not.toBePartiallyChecked();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(all()).toBePartiallyChecked();
    expect(all()).not.toBeChecked();
    expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
  });

  it("marks a selected row by more than colour", async () => {
    open();
    await table();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(rowOf("ACT-001", "Plakat gestalten")).toHaveAttribute("data-selected");
    expect(box("ACT-001", "Plakat gestalten")).toBeChecked();
    expect(rowOf("ACT-006", "Rechnung bezahlen")).not.toHaveAttribute("data-selected");
  });

  it("has neither column nor header box for records that nobody may change", async () => {
    open("?view=table", {
      event: { ...EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" },
    });
    await table();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("selects the rows with Ctrl+A and clears with Escape while focus is in the table", async () => {
    open();
    await table();
    const link = screen.getByRole("link", { name: "Plakat gestalten" });
    link.focus();
    expect(fireEvent.keyDown(link, { key: "a", metaKey: true })).toBe(false);
    expect(selected()).toHaveLength(10);
    await userEvent.keyboard("{Escape}");
    expect(selected()).toHaveLength(0);
    noBar();
  });

  it("clears the selection of a record that the timeline does not show, and hides the bar", async () => {
    open();
    await table();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    await userEvent.click(tab("Zeitplan"));
    await screen.findByRole("region", { name: "Zeitachse" });
    noBar();
    await userEvent.click(tab("Tabelle"));
    await table();
    expect(selected()).toHaveLength(0);
  });

  it("keeps the selection of a record that the board shows, and drops the one it hides", async () => {
    open();
    await table();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    // ACT-005 is canceled: the board folds that column, so it is out of sight there.
    await userEvent.click(box("ACT-005", "Hallenplan zeichnen"));
    expect(selected()).toHaveLength(2);
    await userEvent.click(tab("Board"));
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(selected()).toEqual(["ACT-001 Plakat gestalten auswählen"]);
    expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
    await userEvent.click(tab("Tabelle"));
    await table();
    expect(selected()).toEqual(["ACT-001 Plakat gestalten auswählen"]);
  });

  it("shares one selection between the board and the table", async () => {
    open("");
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    await userEvent.click(tab("Tabelle"));
    await table();
    expect(box("ACT-001", "Plakat gestalten")).toBeChecked();
    expect(all()).toBePartiallyChecked();
  });

  it("selects on the phone as on the board, in the list of cards", async () => {
    open("?view=table", { narrow: true });
    await screen.findByRole("checkbox", { name: "ACT-001 Plakat gestalten auswählen" });
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
    await userEvent.click(all());
    expect(selected()).toHaveLength(10);
    await userEvent.keyboard("{Escape}");
    expect(selected()).toHaveLength(0);
  });
});
