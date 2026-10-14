import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, json, problem, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
];

const PROFILE = {
  facts: [
    {
      id: "f1",
      field_id: "fd1",
      field_key: "date_window",
      state: "assumption",
      value: { type: "date-window", start: "2030-05-13", end: "2030-05-19", granularity: "day" },
      redacted: false,
      version: 1,
      evidence: [],
      accepted_by: { kind: "member", id: "x", channel: "web" },
      accepted_at: "2030-03-01T13:12:00Z",
    },
  ],
  proposals: [],
  open_questions: [],
};

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
  const { narrow = false, answers, ...rest } = setup;
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
      answers: {
        "GET /profile": () => json(200, PROFILE),
        "GET /fields": () => json(200, { items: [] }),
        ...answers,
      },
      ...rest,
    },
  );
}

const query = () => new URLSearchParams(window.location.search);
const ids = () =>
  screen.queryAllByText(/^(ACT|COM|MIL|DEC|RSK)-\d{3}$/).map((element) => element.textContent);
const tab = (name: string) => screen.getByRole("tab", { name });
const table = () => screen.findByRole("table", { name: "Arbeit als Tabelle" });
const axis = () => screen.findByRole("region", { name: "Zeitachse" });
const row = (id: string) => screen.getByText(id).closest("tr") as HTMLElement;
const goBack = () =>
  act(async () => {
    window.history.back();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

/** Table order of the sample: by the date of the column, the undated last, then by ID. */
const BY_DATE = [
  "ACT-006",
  "ACT-004",
  "DEC-001",
  "ACT-001",
  "COM-001",
  "MIL-001",
  "ACT-002",
  "ACT-003",
  "ACT-005",
  "RSK-001",
];

describe("the view tabs", () => {
  it("shows Board, Tabelle and Zeitplan, with the board selected and no key in the URL", async () => {
    open();
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    const list = screen.getByRole("tablist", { name: "Ansicht" });
    expect(
      within(list)
        .getAllByRole("tab")
        .map((item) => item.textContent),
    ).toEqual(["Board", "Tabelle", "Zeitplan"]);
    expect(tab("Board")).toHaveAttribute("aria-selected", "true");
    expect(window.location.search).toBe("");
  });

  it("writes the view into the URL as a new history entry and drops it for the board", async () => {
    open("?type=action");
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    const before = window.history.length;
    await userEvent.click(tab("Tabelle"));
    await table();
    expect(query().get("view")).toBe("table");
    expect(query().get("type")).toBe("action");
    expect(window.history.length).toBe(before + 1);
    expect(tab("Tabelle")).toHaveAttribute("aria-selected", "true");

    await userEvent.click(tab("Board"));
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(window.location.search).toBe("?type=action");
  });

  it("follows the buttons of the browser back and forward", async () => {
    open();
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    await userEvent.click(tab("Tabelle"));
    await table();
    await userEvent.click(tab("Zeitplan"));
    await axis();
    expect(query().get("view")).toBe("timeline");

    await goBack();
    await table();
    expect(tab("Tabelle")).toHaveAttribute("aria-selected", "true");
    await goBack();
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(tab("Board")).toHaveAttribute("aria-selected", "true");
  });

  it.each([
    ["?view=table", "Tabelle"],
    ["?view=timeline", "Zeitplan"],
    ["?view=board", "Board"],
    ["?view=nonsense", "Board"],
  ])("opens %s on the tab %s", async (search, name) => {
    open(search);
    await waitFor(() => expect(tab(name)).toHaveAttribute("aria-selected", "true"));
  });

  it("moves between the tabs with the arrow keys", async () => {
    open();
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    tab("Board").focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(tab("Tabelle")).toHaveFocus();
    await waitFor(() => expect(query().get("view")).toBe("table"));
  });

  it("applies the same filters and the same type counts in every view", async () => {
    open("?view=table&type=action&owner=mine");
    await table();
    expect(ids().sort()).toEqual(["ACT-002", "ACT-004", "ACT-005"]);
    expect(screen.getByRole("radio", { name: "Aufgaben 3", checked: true })).toBeInTheDocument();
    await userEvent.click(tab("Board"));
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    // The board keeps „Abgebrochen“ closed, so ACT-005 is not on a card.
    expect(ids().sort()).toEqual(["ACT-002", "ACT-004"]);
    expect(query().get("owner")).toBe("mine");
  });
});

describe("the table view", () => {
  it("has the columns of the work records", async () => {
    open("?view=table");
    const grid = await table();
    expect(
      within(grid)
        .getAllByRole("columnheader")
        .map((header) => header.textContent),
      // The first header holds the box „Alle auswählen“.
    ).toEqual([
      "Alle auswählen",
      "ID",
      "Typ",
      "Titel",
      "Status",
      "Verantwortlich",
      "Fällig",
      "Bereich",
    ]);
  });

  it("lists every record by date, the soonest first, by default", async () => {
    open("?view=table");
    await table();
    expect(ids()).toEqual(BY_DATE);
    expect(screen.getByRole("columnheader", { name: "Fällig" })).toHaveAttribute(
      "aria-sort",
      "ascending",
    );
  });

  it("shows the title as a link, the owner, the status, the date and the area", async () => {
    open("?view=table");
    await table();
    const act = within(row("ACT-001"));
    // An action opens in the panel and keeps the view; the other kinds keep their old pages.
    expect(act.getByRole("link", { name: "Plakat gestalten" })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work?view=table#ACT-001`,
    );
    expect(act.getByRole("img", { name: "Aufgabe" })).toBeInTheDocument();
    expect(act.getByText("offen")).toBeInTheDocument();
    expect(act.getByText("Bernd Beispiel")).toBeInTheDocument();
    expect(act.getByText("18.05.2030")).toBeInTheDocument();
    expect(act.getByText("Bodenbetrieb")).toBeInTheDocument();
    expect(within(row("DEC-001")).getByRole("link", { name: /Das Fest findet/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work?view=table#DEC-001`,
    );
  });

  it("marks an overdue date in words and not by color alone", async () => {
    open("?view=table");
    await table();
    expect(within(row("ACT-006")).getByText(/überfällig/)).toBeInTheDocument();
    expect(within(row("ACT-004")).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("links the risk to the panel", async () => {
    open("?view=table");
    await table();
    const risk = within(row("RSK-001"));
    expect(risk.getByRole("link", { name: "Regen ohne Ausweichraum" })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work?view=table#RSK-001`,
    );
  });

  it("shows the tombstone of a redacted record", async () => {
    const work = r.sampleWork();
    work["/actions"] = [r.action("ACT-001", { title: "", redacted: true })];
    open("?view=table", { lists: work });
    await table();
    expect(within(row("ACT-001")).getByText("entfernt")).toBeInTheDocument();
  });

  describe("sorting", () => {
    it("sorts by a header, writes the key and flips the direction on a second press", async () => {
      open("?view=table");
      await table();
      const before = window.history.length;
      await userEvent.click(screen.getByRole("button", { name: "Titel" }));
      expect(query().get("sort")).toBe("title");
      expect(query().has("dir")).toBe(false);
      expect(window.history.length).toBe(before);
      expect(screen.getByRole("columnheader", { name: "Titel" })).toHaveAttribute(
        "aria-sort",
        "ascending",
      );
      expect(screen.getByRole("columnheader", { name: "Fällig" })).not.toHaveAttribute("aria-sort");
      expect(ids().slice(0, 3)).toEqual(["COM-001", "DEC-001", "ACT-003"]); // Bäckerei, Das, Festzelt
      await userEvent.click(screen.getByRole("button", { name: "Titel" }));
      expect(query().get("dir")).toBe("desc");
      expect(screen.getByRole("columnheader", { name: "Titel" })).toHaveAttribute(
        "aria-sort",
        "descending",
      );
    });

    it("takes the key and the direction out of the URL when they are back at the default", async () => {
      open("?view=table&sort=title&dir=desc");
      await table();
      await userEvent.click(screen.getByRole("button", { name: "Fällig" }));
      expect(query().has("sort")).toBe(false);
      expect(query().has("dir")).toBe(false);
      expect(ids()).toEqual(BY_DATE);
      expect(query().get("view")).toBe("table");
    });

    it("opens the same order from the same URL", async () => {
      open("?view=table&sort=id&dir=desc");
      await table();
      expect(ids()).toEqual([
        "RSK-001",
        "MIL-001",
        "DEC-001",
        "COM-001",
        "ACT-006",
        "ACT-005",
        "ACT-004",
        "ACT-003",
        "ACT-002",
        "ACT-001",
      ]);
      expect(screen.getByRole("columnheader", { name: "ID" })).toHaveAttribute(
        "aria-sort",
        "descending",
      );
    });

    it("sorts with the keyboard", async () => {
      open("?view=table");
      await table();
      screen.getByRole("button", { name: "Verantwortlich" }).focus();
      await userEvent.keyboard("{Enter}");
      expect(query().get("sort")).toBe("owner");
      await userEvent.keyboard(" ");
      expect(query().get("dir")).toBe("desc");
      expect(screen.getByRole("button", { name: "Verantwortlich" })).toHaveFocus();
    });

    it("keeps the sort when the filters change, and the filters when it changes", async () => {
      open("?view=table&sort=title&type=action");
      await table();
      await userEvent.click(screen.getByRole("radio", { name: /^Alle/ }));
      expect(query().get("sort")).toBe("title");
      await userEvent.click(screen.getByRole("button", { name: "ID" }));
      expect(query().get("sort")).toBe("id");
      expect(query().has("type")).toBe(false);
    });

    it("announces the sort in the polite region", async () => {
      open("?view=table");
      await table();
      await userEvent.click(screen.getByRole("button", { name: "Titel" }));
      expect(await screen.findByText("Sortiert nach Titel, aufsteigend")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Titel" }));
      expect(await screen.findByText("Sortiert nach Titel, absteigend")).toBeInTheDocument();
    });
  });

  it("announces the count after a filter changes, as the board does", async () => {
    open("?view=table");
    await table();
    await userEvent.click(screen.getByRole("radio", { name: /^Entscheide/ }));
    expect(await screen.findByText("1 Eintrag")).toBeInTheDocument();
    expect(ids()).toEqual(["DEC-001"]);
  });

  describe("states", () => {
    it("shows the empty state of an event without work", async () => {
      open("?view=table", {
        lists: {
          "/actions": [],
          "/commitments": [],
          "/milestones": [],
          "/decisions": [],
          "/risks": [],
        },
      });
      expect(await screen.findByText("Noch keine Arbeit in diesem Anlass")).toBeInTheDocument();
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
    });

    it("shows the empty state of the filters, with a reset", async () => {
      open("?view=table&q=zzz");
      expect(await screen.findByText("Kein Eintrag passt zu den Filtern")).toBeInTheDocument();
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      const buttons = screen.getAllByRole("button", { name: "Filter zurücksetzen" });
      await userEvent.click(buttons[buttons.length - 1] as HTMLElement);
      await table();
      expect(query().get("view")).toBe("table");
      expect(query().has("q")).toBe(false);
    });

    it("shows the skeleton while a kind loads", async () => {
      open("?view=table", { answers: { "GET /risks": () => new Promise<Response>(() => {}) } });
      expect(
        await screen.findByRole("status", { name: "Arbeit wird geladen" }),
      ).toBeInTheDocument();
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
    });

    it("shows a banner for the kind that failed and the table of the others", async () => {
      open("?view=table", { answers: { "GET /risks": () => problem(500, "internal") } });
      expect(
        await screen.findByText("Risiken konnten nicht geladen werden.", { selector: "strong" }),
      ).toBeInTheDocument();
      await table();
      expect(ids()).not.toContain("RSK-001");
      expect(ids()).toContain("ACT-001");
    });
  });

  describe("on a narrow layout", () => {
    it("shows the cards of the board instead of the table, sorted, without sideways scroll", async () => {
      open("?view=table", { narrow: true });
      await screen.findByText("ACT-001");
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      expect(ids()).toEqual(BY_DATE);
      // The same cards as the board: the whole card is the link, named by ID and title.
      expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveAttribute(
        "href",
        `/events/${EVENT.id}/work?view=table#ACT-001`,
      );
    });

    it("gives the open card its status as a chip, because no column names it", async () => {
      open("?view=table", { narrow: true });
      const link = await screen.findByRole("link", { name: /ACT-001/ });
      expect(within(link).getByText("offen").closest("[class*=chips]")).not.toBeNull();
    });
  });
});

describe("the timeline view", () => {
  it("shows the axis with the dated actions and the milestones of the filtered work", async () => {
    open("?view=timeline");
    const chart = await axis();
    expect(within(chart).getAllByRole("link")).toHaveLength(4);
    expect(
      within(chart).getByRole("link", {
        name: "ACT-001 Plakat gestalten, fällig 18.05.2030, offen",
      }),
    ).toHaveAttribute("href", `/events/${EVENT.id}/actions#ACT-001`);
    expect(within(chart).getByRole("link", { name: /MIL-001/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/milestones#MIL-001`,
    );
    expect(within(chart).getByRole("heading", { name: "Bodenbetrieb" })).toBeInTheDocument();
    const legend = screen.getByRole("list", { name: "Legende" });
    expect(within(legend).getByText("Annahme")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "3 Aufgaben ohne Fälligkeit" })).toBeInTheDocument();
  });

  it("uses the filters of the page", async () => {
    open("?view=timeline&owner=mine");
    const chart = await axis();
    // Of the work of the signed-in member: one dated action and the milestone.
    expect(within(chart).getAllByRole("link")).toHaveLength(2);
    expect(within(chart).getByRole("link", { name: /ACT-004/ })).toBeInTheDocument();
    expect(within(chart).getByRole("link", { name: /MIL-001/ })).toBeInTheDocument();
  });

  it("shows the type filter and its counts as in the other views", async () => {
    open("?view=timeline&type=milestone");
    const chart = await axis();
    expect(within(chart).getAllByRole("link")).toHaveLength(1);
    expect(
      screen.getByRole("radio", { name: "Meilensteine 1", checked: true }),
    ).toBeInTheDocument();
  });

  it("says what the view shows and offers a reset when the type is not on the timeline", async () => {
    open("?view=timeline&type=risk");
    expect(
      await screen.findByText("Diese Ansicht zeigt Aufgaben und Meilensteine."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Zeitachse" })).not.toBeInTheDocument();
    const reset = screen.getAllByRole("button", { name: "Filter zurücksetzen" });
    await userEvent.click(reset[reset.length - 1] as HTMLElement);
    await axis();
    expect(query().get("view")).toBe("timeline");
    expect(query().has("type")).toBe(false);
  });

  it("shows the axis when the type filter mixes a kind of the timeline with others", async () => {
    open("?view=timeline&type=milestone,risk");
    const chart = await axis();
    expect(within(chart).getAllByRole("link")).toHaveLength(1);
    expect(
      screen.queryByText("Diese Ansicht zeigt Aufgaben und Meilensteine."),
    ).not.toBeInTheDocument();
  });

  it("shows the empty state of the timeline when nothing has a date and there is no window", async () => {
    const work = r.sampleWork();
    work["/actions"] = [r.action("ACT-001", { due_date: null })];
    work["/milestones"] = [];
    open("?view=timeline", {
      lists: work,
      answers: { "GET /profile": () => json(200, { ...PROFILE, facts: [] }) },
    });
    expect(await screen.findByText("Noch nichts im Zeitplan")).toBeInTheDocument();
  });

  it("waits for the profile before it chooses between the plan and the empty state", async () => {
    open("?view=timeline", { answers: { "GET /profile": () => new Promise<Response>(() => {}) } });
    expect(
      await screen.findByRole("status", { name: "Daten des Anlasses werden geladen" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Noch nichts im Zeitplan")).not.toBeInTheDocument();
  });

  it("says that the filters match nothing on the timeline, with a reset, when only other kinds match", async () => {
    const work = r.sampleWork();
    work["/actions"] = [r.action("ACT-001", { due_date: null })];
    work["/milestones"] = [];
    open("?view=timeline&q=B%C3%A4ckerei", {
      lists: work,
      answers: { "GET /profile": () => json(200, { ...PROFILE, facts: [] }) },
    });
    expect(await screen.findByText("Keine Treffer für diese Filter.")).toBeInTheDocument();
    expect(screen.queryByText("Noch nichts im Zeitplan")).not.toBeInTheDocument();
    const reset = screen.getAllByRole("button", { name: "Filter zurücksetzen" });
    await userEvent.click(reset[reset.length - 1] as HTMLElement);
    expect(query().has("q")).toBe(false);
  });

  it("shows a banner with a retry when the facts of the event do not load", async () => {
    let failing = true;
    open("?view=timeline", {
      answers: {
        "GET /profile": () => (failing ? problem(500, "internal") : json(200, PROFILE)),
      },
    });
    expect(
      await screen.findByText("Daten des Anlasses konnten nicht geladen werden."),
    ).toBeInTheDocument();
    failing = false;
    await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
    await axis();
    expect(
      screen.queryByText("Daten des Anlasses konnten nicht geladen werden."),
    ).not.toBeInTheDocument();
  });

  it("shows the empty state of the filters when they let nothing through", async () => {
    open("?view=timeline&q=zzz");
    expect(await screen.findByText("Kein Eintrag passt zu den Filtern")).toBeInTheDocument();
  });

  it("loads the facts of the event only when the view opens", async () => {
    const { calls } = open();
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(calls.some((c) => c.call.endsWith("/profile"))).toBe(false);
    await userEvent.click(tab("Zeitplan"));
    await axis();
    expect(calls.some((c) => c.call.endsWith("/profile"))).toBe(true);
  });

  it("keeps the other views free of the plan", async () => {
    open("?view=table");
    await table();
    expect(screen.queryByRole("region", { name: "Zeitachse" })).not.toBeInTheDocument();
  });
});
