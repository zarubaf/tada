import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { BERND, EVENT, json, problem, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
  { id: "w3", event_id: EVENT.id, name: "Altlast", status: "closed", version: 1 },
];

const NEXT = ["in-progress", "blocked", "done", "canceled"];

function work() {
  const sample = r.sampleWork();
  sample["/actions"] = [
    r.action("ACT-001", { title: "Plakat gestalten", next_statuses: NEXT }),
    r.action("ACT-002", {
      title: "Sponsorenliste",
      status: "in-progress",
      next_statuses: ["open", "blocked", "done", "canceled"],
    }),
    r.action("ACT-006", { title: "Rechnung bezahlen", next_statuses: NEXT }),
  ];
  return sample;
}

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

function open(setup: Setup & { narrow?: boolean } = {}) {
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
    { url: `/events/${EVENT.id}/work`, lists: work(), workstreams: STREAMS, ...rest },
  );
}

const ready = () => screen.findByRole("heading", { level: 2, name: /^Offen/ });
const pick = (title: string) =>
  userEvent.click(screen.getByRole("checkbox", { name: new RegExp(`${title} auswählen$`) }));
const BAR = { name: "Aktionen für die Auswahl" };
const bar = () => screen.getByRole("toolbar", BAR);
const noBar = () => expect(screen.queryByRole("toolbar", BAR)).not.toBeInTheDocument();
/** A control outside the bar: the „Filter“ button of the toolbar row. */
const filterButton = () => screen.getByRole("button", { name: /^Filter/ });
const inBar = (name: string | RegExp) => within(bar()).getByRole("button", { name });
/** Answers a change of an action with its record at the next version. */
const patched =
  (id: string, over: object = {}) =>
  () => {
    const before = work()["/actions"]?.find((record) => (record as { id: string }).id === id);
    return json(200, { ...(before as object), version: 2, ...over });
  };
const patchCalls = (calls: { call: string; body: unknown }[]) =>
  calls.filter((c) => c.call.startsWith("PATCH"));

describe("the bulk bar", () => {
  it("is not there without a selection and counts the selection", async () => {
    open();
    await ready();
    noBar();
    await pick("Plakat gestalten");
    expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
    await pick("Sponsorenliste");
    expect(within(bar()).getByText("2 ausgewählt")).toBeInTheDocument();
  });

  it("clears with „Auswahl aufheben“ and with Escape", async () => {
    open();
    await ready();
    await pick("Plakat gestalten");
    await userEvent.click(inBar("Auswahl aufheben"));
    noBar();
    await pick("Plakat gestalten");
    inBar("Status").focus();
    await userEvent.keyboard("{Escape}");
    noBar();
  });

  describe("Status", () => {
    it("offers the statuses that all selected records may take next", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      await userEvent.click(inBar("Status"));
      const options = within(screen.getByRole("listbox", { name: "Status" })).getAllByRole(
        "option",
      );
      expect(options.map((o) => o.textContent)).toEqual(["Blockiert", "Erledigt", "Abgebrochen"]);
    });

    it("changes the status of each record and says so", async () => {
      const { calls } = open({
        answers: {
          "PATCH /actions/rec-act-001": patched("rec-act-001", { status: "done" }),
          "PATCH /actions/rec-act-002": patched("rec-act-002", { status: "done" }),
        },
      });
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));

      expect(await screen.findByText("2 Einträge geändert")).toBeInTheDocument();
      expect(patchCalls(calls).map((c) => [c.call, c.body])).toEqual([
        [
          `PATCH /api/v1/events/${EVENT.id}/actions/rec-act-001`,
          { status: "done", expected_version: 1 },
        ],
        [
          `PATCH /api/v1/events/${EVENT.id}/actions/rec-act-002`,
          { status: "done", expected_version: 1 },
        ],
      ]);
      // The announcement is polite, the selection is gone, and the cards moved.
      noBar();
      expect(
        within(screen.getByRole("region", { name: /^Erledigt/ })).getByText("ACT-001"),
      ).toBeInTheDocument();
    });

    it("moves focus to the heading of the page when the bar leaves", async () => {
      open({
        answers: { "PATCH /actions/rec-act-001": patched("rec-act-001", { status: "done" }) },
      });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("1 Eintrag geändert");
      await waitFor(() =>
        expect(screen.getByRole("heading", { level: 1, name: "Arbeit" })).toHaveFocus(),
      );
    });

    it("is off for records of several kinds, and says why", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      await pick("Bäckerei liefert 120 Zöpfe");
      const status = inBar("Status");
      expect(status).toHaveAttribute("aria-disabled", "true");
      expect(status).toHaveAccessibleDescription(/nur für Einträge derselben Art/);
      // The other two still work.
      expect(inBar("Verantwortlich")).not.toHaveAttribute("aria-disabled", "true");
      expect(inBar("Bereich")).not.toHaveAttribute("aria-disabled", "true");
    });

    it("keeps a control that is off in the tab order and refuses the press", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      await pick("Bäckerei liefert 120 Zöpfe");
      const status = inBar("Status");
      expect(status).not.toBeDisabled();
      await userEvent.click(status);
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      status.focus();
      await userEvent.keyboard("{Enter}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(status).toHaveFocus();
    });

    it("is off for decisions, and says why", async () => {
      open();
      await ready();
      await pick("Das Fest findet bei jedem Wetter statt.");
      expect(inBar("Status")).toHaveAttribute("aria-disabled", "true");
      expect(inBar("Status")).toHaveAccessibleDescription(/Entscheid/);
    });

    it("is off when no status is common", async () => {
      const sample = work();
      sample["/actions"] = [
        r.action("ACT-001", { title: "Plakat gestalten", next_statuses: ["done"] }),
        r.action("ACT-002", { title: "Sponsorenliste", next_statuses: ["canceled"] }),
      ];
      open({ lists: sample });
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      expect(inBar("Status")).toHaveAttribute("aria-disabled", "true");
      expect(inBar("Status")).toHaveAccessibleDescription(/gemeinsamen/);
    });
  });

  describe("Verantwortlich and Bereich", () => {
    it("sets the owner of each selected record, also of different kinds", async () => {
      const { calls } = open({
        answers: {
          "PATCH /actions/rec-act-001": patched("rec-act-001", { owner_user_id: BERND }),
          "PATCH /commitments/rec-com-001": () =>
            json(200, { ...r.commitment("COM-001"), version: 2, owner_user_id: BERND }),
        },
      });
      await ready();
      await pick("Plakat gestalten");
      await pick("Bäckerei liefert 120 Zöpfe");
      await userEvent.click(inBar("Verantwortlich"));
      await userEvent.click(screen.getByRole("option", { name: "Bernd Beispiel" }));
      expect(await screen.findByText("2 Einträge geändert")).toBeInTheDocument();
      expect(patchCalls(calls).map((c) => c.body)).toEqual([
        { owner_user_id: BERND, expected_version: 1 },
        { owner_user_id: BERND, expected_version: 1 },
      ]);
    });

    it("searches the contributors", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Verantwortlich"));
      await userEvent.type(screen.getByRole("searchbox"), "Bern");
      expect(screen.getAllByRole("option")).toHaveLength(1);
      expect(screen.getByRole("option", { name: "Bernd Beispiel" })).toBeInTheDocument();
    });

    it("sets the workstream, offers only active ones and „Kein Bereich“", async () => {
      const { calls } = open({
        answers: { "PATCH /actions/rec-act-001": patched("rec-act-001", { workstream_id: "w2" }) },
      });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Bereich"));
      expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual([
        "Kein Bereich",
        "Bodenbetrieb",
        "Festwirtschaft",
      ]);
      await userEvent.click(screen.getByRole("option", { name: "Festwirtschaft" }));
      await screen.findByText("1 Eintrag geändert");
      expect(patchCalls(calls)[0]?.body).toEqual({ workstream_id: "w2", expected_version: 1 });
    });

    it("clears the workstream with „Kein Bereich“", async () => {
      const sample = work();
      sample["/actions"] = [
        r.action("ACT-001", { title: "Plakat gestalten", workstream_id: "w1" }),
      ];
      const { calls } = open({
        lists: sample,
        answers: { "PATCH /actions/rec-act-001": patched("rec-act-001", { workstream_id: null }) },
      });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Bereich"));
      await userEvent.click(screen.getByRole("option", { name: "Kein Bereich" }));
      await screen.findByText("1 Eintrag geändert");
      expect(patchCalls(calls)[0]?.body).toEqual({ workstream_id: null, expected_version: 1 });
    });

    it("skips a record that has the value already", async () => {
      const { calls } = open();
      await ready();
      // ACT-001 is owned by ME in the fixture: choosing ME changes nothing.
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Verantwortlich"));
      await userEvent.click(screen.getByRole("option", { name: "Anna Muster" }));
      expect(await screen.findByText("Keine Änderung nötig")).toBeInTheDocument();
      expect(patchCalls(calls)).toHaveLength(0);
      expect(bar()).toBeInTheDocument();
    });
  });

  describe("progress and failure", () => {
    it("shows the progress while the changes run, and is busy", async () => {
      const gates: ((response: Response) => void)[] = [];
      const defer = () => new Promise<Response>((resolve) => gates.push(resolve));
      open({
        answers: { "PATCH /actions/rec-act-001": defer, "PATCH /actions/rec-act-002": defer },
      });
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));

      expect(await within(bar()).findByText("0 von 2 geändert …")).toBeInTheDocument();
      // The bar and its banner are busy together.
      expect(bar().closest("[aria-busy]")).toHaveAttribute("aria-busy", "true");
      await act(async () => gates[0]?.(patched("rec-act-001", { status: "done" })()));
      expect(await within(bar()).findByText("1 von 2 geändert …")).toBeInTheDocument();
      await act(async () => gates[1]?.(patched("rec-act-002", { status: "done" })()));
      expect(await screen.findByText("2 Einträge geändert")).toBeInTheDocument();
    });

    /** Three records; the second is refused, the third has a newer version on the server. */
    async function failed() {
      const sample = work();
      const answers: NonNullable<Setup["answers"]> = {
        "PATCH /actions/rec-act-001": patched("rec-act-001", { status: "done" }),
        "PATCH /actions/rec-act-002": () => problem(403, "forbidden"),
        "PATCH /actions/rec-act-006": () => problem(409, "record-version-conflict"),
      };
      const { calls } = open({ lists: sample, answers });
      await ready();
      for (const title of ["Plakat gestalten", "Sponsorenliste", "Rechnung bezahlen"]) {
        await pick(title);
      }
      // The server has moved on by the time the list is loaded again.
      sample["/actions"] = [
        sample["/actions"]?.[0] as never,
        sample["/actions"]?.[1] as never,
        r.action("ACT-006", {
          title: "Rechnung bezahlen",
          version: 7,
          next_statuses: NEXT,
        }) as never,
      ];
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("2 von 3 Einträgen nicht geändert");
      return { calls, answers };
    }

    // The selection is in the order of the board (column by column), so the list of failures is too.
    it("lists what failed and why, keeps those selected, and reloads after a conflict", async () => {
      const { calls } = await failed();
      const list = screen.getByRole("list", { name: "Nicht geändert" });
      expect(
        within(list)
          .getAllByRole("listitem")
          .map((li) => li.textContent),
      ).toEqual(["ACT-006: Konflikt", "ACT-002: keine Berechtigung"]);
      // The record that changed is not selected any more; the two that failed are.
      expect(within(bar()).getByText("2 ausgewählt")).toBeInTheDocument();
      expect(
        screen.getByRole("checkbox", { name: "ACT-001 Plakat gestalten auswählen" }),
      ).not.toBeChecked();
      // The conflict loaded the actions again.
      expect(calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/actions`)).toHaveLength(
        2,
      );
      // Focus is on the banner, which is also a polite message.
      expect(
        screen.getByText("2 von 3 Einträgen nicht geändert").closest("[tabindex]"),
      ).toHaveFocus();
      expect(screen.getByRole("status")).toBeInTheDocument();
    });

    it("retries only the failed records, with the version of the reload", async () => {
      const { calls, answers } = await failed();
      answers["PATCH /actions/rec-act-002"] = patched("rec-act-002", { status: "done" });
      answers["PATCH /actions/rec-act-006"] = patched("rec-act-006", { status: "done" });
      const before = patchCalls(calls).length;
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      await waitFor(() => expect(retry).toBeEnabled());
      await userEvent.click(retry);

      expect(await screen.findByText("3 Einträge geändert")).toBeInTheDocument();
      expect(
        patchCalls(calls)
          .slice(before)
          .map((c) => [c.call.split("/").pop(), c.body]),
      ).toEqual([
        ["rec-act-002", { status: "done", expected_version: 1 }],
        ["rec-act-006", { status: "done", expected_version: 7 }],
      ]);
      noBar();
    });

    it("keeps the failures when the retry fails again", async () => {
      await failed();
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      await waitFor(() => expect(retry).toBeEnabled());
      await userEvent.click(retry);
      await waitFor(() =>
        expect(screen.getByRole("button", { name: "Erneut versuchen" })).toBeEnabled(),
      );
      expect(screen.getByText("2 von 2 Einträgen nicht geändert")).toBeInTheDocument();
      expect(within(bar()).getByText("2 ausgewählt")).toBeInTheDocument();
    });

    it("names a failure that has no short word with the message of the server", async () => {
      open({ answers: { "PATCH /actions/rec-act-001": () => problem(500, "internal") } });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      const list = await screen.findByRole("list", { name: "Nicht geändert" });
      expect(within(list).getByText(/^ACT-001: .+/)).toBeInTheDocument();
    });
  });

  describe("toolbar", () => {
    it("is one tab stop, and the arrow keys move between the buttons", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      inBar("Status").focus();
      await userEvent.keyboard("{ArrowRight}");
      expect(inBar("Verantwortlich")).toHaveFocus();
      await userEvent.keyboard("{ArrowRight}{ArrowRight}");
      expect(inBar("Auswahl aufheben")).toHaveFocus();
      await userEvent.keyboard("{ArrowLeft}");
      expect(inBar("Bereich")).toHaveFocus();
    });

    it("reaches a button that is off with the arrow keys", async () => {
      open();
      await ready();
      await pick("Plakat gestalten");
      await pick("Bäckerei liefert 120 Zöpfe");
      inBar("Verantwortlich").focus();
      await userEvent.keyboard("{ArrowLeft}");
      expect(inBar("Status")).toHaveFocus();
    });
  });

  describe("during a run", () => {
    /** Two records whose changes wait for the test. */
    async function running() {
      const gates: ((response: Response) => void)[] = [];
      const defer = () => new Promise<Response>((resolve) => gates.push(resolve));
      open({
        answers: { "PATCH /actions/rec-act-001": defer, "PATCH /actions/rec-act-002": defer },
      });
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await within(bar()).findByText("0 von 2 geändert …");
      return gates;
    }

    /** Starts the run on the board, then opens another view while the answers wait. */
    async function runningIn(view: "Tabelle" | "Zeitplan") {
      const gates = await running();
      await userEvent.click(screen.getByRole("tab", { name: view }));
      return gates;
    }
    const tableOpen = () => screen.findByRole("table", { name: "Arbeit als Tabelle" });
    const doneTwice = () => screen.queryAllByText("2 Einträge geändert");

    it("delivers the result once when the board gives way to the table in the middle of a run", async () => {
      const gates = await runningIn("Tabelle");
      await tableOpen();
      expect(within(bar()).getByText("0 von 2 geändert …")).toBeInTheDocument();
      await act(async () => gates[0]?.(patched("rec-act-001", { status: "done" })()));
      await act(async () => gates[1]?.(patched("rec-act-002", { status: "done" })()));
      await screen.findByText("2 Einträge geändert");
      expect(doneTwice()).toHaveLength(1);
      noBar();
      expect(screen.queryAllByRole("checkbox", { checked: true })).toHaveLength(0);
    });

    it("shows no bar in the timeline during a run and clears the selection when the run succeeds", async () => {
      const gates = await runningIn("Zeitplan");
      await screen.findByRole("region", { name: "Zeitachse" });
      noBar();
      await act(async () => gates[0]?.(patched("rec-act-001", { status: "done" })()));
      await act(async () => gates[1]?.(patched("rec-act-002", { status: "done" })()));
      await screen.findByText("2 Einträge geändert");
      expect(doneTwice()).toHaveLength(1);
      noBar();
      await userEvent.click(screen.getByRole("tab", { name: "Tabelle" }));
      await tableOpen();
      noBar();
      expect(screen.queryAllByRole("checkbox", { checked: true })).toHaveLength(0);
    });

    it("keeps the failures of a run that ends in the timeline for the return to the board", async () => {
      const gates = await runningIn("Zeitplan");
      await screen.findByRole("region", { name: "Zeitachse" });
      await act(async () => gates[0]?.(problem(403, "forbidden")));
      await act(async () => gates[1]?.(patched("rec-act-002", { status: "done" })()));
      await screen.findByText("1 Eintrag konnte nicht geändert werden.");
      noBar();
      await userEvent.click(screen.getByRole("tab", { name: "Board" }));
      await ready();
      expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
      expect(screen.getByText("1 von 2 Einträgen nicht geändert")).toBeInTheDocument();
      expect(screen.getByRole("checkbox", { name: /Plakat gestalten auswählen$/ })).toBeChecked();
    });

    it("keeps focus on the trigger that started the run", async () => {
      await running();
      expect(inBar("Status")).toHaveFocus();
      expect(inBar("Status")).toHaveAttribute("aria-disabled", "true");
    });

    it("does not clear the selection, with the button or with Escape", async () => {
      const gates = await running();
      await userEvent.click(inBar("Auswahl aufheben"));
      await userEvent.keyboard("{Escape}");
      screen.getByRole("link", { name: /^ACT-006/ }).focus();
      await userEvent.keyboard("{Escape}");
      expect(bar()).toBeInTheDocument();
      expect(within(bar()).getByText("0 von 2 geändert …")).toBeInTheDocument();
      // The cards cannot be selected or deselected either.
      expect(screen.getByRole("checkbox", { name: /Plakat gestalten auswählen$/ })).toBeDisabled();
      await act(async () => gates[0]?.(problem(403, "forbidden")));
      await act(async () => gates[1]?.(problem(403, "forbidden")));
      expect(await screen.findByText("2 von 2 Einträgen nicht geändert")).toBeInTheDocument();
      expect(within(bar()).getByText("2 ausgewählt")).toBeInTheDocument();
    });

    it("keeps focus on the retry button, and moves it to the banner on a second failure", async () => {
      const answers: NonNullable<Setup["answers"]> = {
        "PATCH /actions/rec-act-001": () => problem(403, "forbidden"),
      };
      open({ answers });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("1 von 1 Einträgen nicht geändert");
      await waitFor(() =>
        expect(
          screen.getByText("1 von 1 Einträgen nicht geändert").closest("[tabindex]"),
        ).toHaveFocus(),
      );
      const gate: { release?: (response: Response) => void } = {};
      answers["PATCH /actions/rec-act-001"] = () =>
        new Promise<Response>((resolve) => (gate.release = resolve));
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      retry.focus();
      await userEvent.keyboard("{Enter}");
      // The banner stays while the retry runs, and so does focus.
      await waitFor(() => expect(gate.release).toBeDefined());
      expect(screen.getByRole("button", { name: "Erneut versuchen" })).toHaveFocus();
      await act(async () => gate.release?.(problem(403, "forbidden")));
      await waitFor(() =>
        expect(
          screen.getByText("1 von 1 Einträgen nicht geändert").closest("[tabindex]"),
        ).toHaveFocus(),
      );
    });

    it("leaves focus where it is when the run ends outside the bar", async () => {
      const gates = await running();
      filterButton().focus();
      await act(async () => gates[0]?.(patched("rec-act-001", { status: "done" })()));
      await act(async () => gates[1]?.(patched("rec-act-002", { status: "done" })()));
      await screen.findByText("2 Einträge geändert");
      expect(filterButton()).toHaveFocus();
    });

    it("leaves focus where it is when a failed run ends outside the bar", async () => {
      const gates = await running();
      filterButton().focus();
      await act(async () => gates[0]?.(problem(403, "forbidden")));
      await act(async () => gates[1]?.(problem(403, "forbidden")));
      await screen.findByText("2 von 2 Einträgen nicht geändert");
      expect(filterButton()).toHaveFocus();
    });

    it("says why in the description of every control while the run is busy", async () => {
      await running();
      for (const name of ["Status", "Verantwortlich", "Bereich", "Auswahl aufheben"]) {
        expect(inBar(name)).toHaveAccessibleDescription("Die vorige Änderung läuft noch.");
      }
    });

    it("says why in the description of the retry button while a retry runs", async () => {
      const answers: NonNullable<Setup["answers"]> = {
        "PATCH /actions/rec-act-001": () => problem(403, "forbidden"),
      };
      open({ answers });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("1 von 1 Einträgen nicht geändert");
      const gate: { release?: (response: Response) => void } = {};
      answers["PATCH /actions/rec-act-001"] = () =>
        new Promise<Response>((resolve) => (gate.release = resolve));
      await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
      await waitFor(() => expect(gate.release).toBeDefined());
      expect(screen.getByRole("button", { name: "Erneut versuchen" })).toHaveAccessibleDescription(
        "Die vorige Änderung läuft noch.",
      );
      await act(async () => gate.release?.(problem(403, "forbidden")));
    });

    it("lists and retries only the failures that are still selected and in sight", async () => {
      const { calls } = open({
        answers: {
          "PATCH /actions/rec-act-001": () => problem(403, "forbidden"),
          "PATCH /actions/rec-act-002": () => problem(403, "forbidden"),
        },
      });
      await ready();
      await pick("Plakat gestalten");
      await pick("Sponsorenliste");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("2 von 2 Einträgen nicht geändert");
      const retry = screen.getByRole("button", { name: "Erneut versuchen" });
      await waitFor(() => expect(retry).toBeEnabled());
      // A search hides the second record: it leaves the selection, so it leaves the list.
      await userEvent.click(filterButton());
      await userEvent.type(screen.getByRole("searchbox"), "Plakat");
      await userEvent.click(filterButton());
      await waitFor(() => expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument());
      const list = screen.getByRole("list", { name: "Nicht geändert" });
      expect(
        within(list)
          .getAllByRole("listitem")
          .map((li) => li.textContent),
      ).toEqual(["ACT-001: keine Berechtigung"]);
      const before = patchCalls(calls).length;
      await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));
      await waitFor(() => expect(patchCalls(calls).length).toBeGreaterThan(before));
      expect(
        patchCalls(calls)
          .slice(before)
          .map((c) => c.call.split("/").pop()),
      ).toEqual(["rec-act-001"]);
    });

    it("keeps the cards when the reload after a conflict fails", async () => {
      let loads = 0;
      open({
        answers: {
          "PATCH /actions/rec-act-001": () => problem(409, "record-version-conflict"),
          "GET /actions": () => {
            loads += 1;
            return loads === 1
              ? json(200, { items: work()["/actions"] })
              : problem(500, "internal");
          },
        },
      });
      await ready();
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Status"));
      await userEvent.click(screen.getByRole("option", { name: "Erledigt" }));
      await screen.findByText("1 von 1 Einträgen nicht geändert");
      await waitFor(() => expect(loads).toBe(2));
      expect(screen.getByText("ACT-001")).toBeInTheDocument();
      expect(within(bar()).getByText("1 ausgewählt")).toBeInTheDocument();
      expect(screen.getByRole("list", { name: "Nicht geändert" })).toBeInTheDocument();
    });
  });

  describe("on a phone", () => {
    it("opens the pickers as a sheet", async () => {
      const { calls } = open({
        narrow: true,
        answers: { "PATCH /actions/rec-act-001": patched("rec-act-001", { owner_user_id: BERND }) },
      });
      await screen.findByRole("button", { name: /^Offen/ });
      await pick("Plakat gestalten");
      await userEvent.click(inBar("Verantwortlich"));
      const sheet = await screen.findByRole("dialog", { name: "Verantwortlich" });
      await userEvent.click(within(sheet).getByRole("option", { name: "Bernd Beispiel" }));
      expect(await screen.findByText("1 Eintrag geändert")).toBeInTheDocument();
      expect(patchCalls(calls)[0]?.body).toEqual({ owner_user_id: BERND, expected_version: 1 });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("keeps the bar as a toolbar with the same buttons", async () => {
      open({ narrow: true });
      await screen.findByRole("button", { name: /^Offen/ });
      await pick("Plakat gestalten");
      for (const name of ["Status", "Verantwortlich", "Bereich", "Auswahl aufheben"]) {
        expect(inBar(name)).toBeInTheDocument();
      }
    });
  });
});
