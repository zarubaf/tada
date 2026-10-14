import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import {
  BERND,
  CLARA,
  EVENT,
  json,
  ME,
  problem,
  renderWork,
  type Setup,
} from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
];
const user = userEvent.setup({ delay: null });

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

type Body = Record<string, unknown>;

/**
 * Opens ACT-001 in the panel. `patch` answers each change; by default it applies the body to the
 * record and raises the version, as the server does.
 */
function openAction(
  over: Body = {},
  setup: Setup = {},
  patch?: (body: Body, current: Body) => Response | Promise<Response>,
) {
  const work: Record<string, unknown[]> = r.sampleWork();
  let current: Body = r.action("ACT-001", {
    title: "Plakat gestalten",
    owner_user_id: BERND,
    due_date: "2030-05-18",
    workstream_id: "w1",
    description: "Alte Beschreibung",
    next_statuses: ["in-progress", "blocked", "done", "canceled"],
    ...over,
  });
  work["/actions"] = [current, ...(work["/actions"] ?? []).slice(1)];
  work["/milestones"] = [
    r.milestone("MIL-001", { title: "Programm steht", id: "m1" }),
    r.milestone("MIL-002", { title: "Fest vorbei", id: "m2" }),
  ];
  const result = renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work#ACT-001`,
      lists: work,
      workstreams: STREAMS,
      answers: {
        "PATCH /actions/rec-act-001": (_url, body) => {
          const changes = body as Body;
          if (patch) {
            return patch(changes, current);
          }
          const { expected_version: _, ...fields } = changes;
          current = { ...current, ...fields, version: (current.version as number) + 1 };
          return json(200, current);
        },
      },
      ...setup,
    },
  );
  return {
    ...result,
    /** The lists that the fake server serves: change one to play another person's change. */
    work,
    /** The bodies of the changes that were sent. */
    sent: () => result.calls.filter((c) => c.call.startsWith("PATCH")).map((c) => c.body as Body),
  };
}

const panel = () => screen.findByRole("complementary", { name: /ACT-001/ }, { timeout: 3000 });
/** The panel that has loaded, for the queries inside it. */
async function region() {
  return within(await panel());
}

describe("editing an action in the panel", () => {
  describe("the title", () => {
    it("edits on a click and saves with Enter, with the version of the list", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Titel bearbeiten: Plakat gestalten" }));
      const input = view.getByRole("textbox", { name: "Titel" });
      expect(input).toHaveFocus();
      await user.clear(input);
      await user.type(input, "Plakat entwerfen{Enter}");

      await view.findByRole("button", { name: "Titel bearbeiten: Plakat entwerfen" });
      expect(server.sent()).toEqual([{ title: "Plakat entwerfen", expected_version: 1 }]);
      // The list shows the new title, and the member hears the save.
      expect(screen.getByRole("link", { name: /ACT-001 Plakat entwerfen/ })).toBeInTheDocument();
      expect(screen.getByText("Titel von ACT-001 gespeichert.")).toBeInTheDocument();
      // Focus is back on the title, which now reads as text again.
      expect(view.getByRole("button", { name: /Titel bearbeiten/ })).toHaveFocus();
    });

    it("marks the field busy while it saves", async () => {
      let release: (response: Response) => void = () => undefined;
      openAction({}, {}, () => new Promise<Response>((resolve) => (release = resolve)));
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "!{Enter}");

      const field = view.getByRole("textbox", { name: "Titel" }).closest("[aria-busy]");
      expect(field).toHaveAttribute("aria-busy", "true");
      release(json(200, r.action("ACT-001", { title: "Plakat gestalten!", version: 2 })));
      await view.findByRole("button", { name: /Plakat gestalten!/ });
    });

    it("cancels with Escape and keeps the panel open; the next Escape closes it", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "xyz{Escape}");

      expect(
        view.getByRole("button", { name: "Titel bearbeiten: Plakat gestalten" }),
      ).toHaveFocus();
      expect(server.sent()).toEqual([]);
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
    });

    it("saves on blur", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "!");
      await user.tab();

      await view.findByRole("button", { name: /Plakat gestalten!/ });
      expect(server.sent()).toEqual([{ title: "Plakat gestalten!", expected_version: 1 }]);
    });

    it("sends nothing when the text did not change", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.keyboard("{Enter}");

      expect(await view.findByRole("button", { name: /Titel bearbeiten/ })).toBeInTheDocument();
      expect(server.sent()).toEqual([]);
    });

    it("refuses an empty title at the field and sends nothing", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.clear(view.getByRole("textbox", { name: "Titel" }));
      await user.keyboard("{Enter}");

      expect(await view.findByText("Der Titel hat 1 bis 200 Zeichen.")).toBeVisible();
      expect(view.getByRole("textbox", { name: "Titel" })).toBeInvalid();
      expect(server.sent()).toEqual([]);
    });

    it("shows a failure at the field, keeps the draft and saves it again on request", async () => {
      let calls = 0;
      const server = openAction({}, {}, (body, current) =>
        ++calls === 1 ? problem(500, "internal") : json(200, { ...current, ...body, version: 2 }),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.clear(view.getByRole("textbox", { name: "Titel" }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "Neuer Titel{Enter}");

      const input = await view.findByRole("textbox", { name: "Titel" });
      await waitFor(() => expect(input).toHaveAccessibleDescription(/\S/));
      expect(input).toHaveValue("Neuer Titel");
      // The member hears the failure at once, with the field and the record.
      expect(await screen.findByText(/^Titel von ACT-001 nicht gespeichert\./)).toBeInTheDocument();

      await user.click(input);
      await user.keyboard("{Enter}");
      await view.findByRole("button", { name: "Titel bearbeiten: Neuer Titel" });
      expect(server.sent()).toHaveLength(2);
    });

    it("tells the member of a conflict, reloads the record and keeps the draft", async () => {
      const server = openAction({}, {}, () => problem(409, "record-version-conflict"));
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.clear(view.getByRole("textbox", { name: "Titel" }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "Mein Titel{Enter}");

      const input = await view.findByRole("textbox", { name: "Titel" });
      await waitFor(() => expect(input).toHaveAccessibleDescription(/inzwischen geändert/));
      expect(input).toHaveValue("Mein Titel");
      // The list was loaded again.
      await waitFor(() =>
        expect(
          server.calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/actions`).length,
        ).toBeGreaterThan(1),
      );
    });
  });

  describe("a conflict", () => {
    /** Opens the title with a draft, and the server answers that another person was faster. */
    async function conflict() {
      let calls = 0;
      const served: { lists?: Record<string, unknown[]> } = {};
      const server = openAction({}, {}, (body, current) => {
        if (++calls === 1) {
          const lists = served.lists ?? {};
          lists["/actions"] = [{ ...current, title: "Titel von Bernd", version: 2 }];
          return problem(409, "record-version-conflict");
        }
        return json(200, { ...current, ...body, version: 3 });
      });
      served.lists = server.work;
      const view = await region();
      await user.click(view.getByRole("button", { name: /Titel bearbeiten/ }));
      await user.clear(view.getByRole("textbox", { name: "Titel" }));
      await user.type(view.getByRole("textbox", { name: "Titel" }), "Mein Titel{Enter}");
      await view.findByText("Jetzt gespeichert: Titel von Bernd");
      return { server, view };
    }

    it("shows the value on the server beside the draft", async () => {
      const { view } = await conflict();
      expect(view.getByRole("textbox", { name: "Titel" })).toHaveValue("Mein Titel");
    });

    it("does not save the draft over the other change when the member clicks elsewhere", async () => {
      const { server, view } = await conflict();
      await user.click(screen.getByRole("heading", { level: 1 }));

      expect(view.getByRole("textbox", { name: "Titel" })).toHaveValue("Mein Titel");
      expect(server.sent()).toHaveLength(1);
    });

    it("saves the draft with Enter, with the version that was loaded after the conflict", async () => {
      const { server, view } = await conflict();
      await user.click(view.getByRole("textbox", { name: "Titel" }));
      await user.keyboard("{Enter}");

      await view.findByRole("button", { name: "Titel bearbeiten: Mein Titel" });
      expect(server.sent()[1]).toEqual({ title: "Mein Titel", expected_version: 2 });
    });

    it("saves the draft with Speichern", async () => {
      const { server, view } = await conflict();
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByRole("button", { name: "Titel bearbeiten: Mein Titel" });
      expect(server.sent()).toHaveLength(2);
    });
  });

  describe("the pickers", () => {
    it("changes the status among the next ones of the record, without a search field", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: offen" }));
      const list = await screen.findByRole("listbox", { name: "Status" });
      expect(
        within(list)
          .getAllByRole("option")
          .map((o) => o.textContent),
      ).toEqual(["offen", "in Arbeit", "blockiert", "erledigt", "abgebrochen"]);
      expect(screen.queryByRole("searchbox")).toBeNull();
      await user.click(within(list).getByRole("option", { name: "in Arbeit" }));

      await view.findByRole("button", { name: "Status: in Arbeit" });
      expect(server.sent()).toEqual([{ status: "in-progress", expected_version: 1 }]);
      // The head and the list follow.
      expect(screen.getByText("Status von ACT-001 gespeichert.")).toBeInTheDocument();
    });

    it("offers a status only if the record can move", async () => {
      openAction({ next_statuses: [] });
      const view = await region();
      expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
    });

    it("changes the owner among the contributors, and cannot clear it", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
      const list = await screen.findByRole("listbox", { name: "Verantwortlich" });
      // The viewer of the fake event cannot own a record.
      expect(within(list).queryByRole("option", { name: "Cäcilia Probst" })).toBeNull();
      expect(screen.queryByRole("button", { name: /Niemand|Kein/ })).toBeNull();
      await user.click(within(list).getByRole("option", { name: "Anna Muster" }));

      await view.findByRole("button", { name: "Verantwortlich: Anna Muster" });
      expect(server.sent()).toEqual([{ owner_user_id: ME, expected_version: 1 }]);
    });

    it("keeps the choice after a failure and offers to try again or to discard it", async () => {
      let calls = 0;
      const server = openAction({}, {}, (body, current) =>
        ++calls === 1 ? problem(500, "internal") : json(200, { ...current, ...body, version: 2 }),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
      await user.click(await screen.findByRole("option", { name: "Anna Muster" }));

      const group = await view.findByRole("group", { name: "Verantwortlich" });
      expect(view.getByRole("button", { name: "Verantwortlich: Anna Muster" })).toBeVisible();
      expect(group).toHaveAccessibleDescription(/\S/);
      await user.click(within(group).getByRole("button", { name: "Erneut versuchen" }));
      await waitFor(() => expect(view.queryByRole("group", { name: "Verantwortlich" })).toBeNull());
      expect(server.sent()).toHaveLength(2);
    });

    it("discards a refused choice", async () => {
      openAction({}, {}, () => problem(403, "forbidden"));
      const view = await region();
      await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
      await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
      const group = await view.findByRole("group", { name: "Verantwortlich" });
      await user.click(within(group).getByRole("button", { name: "Verwerfen" }));

      expect(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
    });

    it("changes the area and clears it with Kein Bereich", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
      await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
      await view.findByRole("button", { name: "Bereich: Festwirtschaft" });

      await user.click(view.getByRole("button", { name: "Bereich: Festwirtschaft" }));
      await user.click(await screen.findByRole("button", { name: "Kein Bereich" }));
      await view.findByRole("button", { name: "Bereich: Kein Bereich" });

      expect(server.sent()).toEqual([
        { workstream_id: "w2", expected_version: 1 },
        { workstream_id: null, expected_version: 2 },
      ]);
    });

    it("links a milestone of the event and clears it", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Meilenstein: Kein Meilenstein" }));
      await user.type(await screen.findByRole("searchbox"), "fest");
      await user.click(await screen.findByRole("option", { name: "MIL-002 Fest vorbei" }));
      await view.findByRole("button", { name: "Meilenstein: MIL-002 Fest vorbei" });

      await user.click(view.getByRole("button", { name: /^Meilenstein:/ }));
      await user.click(await screen.findByRole("button", { name: "Kein Meilenstein" }));
      await view.findByRole("button", { name: "Meilenstein: Kein Meilenstein" });

      expect(server.sent()).toEqual([
        { milestone_id: "m2", expected_version: 1 },
        { milestone_id: null, expected_version: 2 },
      ]);
    });
  });

  describe("the dates", () => {
    it("sets and clears the due date", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Fällig bearbeiten: 18\.05\.2030/ }));
      const input = view.getByLabelText("Fällig");
      expect(input).toHaveAttribute("type", "date");
      expect(input).toHaveValue("2030-05-18");
      await user.clear(input);
      await user.type(input, "2030-06-01{Enter}");
      await view.findByRole("button", { name: /^Fällig bearbeiten: 01\.06\.2030/ });

      await user.click(view.getByRole("button", { name: /^Fällig bearbeiten/ }));
      await user.clear(view.getByLabelText("Fällig"));
      await user.keyboard("{Enter}");
      await view.findByRole("button", { name: "Fällig bearbeiten: Kein Datum" });

      expect(server.sent()).toEqual([
        { due_date: "2030-06-01", expected_version: 1 },
        { due_date: null, expected_version: 2 },
      ]);
    });

    it("shows the message of the server at the start date", async () => {
      openAction({}, {}, () =>
        problem(422, "validation-failed", [{ pointer: "/start_date", code: "start-after-due" }]),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: "Startdatum bearbeiten: Kein Datum" }));
      await user.type(view.getByLabelText("Startdatum"), "2031-01-01{Enter}");

      const input = await view.findByLabelText("Startdatum");
      await waitFor(() =>
        expect(input).toHaveAccessibleDescription(/Das Startdatum liegt nach dem Fälligkeitsdatum/),
      );
      expect(input).toHaveValue("2031-01-01");
    });

    it("shows the order error of the server at the due date, which the member edits", async () => {
      // The server names the start date for a due date before it (crates/app checks.rs).
      openAction({ start_date: "2030-05-10" }, {}, () =>
        problem(422, "validation-failed", [{ pointer: "/start_date", code: "start-after-due" }]),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Fällig bearbeiten/ }));
      await user.clear(view.getByLabelText("Fällig"));
      await user.type(view.getByLabelText("Fällig"), "2030-06-01{Enter}");

      await waitFor(() =>
        expect(view.getByLabelText("Fällig")).toHaveAccessibleDescription(
          "Fälligkeit liegt vor dem Start.",
        ),
      );
    });

    it("sends no due date before the start date", async () => {
      const server = openAction({ start_date: "2030-05-10" });
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Fällig bearbeiten/ }));
      await user.clear(view.getByLabelText("Fällig"));
      await user.type(view.getByLabelText("Fällig"), "2030-05-01{Enter}");

      await waitFor(() =>
        expect(view.getByLabelText("Fällig")).toHaveAccessibleDescription(
          "Fälligkeit liegt vor dem Start.",
        ),
      );
      expect(server.sent()).toEqual([]);
    });

    it("sends no start date after the due date", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Startdatum bearbeiten: Kein Datum" }));
      await user.type(view.getByLabelText("Startdatum"), "2031-01-01{Enter}");

      await waitFor(() =>
        expect(view.getByLabelText("Startdatum")).toHaveAccessibleDescription(
          /Das Startdatum liegt nach dem Fälligkeitsdatum/,
        ),
      );
      expect(server.sent()).toEqual([]);
    });
  });

  describe("the description", () => {
    it("edits in place with Speichern and Abbrechen, and Enter adds a line", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Beschreibung bearbeiten/ }));
      const text = view.getByRole("textbox", { name: "Beschreibung" });
      await user.clear(text);
      await user.type(text, "Zeile eins{Enter}Zeile zwei");
      expect(text).toHaveValue("Zeile eins\nZeile zwei");
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByRole("button", { name: /^Beschreibung bearbeiten: Zeile eins/ });
      expect(server.sent()).toEqual([
        { description: "Zeile eins\nZeile zwei", expected_version: 1 },
      ]);
    });

    it("discards the draft with Abbrechen", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Beschreibung bearbeiten/ }));
      await user.type(view.getByRole("textbox", { name: "Beschreibung" }), " mehr");
      await user.click(view.getByRole("button", { name: "Abbrechen" }));

      expect(view.getByText("Alte Beschreibung")).toBeVisible();
      expect(server.sent()).toEqual([]);
    });

    it("clears the description when the text is empty", async () => {
      const server = openAction();
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Beschreibung bearbeiten/ }));
      await user.clear(view.getByRole("textbox", { name: "Beschreibung" }));
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByRole("button", { name: "Beschreibung bearbeiten: Keine Beschreibung" });
      expect(server.sent()).toEqual([{ description: null, expected_version: 1 }]);
    });
  });

  describe("who may change it", () => {
    it("shows plain text for a record that the member may not change", async () => {
      openAction({ can_change: false });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten/ })).toBeNull();
      expect(
        view.queryByRole("button", { name: /^(Status|Verantwortlich|Bereich|Meilenstein):/ }),
      ).toBeNull();
      expect(view.getByRole("heading", { level: 2, name: "Plakat gestalten" })).toBeVisible();
      expect(view.getByText("Bernd Beispiel")).toBeVisible();
      expect(view.getByText("Alte Beschreibung")).toBeVisible();
    });

    it("shows plain text in a closed event", async () => {
      openAction({}, { event: { ...EVENT, status: "closed" } });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten/ })).toBeNull();
      expect(view.getByText("Bernd Beispiel")).toBeVisible();
    });

    it("has no control for a redacted record", async () => {
      openAction({ redacted: true, title: "", description: null });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten/ })).toBeNull();
      expect(view.getByText("entfernt")).toBeVisible();
    });

    it("keeps the owner readable when the directory does not list the member", async () => {
      openAction({ owner_user_id: CLARA });
      const view = await region();
      expect(view.getByRole("button", { name: "Verantwortlich: Cäcilia Probst" })).toBeVisible();
    });
  });
});
