import { act, fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
];

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

function open(search = "", setup: Setup & { narrow?: boolean } = {}) {
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
      ...rest,
    },
  );
}

const ready = () => screen.findByRole("heading", { level: 2, name: /^Offen/ });
const box = (id: string, title: string) =>
  screen.getByRole("checkbox", { name: `${id} ${title} auswählen` });
const item = (id: string, title: string) => box(id, title).closest("li") as HTMLElement;
/** The names of the checked cards; the boxes of the columns are not cards. */
const selected = () =>
  screen
    .queryAllByRole("checkbox", { checked: true })
    .map((c) => (c.closest("label")?.textContent ?? "").replace("✓", ""))
    .filter((name) => /^[A-Z]{3}-\d{3} /.test(name));

describe("the selection on the work board", () => {
  it("gives each card a checkbox that is named by its ID and title and is not in the link", async () => {
    open();
    await ready();
    const checkbox = box("ACT-001", "Plakat gestalten");
    expect(checkbox).not.toBeChecked();
    const link = screen.getByRole("link", { name: /^ACT-001/ });
    expect(link).not.toContainElement(checkbox);
    // One link, one name: the checkbox does not change the name of the card.
    expect(link).toHaveAccessibleName("ACT-001 Plakat gestalten");
    // The checkbox comes before the link, so Tab reaches it first.
    expect(checkbox.compareDocumentPosition(link) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("marks the card, and every card, while a selection exists", async () => {
    open();
    await ready();
    expect(item("ACT-001", "Plakat gestalten")).not.toHaveAttribute("data-selecting");
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(box("ACT-001", "Plakat gestalten")).toBeChecked();
    expect(item("ACT-001", "Plakat gestalten")).toHaveAttribute("data-selected");
    // Every other card shows its checkbox now (the CSS reads this attribute).
    expect(item("ACT-006", "Rechnung bezahlen")).toHaveAttribute("data-selecting");
    expect(item("ACT-006", "Rechnung bezahlen")).not.toHaveAttribute("data-selected");
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(item("ACT-006", "Rechnung bezahlen")).not.toHaveAttribute("data-selecting");
  });

  it("offers no checkbox for a record the member cannot change, a redacted one or in a closed event", async () => {
    const work = r.sampleWork();
    work["/actions"] = [
      r.action("ACT-001", { title: "Plakat gestalten" }),
      r.action("ACT-002", { title: "Nur lesen", can_change: false }),
      r.action("ACT-003", { title: "", redacted: true }),
    ];
    open("", { lists: work });
    await ready();
    expect(box("ACT-001", "Plakat gestalten")).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /ACT-002/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /ACT-003/ })).not.toBeInTheDocument();
    // The card is still there.
    expect(screen.getByText("ACT-002")).toBeInTheDocument();
  });

  it("offers no checkbox in a closed event", async () => {
    open("", { event: { ...EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" } });
    await ready();
    expect(screen.getByText("ACT-001")).toBeInTheDocument();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("drops a record that the filters hide, and does not bring it back", async () => {
    open();
    await ready();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    await userEvent.click(box("MIL-001", "Programm steht"));
    expect(selected()).toHaveLength(2);
    await userEvent.click(screen.getByRole("radio", { name: /^Meilensteine/ }));
    expect(selected()).toEqual(["MIL-001 Programm steht auswählen"]);
    await userEvent.click(screen.getByRole("radio", { name: /^Alle/ }));
    expect(selected()).toEqual(["MIL-001 Programm steht auswählen"]);
  });

  it("selects all that can be changed in a column", async () => {
    open();
    await ready();
    const offen = screen.getByRole("region", { name: /^Offen/ });
    await userEvent.click(
      within(offen).getByRole("checkbox", { name: "Alle in Spalte auswählen" }),
    );
    expect(selected()).toHaveLength(5);
    expect(within(offen).getByRole("checkbox", { name: "Alle in Spalte auswählen" })).toBeChecked();
    await userEvent.click(
      within(offen).getByRole("checkbox", { name: "Alle in Spalte auswählen" }),
    );
    expect(selected()).toHaveLength(0);
  });

  it("has no select-all box in a column where nothing can be changed", async () => {
    const work = r.sampleWork();
    work["/actions"] = [r.action("ACT-002", { status: "in-progress", can_change: false })];
    open("", { lists: work });
    await ready();
    const column = screen.getByRole("region", { name: /^In Arbeit/ });
    expect(within(column).queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("selects all visible cards with Ctrl+A on the board", async () => {
    open();
    await ready();
    screen.getByRole("link", { name: /^ACT-001/ }).focus();
    await userEvent.keyboard("{Control>}a{/Control}");
    // Nine cards show; the canceled column is collapsed and does not count.
    expect(selected()).toHaveLength(9);
    await userEvent.keyboard("{Escape}");
    expect(selected()).toHaveLength(0);
  });

  it("takes Ctrl+A from the browser only when it selects something", async () => {
    const link = () => screen.getByRole("link", { name: /^ACT-001/ });
    open();
    await ready();
    // Not cancelled: the browser would select the text of the page.
    expect(fireEvent.keyDown(link(), { key: "a", ctrlKey: true })).toBe(false);
    expect(selected()).toHaveLength(9);
  });

  it("leaves Ctrl+A to the browser in a closed event, where nothing can be selected", async () => {
    open("", { event: { ...EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" } });
    await ready();
    const link = screen.getByRole("link", { name: /^ACT-001/ });
    expect(fireEvent.keyDown(link, { key: "a", ctrlKey: true })).toBe(true);
  });

  it("selects only the status that shows with Ctrl+A on the phone", async () => {
    open("", { narrow: true });
    await screen.findByRole("button", { name: /^Offen/ });
    screen.getByRole("link", { name: /^ACT-001/ }).focus();
    await userEvent.keyboard("{Control>}a{/Control}");
    expect(selected()).toHaveLength(5);
  });

  it("shows a partly selected column as mixed", async () => {
    open();
    await ready();
    const offen = screen.getByRole("region", { name: /^Offen/ });
    const all = () => within(offen).getByRole("checkbox", { name: "Alle in Spalte auswählen" });
    expect(all()).not.toBePartiallyChecked();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(all()).toBePartiallyChecked();
    expect(all()).not.toBeChecked();
    await userEvent.click(all());
    expect(all()).toBeChecked();
    expect(all()).not.toBePartiallyChecked();
  });

  it("drops a card of another status when the status select of the phone changes", async () => {
    open("", { narrow: true });
    await screen.findByRole("button", { name: /^Offen/ });
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(selected()).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: /^Offen/ }));
    await userEvent.click(screen.getByRole("option", { name: /^Erledigt/ }));
    expect(selected()).toHaveLength(0);
    await userEvent.click(screen.getByRole("button", { name: /^Erledigt/ }));
    await userEvent.click(screen.getByRole("option", { name: /^Offen/ }));
    expect(selected()).toHaveLength(0);
  });

  it("drops the cards of the canceled column when a step back in the history closes it", async () => {
    open("?canceled=1");
    await ready();
    await userEvent.click(box("ACT-005", "Hallenplan zeichnen"));
    expect(selected()).toHaveLength(1);
    act(() => {
      window.history.replaceState(null, "", `/events/${EVENT.id}/work`);
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    // The cards are gone, so the check is the bar: a selection that is out of sight is none.
    expect(
      screen.queryByRole("toolbar", { name: "Aktionen für die Auswahl" }),
    ).not.toBeInTheDocument();
  });

  it("clears the selection with Escape and with the meta key as well", async () => {
    open();
    await ready();
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    await userEvent.keyboard("{Escape}");
    expect(selected()).toHaveLength(0);
    screen.getByRole("link", { name: /^ACT-001/ }).focus();
    await userEvent.keyboard("{Meta>}a{/Meta}");
    expect(selected()).toHaveLength(9);
  });

  it("drops the selected records of the canceled column when it collapses", async () => {
    open();
    await ready();
    await userEvent.click(screen.getByRole("button", { name: /^Abgebrochen/ }));
    await userEvent.click(box("ACT-005", "Hallenplan zeichnen"));
    expect(selected()).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: /^Abgebrochen/ }));
    expect(selected()).toHaveLength(0);
  });

  it("selects on the phone, in the list of one status", async () => {
    open("", { narrow: true });
    await screen.findByRole("button", { name: /^Offen/ });
    await userEvent.click(box("ACT-001", "Plakat gestalten"));
    expect(box("ACT-001", "Plakat gestalten")).toBeChecked();
    await userEvent.click(screen.getByRole("checkbox", { name: "Alle in Spalte auswählen" }));
    expect(selected()).toHaveLength(5);
  });
});
