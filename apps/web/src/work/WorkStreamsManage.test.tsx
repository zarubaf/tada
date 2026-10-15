import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, json, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

// „Bereiche verwalten“ on the „Arbeit“ page (ADR 0082): reached from the Bereich filter and by an
// address, and the page shows its changes without a reload. The dialog itself is tested in
// WorkstreamsDialog.test.tsx.
const user = userEvent.setup({ delay: null });
const GROUND = { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 };

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
      workstreams: [GROUND],
      eventManager: true,
      ...rest,
    },
  );
}

// Some record has loaded: a phone shows one column, so no heading is certain.
const ready = () => screen.findAllByRole("link", { name: /^(ACT|COM|MIL|DEC|RSK)-\d{3}/ });
const query = () => new URLSearchParams(window.location.search);
const dialog = () => screen.findByRole("dialog", { name: "Bereiche verwalten" });
const filterButton = () => screen.getByRole("button", { name: /^Filter(, \d+ gesetzt)?$/ });

async function openFromFilter() {
  await user.click(filterButton());
  await user.click(screen.getByRole("button", { name: /^Bereich/ }));
  await user.click(await screen.findByRole("button", { name: "Bereiche verwalten …" }));
}

describe("Bereiche verwalten on the Arbeit page", () => {
  it.each([false, true])(
    "opens from the end of the Bereich filter (narrow: %s)",
    async (narrow) => {
      open("", { narrow });
      await ready();

      await openFromFilter();

      const sheet = await dialog();
      expect(query().get("manage")).toBe("streams");
      expect(sheet).toContainElement(document.activeElement as HTMLElement);
      // The filter has made room: only the dialog is open.
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
    },
  );

  it("opens from the address, as the command palette goes there", async () => {
    open("?manage=streams");
    await dialog();
    expect(
      await within(await screen.findByRole("list", { name: "Bereiche" })).findByText(
        "Bodenbetrieb",
      ),
    ).toBeInTheDocument();
  });

  it("closes with Escape, drops the key, keeps the filters and returns focus to the page", async () => {
    open("?view=table&type=action&manage=streams");
    await dialog();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(query().get("manage")).toBeNull();
    expect(query().get("view")).toBe("table");
    expect(query().get("type")).toBe("action");
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus(),
    );
  });

  it("lists the workstreams read-only for a member who may not manage", async () => {
    open("?manage=streams", { eventManager: false });
    await dialog();
    await screen.findByRole("list", { name: "Bereiche" });
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
  });

  it("offers the new workstream in the filter after a create, without loading the page again", async () => {
    let rows: unknown[] = [GROUND];
    const { calls } = open("?manage=streams", {
      answers: {
        "GET /workstreams": () => json(200, { items: rows }),
        "POST /workstreams": () => {
          rows = [...rows, { ...GROUND, id: "w9", name: "Gelände" }];
          return json(201, rows[1]);
        },
      },
    });
    await dialog();
    await user.type(await screen.findByRole("textbox", { name: "Name (Pflichtfeld)" }), "Gelände");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));
    await within(await screen.findByRole("list", { name: "Bereiche" })).findByText("Gelände");
    await user.click(screen.getByRole("button", { name: "Schliessen" }));

    await user.click(filterButton());
    await user.click(screen.getByRole("button", { name: /^Bereich/ }));
    expect(await screen.findByRole("option", { name: "Gelände" })).toBeInTheDocument();
    // The records are not loaded again.
    expect(
      calls.filter((c) => c.call.endsWith("/actions") && c.call.startsWith("GET")),
    ).toHaveLength(1);
  });

  it("keeps the chosen workstream of the filter and shows a new name on the cards", async () => {
    let rows: unknown[] = [GROUND];
    open("?stream=w1&manage=streams", {
      answers: {
        "GET /workstreams": () => json(200, { items: rows }),
        "PATCH /workstreams/w1": () => {
          rows = [{ ...GROUND, name: "Flugplatz", version: 2 }];
          return json(200, rows[0]);
        },
      },
    });
    await dialog();
    await user.click(await screen.findByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    const name = screen.getByRole("textbox", { name: "Name (Pflichtfeld)" });
    await user.clear(name);
    await user.type(name, "Flugplatz");
    await user.click(screen.getByRole("button", { name: "Speichern" }));
    await within(await screen.findByRole("list", { name: "Bereiche" })).findByText("Flugplatz");
    await user.click(screen.getByRole("button", { name: "Schliessen" }));

    expect(query().get("stream")).toBe("w1");
    expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveAccessibleDescription(
      /Bereich: Flugplatz/,
    );
    await user.click(filterButton());
    expect(screen.getByRole("button", { name: "Bereich: Flugplatz" })).toBeInTheDocument();
  });

  it("does not open before the directory and the event have loaded", async () => {
    open("?manage=streams", { answers: { "GET /workstreams": () => json(500, {}) } });
    await screen.findByText("Personen und Bereiche konnten nicht geladen werden.", {
      selector: "strong",
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
