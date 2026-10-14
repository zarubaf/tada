import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
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

/** The text of a failure banner. A hidden polite region repeats it for screen readers. */
const bannerText = { selector: "strong" };
const ready = () => screen.findByRole("heading", { level: 2, name: /^Offen/ });
const region = (name: RegExp) => screen.getByRole("region", { name });
const query = () => new URLSearchParams(window.location.search);
/** The search and the owner, area and due filters sit in the sheet behind the „Filter“ button. */
async function openFilters() {
  await userEvent.click(screen.getByRole("button", { name: /^Filter(, \d+ gesetzt)?$/ }));
  return screen.getByRole("dialog");
}
/** The readable IDs on the cards that show. A collapsed column shows none. */
const shown = () =>
  screen.queryAllByText(/^(ACT|COM|MIL|DEC|RSK)-\d{3}$/).map((element) => element.textContent);

describe("WorkPage", () => {
  it("has its own h1 and the type filter with the counts of every kind", async () => {
    open();
    expect(await screen.findByRole("heading", { level: 1, name: "Arbeit" })).toBeInTheDocument();
    await ready();
    const group = screen.getByRole("radiogroup", { name: "Typ" });
    for (const [name, count] of [
      ["Alle", 10],
      ["Aufgaben", 6],
      ["Zusagen", 1],
      ["Meilensteine", 1],
      ["Entscheide", 1],
      ["Risiken", 1],
    ] as const) {
      expect(within(group).getByRole("radio", { name: `${name} ${count}` })).toBeInTheDocument();
    }
  });

  it("puts every kind into the column of its status and counts them", async () => {
    open();
    await ready();
    const offen = region(/^Offen/);
    for (const id of ["ACT-001", "ACT-006", "COM-001", "MIL-001", "DEC-001"]) {
      expect(within(offen).getByText(id)).toBeInTheDocument();
    }
    expect(within(region(/^In Arbeit/)).getByText("ACT-002")).toBeInTheDocument();
    expect(within(region(/^Blockiert/)).getByText("ACT-003")).toBeInTheDocument();
    expect(within(region(/^Blockiert/)).getByText("RSK-001")).toBeInTheDocument();
    expect(within(region(/^Erledigt/)).getByText("ACT-004")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 2, name: "Offen, 5 Einträge" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Abgebrochen, 1 Eintrag" })).toBeInTheDocument();
  });

  it("links an action and a risk to the panel and the other kinds to their old pages", async () => {
    open();
    await ready();
    expect(screen.getByRole("link", { name: /ACT-001/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work#ACT-001`,
    );
    expect(screen.getByRole("link", { name: /DEC-001/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/decisions#DEC-001`,
    );
    expect(screen.getByRole("link", { name: /RSK-001/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work#RSK-001`,
    );
  });

  it("names the owner and the workstream on a card", async () => {
    open();
    await ready();
    const link = screen.getByRole("link", { name: /ACT-001/ });
    expect(link).toHaveAccessibleDescription(/Verantwortlich: Bernd Beispiel/);
    expect(link).toHaveAccessibleDescription(/Bereich: Bodenbetrieb/);
  });

  it("shows the tombstone of a redacted record", async () => {
    const work = r.sampleWork();
    work["/actions"] = [r.action("ACT-001", { title: "", redacted: true })];
    open("", { lists: work });
    await ready();
    expect(screen.getByText("entfernt")).toBeInTheDocument();
  });

  describe("the query string", () => {
    it("filters by type, with replace, and writes the type", async () => {
      open();
      await ready();
      const before = window.history.length;
      await userEvent.click(screen.getByRole("radio", { name: /^Entscheide/ }));
      expect(query().get("type")).toBe("decision");
      expect(shown()).toEqual(["DEC-001"]);
      expect(window.history.length).toBe(before);
      expect(
        screen.getByRole("heading", { level: 2, name: "Offen, 1 Eintrag" }),
      ).toBeInTheDocument();

      await userEvent.click(screen.getByRole("radio", { name: /^Alle/ }));
      expect(window.location.search).toBe("");
    });

    it("keeps the counts of the other kinds when one kind is chosen", async () => {
      open("?type=risk");
      await ready();
      expect(screen.getByRole("radio", { name: "Aufgaben 6" })).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: "Risiken 1", checked: true })).toBeInTheDocument();
    });

    it("opens the same view from the same URL", async () => {
      open("?type=action&owner=mine&stream=w1&due=none&q=plakat");
      // Owner „Ich“ and the area do not match ACT-001 (owner Bernd), so nothing shows.
      expect(await screen.findByText("Kein Eintrag passt zu den Filtern")).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: /^Aufgaben/, checked: true })).toBeInTheDocument();
      await openFilters();
      expect(screen.getByRole("searchbox", { name: "Arbeit durchsuchen" })).toHaveValue("plakat");
      expect(screen.getByRole("button", { name: "Verantwortlich: Ich" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /Ohne Datum/ })).toBeInTheDocument();
    });

    it("ignores what it does not know", async () => {
      open("?type=nonsense&due=never&view=board");
      await ready();
      expect(shown()).toHaveLength(9);
    });
  });

  describe("filters", () => {
    it("searches the title and keeps spaces while typing", async () => {
      open();
      await ready();
      await openFilters();
      await userEvent.type(screen.getByRole("searchbox"), "Festzelt bes");
      expect(screen.getByRole("searchbox")).toHaveValue("Festzelt bes");
      await waitFor(() => expect(query().get("q")).toBe("Festzelt bes"));
      expect(shown()).toEqual(["ACT-003"]);
    });

    it("writes the search once after the typing, not once for each key, and announces once", async () => {
      open();
      await ready();
      await openFilters();
      const replace = vi.spyOn(window.history, "replaceState");
      await userEvent.type(screen.getByRole("searchbox"), "Festzelt");
      expect(replace).not.toHaveBeenCalled();
      expect(screen.queryByText(/^\d+ Eintr/)).not.toBeInTheDocument();
      await waitFor(() => expect(query().get("q")).toBe("Festzelt"));
      expect(replace).toHaveBeenCalledTimes(1);
      expect(await screen.findByText("1 Eintrag")).toBeInTheDocument();
    });

    it("searches at once on Enter and when the field is cleared", async () => {
      open();
      await ready();
      await openFilters();
      await userEvent.type(screen.getByRole("searchbox"), "Festzelt{Enter}");
      expect(query().get("q")).toBe("Festzelt");
      await userEvent.click(screen.getByRole("button", { name: "Suche leeren" }));
      expect(window.location.search).toBe("");
    });

    it("follows the search when the member goes back and forward", async () => {
      open("?q=Plakat");
      await ready();
      await openFilters();
      const box = screen.getByRole("searchbox");
      expect(box).toHaveValue("Plakat");
      act(() => {
        window.history.pushState(null, "", `/events/${EVENT.id}/work?q=Festzelt`);
        window.dispatchEvent(new PopStateEvent("popstate"));
      });
      await waitFor(() => expect(box).toHaveValue("Festzelt"));
      expect(shown()).toEqual(["ACT-003"]);
      act(() => {
        window.history.pushState(null, "", `/events/${EVENT.id}/work`);
        window.dispatchEvent(new PopStateEvent("popstate"));
      });
      await waitFor(() => expect(box).toHaveValue(""));
      expect(shown()).toHaveLength(9);
    });

    it("filters by owner, „Ich“ first", async () => {
      open();
      await ready();
      await openFilters();
      await userEvent.click(screen.getByRole("button", { name: /^Verantwortlich/ }));
      expect(screen.getAllByRole("option")[0]).toHaveAccessibleName("Ich");
      expect(screen.getByRole("option", { name: "Bernd Beispiel" })).toBeInTheDocument();
      expect(screen.queryByRole("option", { name: "Niemand" })).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("option", { name: "Ich" }));
      expect(query().get("owner")).toBe("mine");
      expect([...shown()].sort()).toEqual([
        "ACT-002",
        "ACT-004",
        "COM-001",
        "DEC-001",
        "MIL-001",
        "RSK-001",
      ]);
    });

    it("filters by another member", async () => {
      open();
      await ready();
      await openFilters();
      await userEvent.click(screen.getByRole("button", { name: /^Verantwortlich/ }));
      await userEvent.click(screen.getByRole("option", { name: "Bernd Beispiel" }));
      expect([...shown()].sort()).toEqual(["ACT-001", "ACT-006"]);
    });

    it("filters by area", async () => {
      open();
      await ready();
      await openFilters();
      await userEvent.click(screen.getByRole("button", { name: /^Bereich/ }));
      await userEvent.click(screen.getByRole("option", { name: "Festwirtschaft" }));
      expect(query().get("stream")).toBe("w2");
      expect(await screen.findByText("Kein Eintrag passt zu den Filtern")).toBeInTheDocument();
    });

    it.each([
      ["Überfällig", "overdue", ["ACT-006"]],
      ["Ohne Datum", "none", ["ACT-002", "ACT-003", "DEC-001", "RSK-001"]],
    ])("filters by due: %s", async (label, value, expected) => {
      open();
      await ready();
      await openFilters();
      await userEvent.click(screen.getByRole("button", { name: /Fällig/ }));
      await userEvent.click(screen.getByRole("option", { name: label }));
      expect(query().get("due")).toBe(value);
      expect([...shown()].sort()).toEqual(expected);
    });

    it("offers a reset only while a filter is set, and the reset clears all", async () => {
      open();
      await ready();
      expect(screen.queryByRole("button", { name: "Filter zurücksetzen" })).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("radio", { name: /^Aufgaben/ }));
      await openFilters();
      await userEvent.type(screen.getByRole("searchbox"), "Plakat");
      await userEvent.click(screen.getByRole("button", { name: "Filter zurücksetzen" }));
      expect(window.location.search).toBe("");
      expect(screen.getByRole("searchbox")).toHaveValue("");
      expect(shown()).toHaveLength(9);
    });

    it("moves focus to the search after a reset, as the reset button leaves", async () => {
      open("?q=Plakat&type=action");
      await ready();
      await openFilters();
      await userEvent.click(screen.getByRole("button", { name: "Filter zurücksetzen" }));
      expect(screen.getByRole("searchbox")).toHaveFocus();
    });

    it("moves focus to the search after the reset of an empty result", async () => {
      open("?q=zzz");
      await screen.findByText("Kein Eintrag passt zu den Filtern");
      const buttons = screen.getAllByRole("button", { name: "Filter zurücksetzen" });
      await userEvent.click(buttons[buttons.length - 1] as HTMLElement);
      await ready();
      // The sheet is closed, so the search is not in the page: focus goes to the heading.
      expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus();
    });

    it("counts the columns with the filters", async () => {
      open("?owner=mine");
      await ready();
      expect(
        screen.getByRole("heading", { level: 2, name: "Offen, 3 Einträge" }),
      ).toBeInTheDocument();
    });
  });

  describe("states", () => {
    it("shows skeleton columns while a kind loads", async () => {
      open("", { answers: { "GET /risks": () => new Promise<Response>(() => {}) } });
      expect(
        await screen.findByRole("status", { name: "Arbeit wird geladen" }),
      ).toBeInTheDocument();
      expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
    });

    it("shows a banner for the kind that failed, keeps the others and retries that kind", async () => {
      let fail = true;
      const { calls } = open("", {
        answers: {
          "GET /risks": () =>
            fail ? problem(500, "internal") : json(200, { items: [r.risk("RSK-009")] }),
        },
      });
      expect(
        await screen.findByText("Risiken konnten nicht geladen werden.", bannerText),
      ).toBeInTheDocument();
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
      expect(screen.queryByText("RSK-001")).not.toBeInTheDocument();

      fail = false;
      const before = calls.filter((c) => c.call.endsWith("/actions")).length;
      await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
      expect(await screen.findByText("RSK-009")).toBeInTheDocument();
      expect(
        screen.queryByText("Risiken konnten nicht geladen werden.", bannerText),
      ).not.toBeInTheDocument();
      // The retry loads that kind only.
      expect(calls.filter((c) => c.call.endsWith("/actions")).length).toBe(before);
    });

    it("keeps the board and the focus while a retry runs, also when it fails again", async () => {
      let answer: () => Response | Promise<Response> = () => problem(500, "internal");
      open("", { answers: { "GET /risks": () => answer() } });
      await screen.findByText("Risiken konnten nicht geladen werden.", bannerText);

      let fail: (response: Response) => void = () => {};
      answer = () => new Promise<Response>((resolve) => (fail = resolve));
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      await userEvent.click(retry);
      // The other kinds still show; the skeleton is for the first load only.
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
      expect(screen.queryByRole("status", { name: "Arbeit wird geladen" })).not.toBeInTheDocument();
      expect(retry.closest("[aria-busy='true']")).not.toBeNull();

      await act(async () => fail(problem(503, "unavailable")));
      await waitFor(() => expect(retry.closest("[aria-busy='true']")).toBeNull());
      expect(
        screen.getByText("Risiken konnten nicht geladen werden.", bannerText),
      ).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Erneut versuchen" })).toHaveFocus();
    });

    it("retries the people and areas without loading the work again", async () => {
      let fail = true;
      const { calls } = open("", {
        answers: {
          "GET /api/v1/members": () => (fail ? problem(500, "internal") : json(200, { items: [] })),
        },
      });
      await screen.findByText("Personen und Bereiche konnten nicht geladen werden.", bannerText);
      const lists = () => calls.filter((c) => /\/(actions|risks)$/.test(c.call)).length;
      const before = lists();
      fail = false;
      await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
      await waitFor(() =>
        expect(
          screen.queryByText("Personen und Bereiche konnten nicht geladen werden.", bannerText),
        ).not.toBeInTheDocument(),
      );
      expect(lists()).toBe(before);
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
    });

    it("announces the failures in the polite region and has no alert of its own", async () => {
      open("", {
        answers: {
          "GET /risks": () => problem(500, "internal"),
          "GET /commitments": () => problem(500, "internal"),
          "GET /api/v1/members": () => problem(500, "internal"),
        },
      });
      await screen.findByText("Risiken konnten nicht geladen werden.", bannerText);
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      const polite = screen.getByRole("status");
      await waitFor(() =>
        expect(polite).toHaveTextContent("Risiken konnten nicht geladen werden."),
      );
      expect(polite).toHaveTextContent("Zusagen konnten nicht geladen werden.");
      expect(polite).toHaveTextContent("Personen und Bereiche konnten nicht geladen werden.");
      expect(screen.getAllByRole("button", { name: "Erneut versuchen" })).toHaveLength(3);
    });

    it("clears the polite region when the failures are gone and says only what is new", async () => {
      let fail = true;
      open("", {
        answers: {
          "GET /risks": () =>
            fail ? problem(500, "internal") : json(200, { items: [r.risk("RSK-009")] }),
          "GET /commitments": () => problem(500, "internal"),
        },
      });
      await screen.findByText("Risiken konnten nicht geladen werden.", bannerText);
      const polite = screen.getByRole("status");
      await waitFor(() =>
        expect(polite).toHaveTextContent("Zusagen konnten nicht geladen werden."),
      );
      fail = false;
      const banner = screen
        .getByText("Risiken konnten nicht geladen werden.", bannerText)
        .closest("[aria-busy]") as HTMLElement;
      await userEvent.click(within(banner).getByRole("button", { name: "Erneut versuchen" }));
      await screen.findByText("RSK-009");
      // Nothing new failed, so the region does not read the remaining failure again.
      await waitFor(() => expect(polite).toBeEmptyDOMElement());
    });

    it("says so in the polite region when a retry fails again and keeps focus on the button", async () => {
      open("", { answers: { "GET /risks": () => problem(500, "internal") } });
      await screen.findByText("Risiken konnten nicht geladen werden.", bannerText);
      const polite = screen.getByRole("status");
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      await userEvent.click(retry);
      await waitFor(() => expect(polite).toHaveTextContent("Erneut fehlgeschlagen"));
      expect(screen.getByRole("button", { name: "Erneut versuchen" })).toHaveFocus();
    });

    it("says so for an event that is unknown or forbidden, and retries", async () => {
      let known = false;
      open("", {
        answers: {
          [`GET /api/v1/events/${EVENT.id}`]: () =>
            known ? json(200, EVENT) : problem(404, "not-found"),
        },
      });
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("Das gibt es nicht, oder Sie dürfen es nicht sehen.");
      // The failures of the lists do not pile up behind it.
      expect(
        screen.queryByText("Risiken konnten nicht geladen werden.", bannerText),
      ).not.toBeInTheDocument();
      expect(screen.queryByText("ACT-001")).not.toBeInTheDocument();

      known = true;
      await userEvent.click(within(alert).getByRole("button", { name: "Erneut versuchen" }));
      expect(await ready()).toBeInTheDocument();
      expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus();
    });

    it("shows a banner when the people and areas fail, and the work still shows", async () => {
      open("", { answers: { "GET /api/v1/members": () => problem(500, "internal") } });
      expect(
        await screen.findByText("Personen und Bereiche konnten nicht geladen werden.", bannerText),
      ).toBeInTheDocument();
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
    });

    it("explains an empty event and offers a new action", async () => {
      open("", { lists: { "/actions": [], "/commitments": [], "/decisions": [] } });
      expect(await screen.findByText("Noch keine Arbeit in diesem Anlass")).toBeInTheDocument();
      expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
      // The new action opens the create state of the panel, not the old page.
      await userEvent.click(screen.getByRole("button", { name: "Neue Aufgabe" }));
      expect(
        await screen.findByRole("complementary", { name: "Neue Aufgabe" }, { timeout: 3000 }),
      ).toBeVisible();
      expect(window.location.search).toBe("?new=action");
    });

    it("explains an empty result of the filters and resets them", async () => {
      open("?q=zzz");
      expect(await screen.findByText("Kein Eintrag passt zu den Filtern")).toBeInTheDocument();
      const buttons = screen.getAllByRole("button", { name: "Filter zurücksetzen" });
      await userEvent.click(buttons[buttons.length - 1] as HTMLElement);
      expect(await ready()).toBeInTheDocument();
      expect(window.location.search).toBe("");
    });

    it("shows the same board for a closed event", async () => {
      open("", { event: { ...EVENT, status: "closed" } });
      expect(await ready()).toBeInTheDocument();
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
    });

    it("announces the count politely when the filters change, not at first", async () => {
      open();
      await ready();
      expect(screen.queryByText(/^\d+ Einträge?$/)).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("radio", { name: /^Entscheide/ }));
      await waitFor(() => expect(screen.getByText("1 Eintrag")).toBeInTheDocument());
      await userEvent.click(screen.getByRole("radio", { name: /^Aufgaben/ }));
      expect(await screen.findByText("6 Einträge")).toBeInTheDocument();
    });
  });

  describe("the Filter control", () => {
    const filterButton = () => screen.getByRole("button", { name: /^Filter(, \d+ gesetzt)?$/ });

    it("says that it opens a dialog and whether it is open", async () => {
      open();
      await ready();
      expect(filterButton()).toHaveAttribute("aria-haspopup", "dialog");
      expect(filterButton()).toHaveAttribute("aria-expanded", "false");
      await userEvent.click(filterButton());
      expect(filterButton()).toHaveAttribute("aria-expanded", "true");
    });

    it("puts focus in the search when it opens", async () => {
      open();
      await ready();
      await userEvent.click(filterButton());
      expect(await screen.findByRole("searchbox")).toHaveFocus();
    });

    it("leaves the board alone and shows the result of a search behind it", async () => {
      open();
      await ready();
      await userEvent.click(filterButton());
      await userEvent.type(await screen.findByRole("searchbox"), "Festzelt bes");
      // A wide layout has a popover, not a modal sheet: the board stays in the accessibility tree.
      await waitFor(() => expect(shown()).toEqual(["ACT-003"]));
      expect(screen.getByRole("heading", { level: 2, name: /^Offen/ })).toBeInTheDocument();
    });

    it("closes with Escape and returns focus to the button", async () => {
      open();
      await ready();
      await userEvent.click(filterButton());
      await screen.findByRole("searchbox");
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("searchbox")).not.toBeInTheDocument());
      await waitFor(() => expect(filterButton()).toHaveFocus());
    });

    it("clears the text with the first Escape in the search and closes with the second", async () => {
      open();
      await ready();
      await userEvent.keyboard("/");
      await userEvent.type(await screen.findByRole("searchbox"), "Festzelt");
      await waitFor(() => expect(shown()).toEqual(["ACT-003"]));
      await userEvent.keyboard("{Escape}");
      expect(screen.getByRole("searchbox")).toHaveValue("");
      expect(filterButton()).toHaveAttribute("aria-expanded", "true");
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("searchbox")).not.toBeInTheDocument());
      await waitFor(() => expect(filterButton()).toHaveFocus());
    });

    it("opens and closes with the same press on the button", async () => {
      open();
      await ready();
      await userEvent.click(filterButton());
      await screen.findByRole("searchbox");
      await userEvent.click(filterButton());
      await waitFor(() => expect(screen.queryByRole("searchbox")).not.toBeInTheDocument());
      expect(filterButton()).toHaveAttribute("aria-expanded", "false");
    });

    it("opens with the key „/“, with focus in the search", async () => {
      open();
      await ready();
      await userEvent.keyboard("/");
      expect(await screen.findByRole("searchbox")).toHaveFocus();
      expect(filterButton()).toHaveAttribute("aria-expanded", "true");
    });

    it("does not take the key „/“ out of a text field", async () => {
      open();
      await ready();
      await userEvent.click(filterButton());
      await userEvent.type(await screen.findByRole("searchbox"), "a/b");
      expect(screen.getByRole("searchbox")).toHaveValue("a/b");
    });

    it("opens the sheet of a phone with „/“, with focus in the search", async () => {
      open("", { narrow: true });
      await screen.findByRole("button", { name: /Offen \(\d+\)/ });
      await userEvent.keyboard("/");
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("searchbox")).toHaveFocus();
    });
  });

  describe("narrow, focus after a reset", () => {
    it("keeps focus in the search of the open sheet", async () => {
      open("?q=Plakat", { narrow: true });
      await screen.findByRole("button", { name: /Offen \(\d+\)/ });
      await userEvent.click(screen.getByRole("button", { name: "Filter, 1 gesetzt" }));
      const dialog = await screen.findByRole("dialog");
      await userEvent.click(within(dialog).getByRole("button", { name: "Filter zurücksetzen" }));
      expect(within(dialog).getByRole("searchbox")).toHaveFocus();
    });

    it("moves focus to the heading when the search is behind the closed sheet", async () => {
      open("?q=zzz", { narrow: true });
      await screen.findByText("Kein Eintrag passt zu den Filtern");
      await userEvent.click(screen.getByRole("button", { name: "Filter zurücksetzen" }));
      await screen.findByRole("button", { name: /Offen \(\d+\)/ });
      expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus();
    });
  });

  describe("narrow", () => {
    it("shows one list with a status picker and the type as a select", async () => {
      open("", { narrow: true });
      await screen.findByRole("button", { name: /Offen \(5\)/ });
      expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
      expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
      expect(shown()).toHaveLength(5);

      await userEvent.click(screen.getByRole("button", { name: /Typ/ }));
      await userEvent.click(screen.getByRole("option", { name: "Entscheide (1)" }));
      expect(query().get("type")).toBe("decision");
      expect(shown()).toEqual(["DEC-001"]);
    });

    it("keeps the filters behind a Filter button that opens a sheet", async () => {
      open("", { narrow: true });
      await screen.findByRole("button", { name: /Offen \(5\)/ });
      expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Filter" }));
      const dialog = await screen.findByRole("dialog");
      await userEvent.type(within(dialog).getByRole("searchbox"), "Plakat");
      await waitFor(() => expect(query().get("q")).toBe("Plakat"));
      await userEvent.click(within(dialog).getByRole("button", { name: "Schliessen" }));
      expect(screen.getByRole("button", { name: "Filter, 1 gesetzt" })).toBeInTheDocument();
      expect(shown()).toEqual(["ACT-001"]);
    });

    it("keeps the typed text when the sheet closes before the pause is over", async () => {
      open("", { narrow: true });
      await screen.findByRole("button", { name: /Offen \(5\)/ });
      await userEvent.click(screen.getByRole("button", { name: "Filter" }));
      const dialog = await screen.findByRole("dialog");
      await userEvent.type(within(dialog).getByRole("searchbox"), "Plakat");
      // A tap on the overlay closes the sheet without leaving the field.
      // The overlay keeps focus in the field on pointerdown, so no blur flushes the text.
      const overlay = dialog.parentElement?.parentElement as HTMLElement;
      fireEvent.pointerDown(overlay, { pointerType: "mouse", button: 0 });
      fireEvent.pointerUp(overlay, { pointerType: "mouse", button: 0 });
      fireEvent.click(overlay);
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      await waitFor(() => expect(query().get("q")).toBe("Plakat"));
    });
  });
});
