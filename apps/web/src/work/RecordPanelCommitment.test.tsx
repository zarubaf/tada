import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { BERND, EVENT, json, ME, problem, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
];
const user = userEvent.setup({ delay: null });
// The search of the promisor waits 250 ms after the last key.
const SEARCH = { timeout: 3000 };
const BAKERY = {
  kind: "institution",
  id: "i1",
  local_id: "INS-001",
  name: "Bäckerei Zopf AG",
  redacted: false,
};

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

type Body = Record<string, unknown>;

const evidence = {
  proposal_id: "p1",
  record_version: 1,
  source_version_id: "s1",
  captured_at: "2030-03-01T10:00:00Z",
  start_offset: 0,
  end_offset: 5,
  quote: "Wir liefern, wenn bestellt.",
  redacted: false,
};

/** COM-001 as the server holds it: conditional, with every field the panel shows. */
const commitment = (over: Body = {}) =>
  r.commitment("COM-001", {
    text: "Bäckerei liefert 120 Zöpfe",
    condition: "wenn der Auftrag unterschrieben ist",
    promisor: BAKERY,
    owner_user_id: BERND,
    due_date: "2030-06-01",
    workstream_id: "w1",
    can_make_firm: true,
    next_statuses: ["fulfilled", "broken", "withdrawn"],
    evidence: [evidence],
    ...over,
  });

function openWork(
  search: string,
  setup: Setup = {},
  handlers: {
    patch?: (body: Body, current: Body) => Response | Promise<Response>;
    firm?: (body: Body, current: Body) => Response | Promise<Response>;
    post?: (body: Body) => Response | Promise<Response>;
  } = {},
  over: Body = {},
) {
  const work: Record<string, unknown[]> = r.sampleWork();
  let current: Body = commitment(over);
  work["/commitments"] = [current];
  work["/milestones"] = [r.milestone("MIL-001", { title: "Programm steht", id: "m1" })];
  work["/persons"] = [];
  work["/institutions"] = [{ ...BAKERY, version: 1 }];
  const result = renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work${search}`,
      lists: work,
      workstreams: STREAMS,
      ...setup,
      answers: {
        "PATCH /commitments/rec-com-001": (_url, body) => {
          const changes = body as Body;
          if (handlers.patch) {
            return handlers.patch(changes, current);
          }
          const { expected_version: _, ...fields } = changes;
          current = { ...current, ...fields, version: (current.version as number) + 1 };
          work["/commitments"] = [current];
          return json(200, current);
        },
        "POST /commitments/rec-com-001/firm": (_url, body) => {
          const changes = body as Body;
          if (handlers.firm) {
            return handlers.firm(changes, current);
          }
          current = {
            ...current,
            status: "firm",
            firm_reason: changes.reason,
            can_make_firm: false,
            next_statuses: ["fulfilled", "broken", "withdrawn"],
            version: (current.version as number) + 1,
          };
          work["/commitments"] = [current];
          return json(200, current);
        },
        "POST /commitments": (_url, body) =>
          handlers.post
            ? handlers.post(body as Body)
            : json(
                201,
                commitment({
                  id: "rec-com-020",
                  local_id: "COM-020",
                  ...(body as Body),
                  promisor: BAKERY,
                }),
              ),
        ...setup.answers,
      },
    },
  );
  const sent = (method: string) =>
    result.calls.filter((c) => c.call.startsWith(method)).map((c) => c.body as Body);
  return { ...result, work, sent };
}

const panel = () => screen.findByRole("complementary", { name: /COM-001/ }, { timeout: 3000 });
async function region() {
  return within(await panel());
}
const form = async () =>
  within(await screen.findByRole("complementary", { name: "Neue Zusage" }, { timeout: 3000 }));

describe("a commitment in the panel", () => {
  describe("opening", () => {
    it("opens from the hash, with the text as the heading and the fields of a commitment", async () => {
      openWork("#COM-001");
      const view = await region();
      expect(
        view.getByRole("heading", { level: 2, name: "Bäckerei liefert 120 Zöpfe" }),
      ).toBeVisible();
      expect(view.getByText("COM-001")).toBeVisible();
      expect(view.getByText("Zusage")).toBeVisible();
      expect(view.getByRole("button", { name: "Status: bedingt" })).toBeVisible();
      expect(view.getByText("Zugesagt von")).toBeVisible();
      expect(view.getByText("Bäckerei Zopf AG")).toBeVisible();
      expect(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
      expect(view.getByText("wenn der Auftrag unterschrieben ist")).toBeVisible();
      expect(view.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeVisible();
      expect(view.getByText("01.06.2030")).toBeVisible();
    });

    it("does not send the member to the old page any more", async () => {
      openWork("#COM-001");
      await panel();
      expect(screen.queryByText("Eintrag öffnet noch nicht hier")).toBeNull();
    });

    it("says that there is no condition", async () => {
      openWork("#COM-001", {}, {}, { condition: null, status: "firm", can_make_firm: false });
      const view = await region();
      expect(view.getByText("Keine Bedingung")).toBeVisible();
    });

    it("lists the evidence as Quellen with the version of the commitment that it supports", async () => {
      openWork("#COM-001");
      const view = await region();
      expect(view.getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
      expect(view.getByText("Wir liefern, wenn bestellt.")).toBeVisible();
      expect(view.getByText("Stützt die Version 1 der Zusage")).toBeVisible();
    });

    it("shows the promisor and the condition as plain text: they never change", async () => {
      openWork("#COM-001");
      const view = await region();
      expect(view.queryByRole("button", { name: /^Zugesagt von:/ })).toBeNull();
      expect(view.queryByRole("button", { name: /^Bedingung:/ })).toBeNull();
    });

    it("steps over to the next record of the list and back", async () => {
      openWork("#COM-001");
      await panel();
      expect(screen.getByRole("button", { name: "Nächster Eintrag" })).toBeVisible();
    });

    it("shows the tombstone for a redacted commitment, and offers no control", async () => {
      openWork(
        "#COM-001",
        {},
        {},
        {
          text: "",
          condition: null,
          promisor: { ...BAKERY, name: "", redacted: true },
          redacted: true,
        },
      );
      const view = await region();
      expect(view.getAllByText("entfernt").length).toBeGreaterThanOrEqual(2);
      expect(view.queryByRole("button", { name: /^Status:|^Verantwortlich:/ })).toBeNull();
      expect(view.queryByRole("button", { name: "Aktionen" })).toBeNull();
    });
  });

  describe("a redacted person on a commitment", () => {
    /** The text of the row that the label names. */
    const rowOf = (view: ReturnType<typeof within>, label: string) =>
      view.getByText(label).closest("dt")?.nextElementSibling;

    it("shows the tombstone for the promisor only, on a commitment that is not redacted", async () => {
      openWork("#COM-001", {}, {}, { promisor: { ...BAKERY, name: "", redacted: true } });
      const view = await region();
      expect(rowOf(view, "Zugesagt von")).toHaveTextContent("entfernt");
      expect(rowOf(view, "Bedingung")).toHaveTextContent("wenn der Auftrag unterschrieben ist");
    });

    it("shows the name of the promisor on a redacted commitment whose promisor is not redacted", async () => {
      openWork("#COM-001", {}, {}, { text: "", condition: null, redacted: true });
      const view = await region();
      expect(rowOf(view, "Zugesagt von")).toHaveTextContent("Bäckerei Zopf AG");
      expect(rowOf(view, "Bedingung")).toHaveTextContent("entfernt");
    });
  });

  describe("permissions", () => {
    it("shows plain text to a member who may not change the commitment", async () => {
      openWork("#COM-001", {}, {}, { can_change: false, can_make_firm: false, next_statuses: [] });
      const view = await region();
      expect(view.getByText("Bernd Beispiel")).toBeVisible();
      expect(
        view.queryByRole("button", { name: /^(Zusage|Status|Verantwortlich|Fällig|Bereich):/ }),
      ).toBeNull();
      expect(view.queryByRole("button", { name: "Aktionen" })).toBeNull();
    });

    it("has no control in a closed event, not even Aktionen", async () => {
      openWork("#COM-001", { event: { ...EVENT, status: "closed" } });
      const view = await region();
      expect(view.getByText("Bäckerei liefert 120 Zöpfe")).toBeVisible();
      expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
      expect(view.queryByRole("button", { name: "Aktionen" })).toBeNull();
    });
  });

  describe("editing", () => {
    it("renames the text inline, with the version of the list", async () => {
      const server = openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: "Zusage: Bäckerei liefert 120 Zöpfe" }));
      const input = view.getByRole("textbox", { name: "Zusage" });
      await user.clear(input);
      await user.type(input, "Bäckerei liefert 150 Zöpfe{Enter}");

      await view.findByRole("button", { name: "Zusage: Bäckerei liefert 150 Zöpfe" });
      expect(server.sent("PATCH")).toEqual([
        { text: "Bäckerei liefert 150 Zöpfe", expected_version: 1 },
      ]);
      expect(screen.getByText("Zusage von COM-001 gespeichert.")).toBeInTheDocument();
    });

    it("needs a text, with the message of the field", async () => {
      const server = openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Zusage:/ }));
      await user.clear(view.getByRole("textbox", { name: "Zusage" }));
      await user.keyboard("{Enter}");

      expect(await view.findByText("Der Text hat 1 bis 500 Zeichen.")).toBeVisible();
      expect(server.sent("PATCH")).toEqual([]);
    });

    it("puts a refusal of the server at the text and keeps the draft", async () => {
      openWork(
        "#COM-001",
        {},
        {
          patch: () => problem(422, "validation-failed", [{ pointer: "/text", code: "too-long" }]),
        },
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Zusage:/ }));
      await user.type(view.getByRole("textbox", { name: "Zusage" }), " und mehr{Enter}");

      expect(await view.findByText("Der Text hat 1 bis 500 Zeichen.")).toBeVisible();
      expect(view.getByRole("textbox", { name: "Zusage" })).toHaveValue(
        "Bäckerei liefert 120 Zöpfe und mehr",
      );
    });

    it("offers the status and the next ones, never firm, and saves the choice", async () => {
      const server = openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: bedingt" }));
      const list = await screen.findByRole("listbox", { name: "Status" });
      expect(
        within(list)
          .getAllByRole("option")
          .map((o) => o.textContent),
      ).toEqual(["bedingt", "erfüllt", "gebrochen", "zurückgezogen"]);
      await user.click(within(list).getByRole("option", { name: "erfüllt" }));

      await view.findByRole("button", { name: "Status: erfüllt" });
      expect(server.sent("PATCH")).toEqual([{ status: "fulfilled", expected_version: 1 }]);
    });

    it("does not list firm even when the server did, so that only make firm reaches it", async () => {
      openWork("#COM-001", {}, {}, { next_statuses: ["firm", "fulfilled"] });
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: bedingt" }));
      const list = await screen.findByRole("listbox", { name: "Status" });
      expect(within(list).queryByRole("option", { name: "verbindlich" })).toBeNull();
    });

    it("shows a firm commitment as firm and offers the closing statuses", async () => {
      openWork("#COM-001", {}, {}, { status: "firm", can_make_firm: false });
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: verbindlich" }));
      const list = await screen.findByRole("listbox", { name: "Status" });
      expect(
        within(list)
          .getAllByRole("option")
          .map((o) => o.textContent),
      ).toEqual(["verbindlich", "erfüllt", "gebrochen", "zurückgezogen"]);
    });

    it("offers no status for a closed commitment", async () => {
      openWork(
        "#COM-001",
        {},
        {},
        { status: "fulfilled", next_statuses: [], can_make_firm: false },
      );
      const view = await region();
      expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
      expect(view.getByText("erfüllt")).toBeVisible();
    });

    it("changes the owner, the due date and the area", async () => {
      const server = openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
      await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
      await view.findByRole("button", { name: "Verantwortlich: Anna Muster" });

      await user.click(view.getByRole("button", { name: /^Fällig:/ }));
      const date = view.getByLabelText("Fällig");
      await user.clear(date);
      await user.type(date, "2030-07-01{Enter}");
      await view.findByRole("button", { name: /^Fällig:.*01\.07\.2030/ });

      await user.click(view.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
      await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
      await view.findByRole("button", { name: "Bereich: Festwirtschaft" });

      expect(server.sent("PATCH")).toEqual([
        { owner_user_id: ME, expected_version: 1 },
        { due_date: "2030-07-01", expected_version: 2 },
        { workstream_id: "w2", expected_version: 3 },
      ]);
    });

    it("clears the due date and the area", async () => {
      const server = openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Fällig:/ }));
      await user.clear(view.getByLabelText("Fällig"));
      await user.keyboard("{Enter}");
      await view.findByRole("button", { name: "Fällig: Kein Datum" });
      await user.click(view.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
      await user.click(await screen.findByRole("button", { name: "Kein Bereich" }));
      await view.findByRole("button", { name: "Bereich: Kein Bereich" });
      expect(server.sent("PATCH")).toEqual([
        { due_date: null, expected_version: 1 },
        { workstream_id: null, expected_version: 2 },
      ]);
    });

    it("marks an overdue due date", async () => {
      openWork("#COM-001", {}, {}, { due_date: "2020-02-01" });
      const view = await region();
      expect(view.getByText(/überfällig/)).toBeInTheDocument();
    });

    describe("a conflict", () => {
      it("shows the saved text beside the draft, loads the commitment again and saves on purpose", async () => {
        let calls = 0;
        const server = openWork(
          "#COM-001",
          {},
          {
            patch: (body, current) => {
              if (++calls === 1) {
                server.work["/commitments"] = [{ ...current, text: "Text von Bernd", version: 2 }];
                return problem(409, "record-version-conflict");
              }
              return json(200, { ...current, ...body, version: 3 });
            },
          },
        );
        const view = await region();
        await user.click(view.getByRole("button", { name: /^Zusage:/ }));
        await user.clear(view.getByRole("textbox", { name: "Zusage" }));
        await user.type(view.getByRole("textbox", { name: "Zusage" }), "Mein Text{Enter}");

        await view.findByText("Jetzt gespeichert: Text von Bernd");
        expect(view.getByRole("textbox", { name: "Zusage" })).toHaveValue("Mein Text");
        // A click elsewhere does not save over the other change.
        await user.click(screen.getByRole("heading", { level: 1 }));
        expect(server.sent("PATCH")).toHaveLength(1);

        await user.click(view.getByRole("button", { name: "Speichern" }));
        await view.findByRole("button", { name: "Zusage: Mein Text" });
        expect(server.sent("PATCH")[1]).toEqual({ text: "Mein Text", expected_version: 2 });
      });

      it("keeps a refused owner and says that the commitment changed", async () => {
        openWork("#COM-001", {}, { patch: () => problem(409, "record-version-conflict") });
        const view = await region();
        await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
        await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
        const group = await view.findByRole("group", { name: "Verantwortlich" });
        expect(group).toHaveAccessibleDescription(/inzwischen geändert/);
      });
    });
  });

  describe("Verbindlich machen", () => {
    const openStep = async () => {
      const view = await region();
      await user.click(view.getByRole("button", { name: "Aktionen" }));
      await user.click(await screen.findByRole("menuitem", { name: "Verbindlich machen" }));
      return within(await screen.findByRole("dialog", { name: "COM-001 verbindlich machen" }));
    };

    it("is offered in the Aktionen menu of a conditional commitment that the member may firm", async () => {
      openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: "Aktionen" }));
      const menu = await screen.findByRole("menu", { name: "Aktionen" });
      expect(
        within(menu)
          .getAllByRole("menuitem")
          .map((i) => i.textContent),
      ).toEqual(["Verbindlich machen"]);
    });

    it("is not a choice of the status", async () => {
      openWork("#COM-001");
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: bedingt" }));
      expect(screen.queryByRole("option", { name: /verbindlich/ })).toBeNull();
    });

    it.each([
      ["a firm commitment", { status: "firm", can_make_firm: false }],
      ["a commitment that the member may not firm", { can_make_firm: false }],
      ["a fulfilled commitment", { status: "fulfilled", can_make_firm: false, next_statuses: [] }],
    ])("is absent for %s", async (_name, over) => {
      openWork("#COM-001", {}, {}, over);
      const view = await region();
      expect(view.queryByRole("button", { name: "Aktionen" })).toBeNull();
    });

    it("asks for the reason, shows the condition and keeps focus in the reason", async () => {
      openWork("#COM-001");
      const step = await openStep();
      expect(step.getByText(/Die Bedingung bleibt als Verlauf stehen/)).toBeVisible();
      expect(step.getByText(/Bedingung: wenn der Auftrag unterschrieben ist/)).toBeVisible();
      expect(step.getByRole("textbox", { name: "Grund (Pflichtfeld)" })).toHaveFocus();
    });

    it("needs a reason and sends nothing without one", async () => {
      const server = openWork("#COM-001");
      const step = await openStep();
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));

      expect(
        await step.findByText("Geben Sie einen Grund an. Er hat 1 bis 500 Zeichen."),
      ).toBeVisible();
      expect(server.sent("POST")).toEqual([]);
    });

    it("makes the commitment firm with the reason, and the panel shows the new state", async () => {
      const server = openWork("#COM-001");
      const step = await openStep();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Auftrag ist unterschrieben");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));

      const view = await region();
      await view.findByRole("button", { name: "Status: verbindlich" });
      expect(server.sent("POST")).toEqual([
        { reason: "Auftrag ist unterschrieben", expected_version: 1 },
      ]);
      expect(screen.queryByRole("dialog")).toBeNull();
      // The action is gone with the state, and the member is told.
      expect(view.queryByRole("button", { name: "Aktionen" })).toBeNull();
      expect(screen.getByText("COM-001 ist jetzt verbindlich.")).toBeInTheDocument();
      // The condition stays as history.
      expect(view.getByText("wenn der Auftrag unterschrieben ist")).toBeVisible();
    });

    it("shows why the commitment is firm, only once it is", async () => {
      openWork("#COM-001");
      const step = await openStep();
      expect(screen.queryByText("Verbindlich, weil")).toBeNull();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Auftrag ist unterschrieben");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));

      const view = await region();
      expect(await view.findByText("Verbindlich, weil")).toBeVisible();
      expect(view.getByText("Auftrag ist unterschrieben")).toBeVisible();
    });

    it("shows no reason for a commitment that started firm", async () => {
      openWork("#COM-001", {}, {}, { status: "firm", can_make_firm: false, condition: null });
      const view = await region();
      expect(view.queryByText("Verbindlich, weil")).toBeNull();
    });

    it("moves focus into the panel when the menu leaves with the action", async () => {
      openWork("#COM-001");
      const step = await openStep();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Auftrag ist unterschrieben");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));
      await waitFor(() =>
        expect(screen.getByRole("complementary", { name: /COM-001/ })).toHaveFocus(),
      );
    });

    it("puts the refusal of the reason at the reason", async () => {
      openWork(
        "#COM-001",
        {},
        {
          firm: () =>
            problem(422, "validation-failed", [{ pointer: "/reason", code: "control-character" }]),
        },
      );
      const step = await openStep();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Grund");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));
      expect(
        await step.findByText("Der Grund enthält ein Zeichen, das nicht erlaubt ist."),
      ).toBeVisible();
      expect(step.getByRole("textbox", { name: /^Grund/ })).toHaveValue("Grund");
    });

    it("says that the commitment changed after a conflict, loads it again and keeps the step", async () => {
      let calls = 0;
      const server = openWork(
        "#COM-001",
        {},
        {
          firm: (body, current) => {
            if (++calls === 1) {
              server.work["/commitments"] = [{ ...current, due_date: "2030-09-09", version: 2 }];
              return problem(409, "record-version-conflict");
            }
            return json(200, {
              ...current,
              ...body,
              status: "firm",
              version: 3,
              can_make_firm: false,
            });
          },
        },
      );
      const step = await openStep();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Bestellt");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));

      expect(await step.findByRole("alert")).toHaveTextContent(/inzwischen geändert/);
      expect(step.getByRole("textbox", { name: /^Grund/ })).toHaveValue("Bestellt");
      // The list was loaded again, so the next try has the new version.
      await waitFor(() =>
        expect(
          server.calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/commitments`)
            .length,
        ).toBeGreaterThan(1),
      );
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));
      await (await region()).findByRole("button", { name: "Status: verbindlich" });
      expect(server.sent("POST")[1]).toMatchObject({ expected_version: 2 });
    });

    it("shows another failure in the step and keeps it open", async () => {
      openWork("#COM-001", {}, { firm: () => problem(500, "internal") });
      const step = await openStep();
      await user.type(step.getByRole("textbox", { name: /^Grund/ }), "Bestellt");
      await user.click(step.getByRole("button", { name: "Verbindlich machen" }));
      expect(await step.findByRole("alert")).toBeVisible();
      expect(screen.getByRole("dialog")).toBeVisible();
    });

    it("closes the step with Escape and leaves the panel open; focus returns to the menu button", async () => {
      openWork("#COM-001");
      await openStep();
      await user.keyboard("{Escape}");

      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(screen.getByRole("complementary", { name: /COM-001/ })).toBeVisible();
      expect(window.location.hash).toBe("#COM-001");
      await waitFor(() => expect(screen.getByRole("button", { name: "Aktionen" })).toHaveFocus());
    });

    it("closes the step with Abbrechen and changes nothing", async () => {
      const server = openWork("#COM-001");
      const step = await openStep();
      await user.click(step.getByRole("button", { name: "Abbrechen" }));
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(server.sent("POST")).toEqual([]);
    });
  });
});

describe("the create state of a commitment", () => {
  it("shows the fields, with the signed-in member as the owner", async () => {
    openWork("?new=commitment");
    const view = await form();
    expect(view.getByRole("textbox", { name: "Zusage (Pflichtfeld)" })).toBeVisible();
    expect(view.getByRole("combobox", { name: "Zugesagt von (Pflichtfeld)" })).toBeVisible();
    expect(view.getByRole("textbox", { name: "Bedingung" })).toBeVisible();
    expect(
      view.getByText(
        "Mit einer Bedingung ist die Zusage bedingt. Ohne Bedingung ist sie verbindlich.",
      ),
    ).toBeVisible();
    expect(
      view.getByRole("button", { name: "Verantwortlich (Pflichtfeld): Anna Muster" }),
    ).toBeVisible();
    expect(view.getByLabelText("Fällig")).toHaveAttribute("type", "date");
    expect(view.getByRole("button", { name: "Bereich: Kein Bereich" })).toBeVisible();
    expect(view.getByRole("button", { name: "Erstellen" })).toBeVisible();
    expect(view.queryByRole("textbox", { name: /Titel|Beschreibung/ })).toBeNull();
  });

  it("asks for the text and the promisor and focuses the first invalid field", async () => {
    const server = openWork("?new=commitment");
    const view = await form();
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const text = await view.findByRole("textbox", { name: /^Zusage/ });
    expect(text).toBeInvalid();
    expect(text).toHaveFocus();
    expect(view.getByText("Der Text hat 1 bis 500 Zeichen.")).toBeVisible();
    expect(view.getByText("Wählen Sie eine Person oder eine Institution.")).toBeVisible();
    expect(server.sent("POST")).toEqual([]);
  });

  it("asks for the promisor when only the text is there", async () => {
    const server = openWork("?new=commitment");
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Zusage/ }), "Lieferung");
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    const box = await view.findByRole("combobox", { name: /^Zugesagt von/ });
    await waitFor(() => expect(box).toBeInvalid());
    expect(box).toHaveFocus();
    expect(server.sent("POST")).toEqual([]);
  });

  async function fill(view: ReturnType<typeof within>, condition = "") {
    await user.type(view.getByRole("textbox", { name: /^Zusage/ }), "Bäckerei liefert Zöpfe");
    await user.type(view.getByRole("combobox", { name: /^Zugesagt von/ }), "Bäcker");
    await user.click(
      await screen.findByRole("option", { name: "Bäckerei Zopf AG (INS-001)" }, SEARCH),
    );
    if (condition) {
      await user.type(view.getByRole("textbox", { name: "Bedingung" }), condition);
    }
  }

  it("creates the commitment, switches the panel to it and focuses its text", async () => {
    const server = openWork("?view=table&new=commitment");
    const view = await form();
    await fill(view, "bei Bestellung");
    await user.type(view.getByLabelText("Fällig"), "2030-06-01");
    await user.click(view.getByRole("button", { name: "Bereich: Kein Bereich" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const open = await screen.findByRole("complementary", { name: /COM-020/ });
    expect(server.sent("POST")).toEqual([
      {
        id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
        text: "Bäckerei liefert Zöpfe",
        promisor: { kind: "institution", id: "i1" },
        owner_user_id: ME,
        condition: "bei Bestellung",
        due_date: "2030-06-01",
        workstream_id: "w2",
      },
    ]);
    expect(window.location.search).toBe("?view=table");
    expect(window.location.hash).toBe("#COM-020");
    await waitFor(() =>
      expect(within(open).getByRole("button", { name: /^Zusage:/ })).toHaveFocus(),
    );
    expect(screen.getByText("COM-020 erfasst.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Bäckerei liefert Zöpfe" })).toBeInTheDocument();
  });

  it("sends no condition when the field is empty", async () => {
    const server = openWork("?new=commitment");
    const view = await form();
    await fill(view);
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    await screen.findByRole("complementary", { name: /COM-020/ });
    expect(server.sent("POST")[0]).not.toHaveProperty("condition");
    expect(server.sent("POST")[0]).not.toHaveProperty("due_date");
    expect(server.sent("POST")[0]).not.toHaveProperty("workstream_id");
  });

  it("starts without an owner when the member cannot own a record, and asks for one", async () => {
    const server = openWork("?new=commitment", { role: "admin", viewer: true });
    const view = await form();
    expect(
      view.getByRole("button", { name: "Verantwortlich (Pflichtfeld): Person wählen" }),
    ).toBeVisible();
    await fill(view);
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    expect(await view.findByText(/Wählen Sie ein Mitglied des Anlasses/)).toBeVisible();
    expect(server.sent("POST")).toEqual([]);
  });

  it("puts the refusals of the server at their fields and keeps the draft", async () => {
    openWork(
      "?new=commitment",
      {},
      {
        post: () =>
          problem(422, "validation-failed", [
            { pointer: "/promisor", code: "unknown-record" },
            { pointer: "/condition", code: "too-long" },
          ]),
      },
    );
    const view = await form();
    await fill(view, "bei Bestellung");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByText("Diese Person oder Institution gibt es nicht.")).toBeVisible();
    expect(screen.getByText("Die Bedingung hat 1 bis 500 Zeichen.")).toBeVisible();
    // The invalid party has focus and its list is open, which hides the rest from the tree.
    const hidden = { hidden: true };
    expect(view.getByRole("textbox", { name: /^Zusage/, ...hidden })).toHaveValue(
      "Bäckerei liefert Zöpfe",
    );
    expect(view.getByRole("textbox", { name: "Bedingung", ...hidden })).toHaveValue(
      "bei Bestellung",
    );
  });

  it("sends one ID for the record, also on a retry after another failure", async () => {
    let calls = 0;
    const server = openWork(
      "?new=commitment",
      {},
      {
        post: (body) =>
          ++calls === 1
            ? problem(500, "internal")
            : json(
                201,
                commitment({ id: "rec-com-020", local_id: "COM-020", ...body, promisor: BAKERY }),
              ),
      },
    );
    const view = await form();
    await fill(view);
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    expect(await view.findByRole("alert")).toBeVisible();
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await screen.findByRole("complementary", { name: /COM-020/ });
    const ids = server.sent("POST").map((body) => body.id);
    expect(ids[1]).toBe(ids[0]);
  });

  it("shows the commitment that a lost response created, when the retry finds the ID taken", async () => {
    let calls = 0;
    const server = openWork(
      "?new=commitment",
      {},
      {
        post: (body) => {
          if (++calls === 1) {
            server.work["/commitments"]?.push(
              commitment({ id: body.id, local_id: "COM-020", ...body, promisor: BAKERY }),
            );
            return problem(500, "internal");
          }
          return problem(422, "validation-failed", [{ pointer: "/id", code: "taken" }]);
        },
      },
    );
    const view = await form();
    await fill(view);
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    await view.findByRole("alert");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await screen.findByRole("complementary", { name: /COM-020/ });
    expect(window.location.hash).toBe("#COM-020");
    expect(server.sent("POST")).toHaveLength(2);
  });

  it("sends one request for a double submit in the same moment", async () => {
    let release: () => void = () => undefined;
    const server = openWork(
      "?new=commitment",
      {},
      {
        post: (body) =>
          new Promise<Response>((resolve) => {
            release = () =>
              resolve(
                json(
                  201,
                  commitment({ id: "rec-com-020", local_id: "COM-020", ...body, promisor: BAKERY }),
                ),
              );
          }),
      },
    );
    const view = await form();
    await fill(view);
    const submit = view.getByRole("button", { name: "Erstellen" });
    act(() => {
      submit.click();
      submit.click();
    });
    await waitFor(() => expect(server.sent("POST")).toHaveLength(1));
    release();
    await screen.findByRole("complementary", { name: /COM-020/ });
    expect(server.sent("POST")).toHaveLength(1);
  });

  it("explains to a member who may not create that the form is not for them", async () => {
    openWork("?new=commitment", { event: { ...EVENT, status: "closed" } });
    const view = await form();
    expect(view.getByText(/nichts erfassen/)).toBeVisible();
    expect(view.queryByRole("button", { name: "Erstellen" })).toBeNull();
  });

  it("closes with Abbrechen and keeps the query", async () => {
    openWork("?view=table&new=commitment");
    const view = await form();
    await user.click(view.getByRole("button", { name: "Abbrechen" }));
    await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
    expect(window.location.search).toBe("?view=table");
  });
});
