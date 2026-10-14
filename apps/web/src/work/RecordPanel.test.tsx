import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
];

function stubNarrow(narrow: boolean) {
  vi.stubGlobal("matchMedia", () => ({
    matches: narrow,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

function open(search = "", setup: Setup & { narrow?: boolean } = {}) {
  const { narrow = false, ...rest } = setup;
  stubNarrow(narrow);
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

// The panel is a lazy chunk, which the first test of a cold run has to load.
const panel = (name: RegExp | string) =>
  screen.findByRole("complementary", { name }, { timeout: 3000 });
const hash = () => window.location.hash;
const user = userEvent.setup({ delay: null });

describe("the record panel", () => {
  describe("opening", () => {
    it("opens the action that the hash names, with its fields", async () => {
      open("#ACT-001");
      const region = await panel("ACT-001 Plakat gestalten");
      expect(within(region).getByText("ACT-001")).toBeVisible();
      expect(
        within(region).getByRole("heading", { level: 2, name: "Plakat gestalten" }),
      ).toBeVisible();
      expect(within(region).getByText("Bernd Beispiel")).toBeVisible();
      expect(within(region).getByText("Bodenbetrieb")).toBeVisible();
      // The head names the kind; the status shows once, in the list of properties.
      expect(within(region).getByText("Aufgabe")).toBeVisible();
      expect(within(region).getAllByText("offen")).toHaveLength(1);
    });

    it("shows a not-found state with a close button for an ID that does not exist", async () => {
      open("?view=table#ACT-099");
      const region = await panel("Eintrag nicht gefunden");
      expect(within(region).getByText(/ACT-099/)).toBeVisible();
      await user.click(within(region).getByRole("button", { name: "Schliessen" }));
      expect(hash()).toBe("");
      expect(window.location.search).toBe("?view=table");
      expect(screen.queryByRole("complementary")).toBeNull();
    });

    it("finds a record when the address writes its ID in lower case", async () => {
      open("#act-001");
      await panel("ACT-001 Plakat gestalten");
    });

    it("closes without an error when the hash holds a quote and a bracket", async () => {
      // The decoded hash is `x"]`; as a selector it would be a syntax error.
      open("#x%22%5D");
      const region = await panel("Eintrag nicht gefunden");
      await user.click(within(region).getByRole("button", { name: "Schliessen" }));

      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
      expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus();
    });

    it("skips a record of a kind without a panel when it steps", async () => {
      open("#ACT-001");
      await panel(/ACT-001/);
      await user.click(screen.getByRole("button", { name: "Nächster Eintrag" }));
      // COM-001 and DEC-001 open on their old pages; MIL-001 follows ACT-001 in the column.
      expect(hash()).toBe("#MIL-001");
    });

    it("sends a record of a kind without a panel to its page, and does not call it missing", async () => {
      open("#COM-001");
      const region = await panel("Eintrag öffnet noch nicht hier");
      expect(within(region).queryByText(/gibt es in diesem Anlass nicht/)).toBeNull();
      expect(
        within(region).getByRole("link", { name: "Auf der bisherigen Seite öffnen" }),
      ).toHaveAttribute("href", `/events/${EVENT.id}/commitments#COM-001`);
    });

    it("opens the risk that the hash names, with its rating", async () => {
      open("#RSK-001");
      const region = await panel(/RSK-001/);
      expect(within(region).getByRole("button", { name: "Wirkung: hoch" })).toBeVisible();
    });

    it("does not open anything without a hash", async () => {
      open();
      await screen.findByRole("heading", { level: 2, name: /^Offen/ });
      expect(screen.queryByRole("complementary")).toBeNull();
    });

    it("opens from a card with a new history entry, and back closes it", async () => {
      open();
      await user.click(await screen.findByRole("link", { name: /ACT-001/ }));
      await panel("ACT-001 Plakat gestalten");
      expect(hash()).toBe("#ACT-001");
      window.history.back();
      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
    });

    it("opens from a row of the table and keeps the view in the address", async () => {
      open("?view=table");
      await user.click(await screen.findByRole("link", { name: "Plakat gestalten" }));
      await panel("ACT-001 Plakat gestalten");
      expect(window.location.search).toBe("?view=table");
    });

    it("shows the tombstone of a redacted action", async () => {
      const work = r.sampleWork();
      work["/actions"] = [r.action("ACT-001", { title: "", redacted: true })];
      open("#ACT-001", { lists: work });
      const region = await panel("ACT-001");
      expect(within(region).getByText("entfernt")).toBeVisible();
    });

    it("lists the evidence as Quellen", async () => {
      const work = r.sampleWork();
      work["/actions"] = [
        r.action("ACT-001", {
          title: "Plakat gestalten",
          evidence: [
            {
              proposal_id: "p1",
              record_version: 1,
              source_version_id: "s1",
              captured_at: "2030-03-01T10:00:00Z",
              start_offset: 0,
              end_offset: 5,
              quote: "Plakat bis Freitag",
              redacted: false,
            },
          ],
        }),
      ];
      open("#ACT-001", { lists: work });
      const region = await panel(/ACT-001/);
      expect(within(region).getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
      expect(within(region).getByText("Plakat bis Freitag")).toBeVisible();
    });
  });

  describe("stepping", () => {
    it("goes through the board column by column and replaces the history entry", async () => {
      open("#ACT-006");
      const first = await panel(/ACT-006/);
      expect(within(first).getByText(/^1 von \d+$/)).toBeVisible();
      const before = window.history.length;
      // Offen: ACT-006 (overdue, first), ACT-001, then In Arbeit ACT-002, Blockiert ACT-003.
      await user.click(screen.getByRole("button", { name: "Nächster Eintrag" }));
      const second = await panel(/ACT-001/);
      expect(hash()).toBe("#ACT-001");
      expect(within(second).getByText(/^2 von \d+$/)).toBeVisible();
      await user.click(screen.getByRole("button", { name: "Nächster Eintrag" }));
      expect(hash()).toBe("#MIL-001");
      await user.click(screen.getByRole("button", { name: "Vorheriger Eintrag" }));
      expect(hash()).toBe("#ACT-001");
      expect(window.history.length).toBe(before);
    });

    it("keeps the previous button at the start of the list, disabled and focusable", async () => {
      open("#ACT-001");
      await panel(/ACT-001/);
      // ACT-006 is first, so ACT-001 is the second record. Step back to the first.
      const previous = screen.getByRole("button", { name: "Vorheriger Eintrag" });
      previous.focus();
      await user.keyboard("{Enter}");
      await panel(/ACT-006/);

      // The button the member used is still there and still has focus; it now refuses the press.
      expect(previous).toBeInTheDocument();
      expect(previous).toHaveFocus();
      expect(previous).toHaveAttribute("aria-disabled", "true");
      await user.keyboard("{Enter}");
      expect(hash()).toBe("#ACT-006");
      expect(previous).toHaveFocus();
    });

    it("steps in the sort of the table", async () => {
      open("?view=table&sort=id&dir=desc#ACT-006");
      await panel(/ACT-006/);
      await user.click(screen.getByRole("button", { name: "Nächster Eintrag" }));
      expect(hash()).toBe("#ACT-005");
    });
  });

  describe("closing and focus", () => {
    it("closes with the button, removes the hash and keeps the query", async () => {
      open("?type=action#ACT-001");
      const region = await panel(/ACT-001/);
      await user.click(within(region).getByRole("button", { name: "Schliessen" }));
      expect(hash()).toBe("");
      expect(window.location.search).toBe("?type=action");
    });

    it("returns focus to the card that opened it", async () => {
      open();
      const card = await screen.findByRole("link", { name: /ACT-001/ });
      await user.click(card);
      await panel(/ACT-001/);
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
      expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveFocus();
    });

    it("returns focus to the card of the record when it was opened by the address", async () => {
      open("#ACT-001");
      await panel(/ACT-001/);
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
      expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveFocus();
    });
  });

  describe("narrow", () => {
    it("makes the page behind the sheet inert", async () => {
      open("#ACT-001", { narrow: true });
      await panel(/ACT-001/);
      const behind = screen.getByRole("heading", { level: 1, name: "Arbeit", hidden: true });
      const inert = (element: HTMLElement | null): boolean =>
        element !== null && (element.inert || inert(element.parentElement));
      expect(inert(behind)).toBe(true);
    });
  });
});
