import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { BERND, EVENT, json, ME, problem, renderWork, type Setup } from "../test/fakeWorkServer";
import * as r from "../test/workRecords";
import { WorkPage } from "./WorkPage";

const STREAMS = [
  { id: "w1", event_id: EVENT.id, name: "Bodenbetrieb", status: "active", version: 1 },
  { id: "w2", event_id: EVENT.id, name: "Festwirtschaft", status: "active", version: 1 },
];
const user = userEvent.setup({ delay: null });
const SEARCH = { timeout: 3000 };

const clara = {
  kind: "person",
  id: "p1",
  local_id: "PER-001",
  name: "Cäcilia Probst",
  redacted: false,
};
const generators = {
  kind: "institution",
  id: "i1",
  local_id: "INS-001",
  name: "Testwil Generatoren AG",
  redacted: false,
};

// The panel is a lazy chunk with many modules: load it once before the first test, which would
// otherwise wait for it past the timeout of a find.
beforeAll(() => import("./record/RecordPanel"), 30_000);

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

type Body = Record<string, unknown>;

const WORDING = "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK am Vortag.";

/** A decision in force that a manager may end. */
const dec = (over: Body = {}) =>
  r.decision("DEC-001", {
    wording: WORDING,
    decided_by: [clara, generators],
    owner_user_id: BERND,
    workstream_id: "w1",
    can_end: true,
    ...over,
  });

function openDecision(
  over: Body = {},
  setup: Setup = {},
  answers: Setup["answers"] = {},
  hash = "DEC-001",
) {
  const work: Record<string, unknown[]> = {
    ...r.sampleWork(),
    "/persons": [{ id: "p1", local_id: "PER-001", name: "Cäcilia Probst", version: 1 }],
    "/institutions": [
      { id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG", version: 1 },
    ],
  };
  work["/decisions"] = [dec(over)];
  const result = renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work#${hash}`,
      eventManager: true,
      lists: work,
      workstreams: STREAMS,
      answers,
      ...setup,
    },
  );
  return {
    ...result,
    work,
    sent: (method: string) =>
      result.calls.filter((c) => c.call.startsWith(method)).map((c) => c.body as Body),
  };
}

async function region(id = "DEC-001") {
  return within(
    await screen.findByRole("complementary", { name: new RegExp(id) }, { timeout: 3000 }),
  );
}

describe("a decision in the panel", () => {
  it("shows the wording, the day, the parties, the owner and the area", async () => {
    openDecision();
    const view = await region();
    expect(view.getByText("DEC-001")).toBeVisible();
    expect(view.getByText("Entscheid")).toBeVisible();
    const labels = Array.from(document.querySelectorAll("aside dt")).map((dt) => dt.textContent);
    expect(labels).toEqual(["Status", "Entschieden", "Beteiligte", "Verantwortlich", "Bereich"]);
    expect(view.getByText("in Kraft")).toBeVisible();
    expect(view.getByText("12.03.2030")).toBeVisible();
    expect(view.getByText("Cäcilia Probst")).toBeVisible();
    expect(view.getByText("Testwil Generatoren AG")).toBeVisible();
    expect(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeVisible();
  });

  it("names the region by the first line of a long wording, and shows all of it as the heading", async () => {
    openDecision({ wording: `${"Sehr lang ".repeat(30)}\nZweite Zeile` });
    const view = await region();
    const heading = view.getByRole("heading", { level: 2 });
    expect(heading).toHaveTextContent("Zweite Zeile");
    const name = screen.getByRole("complementary").getAttribute("aria-label") ?? "";
    expect(name.length).toBeLessThan(100);
    expect(name).not.toContain("Zweite Zeile");
  });

  it("never edits the wording, the day or the parties, also for a manager", async () => {
    openDecision();
    const view = await region();
    expect(view.queryByRole("button", { name: /^Wortlaut/ })).toBeNull();
    expect(view.queryByRole("button", { name: /^Titel/ })).toBeNull();
    expect(view.queryByRole("button", { name: /^Entschieden/ })).toBeNull();
    expect(view.queryByRole("button", { name: /^Beteiligte/ })).toBeNull();
    await user.click(view.getByRole("heading", { level: 2 }));
    expect(view.queryByRole("textbox")).toBeNull();
  });

  it("lets a manager change the owner and the area", async () => {
    const server = openDecision(
      {},
      {},
      {
        "PATCH /decisions/rec-dec-001": (_url, body) =>
          json(200, { ...dec(), ...(body as Body), version: 2 }),
      },
    );
    const view = await region();
    await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
    await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
    await view.findByRole("button", { name: "Verantwortlich: Anna Muster" });
    await user.click(view.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await view.findByRole("button", { name: "Bereich: Festwirtschaft" });
    expect(server.sent("PATCH")).toEqual([
      { owner_user_id: ME, expected_version: 1 },
      { workstream_id: "w2", expected_version: 2 },
    ]);
  });

  it("shows plain values and no action to a member who cannot change or end it", async () => {
    openDecision({ can_change: false, can_end: false }, { eventManager: false });
    const view = await region();
    expect(view.getByText("Bernd Beispiel")).toBeVisible();
    expect(view.queryByRole("button", { name: /^Verantwortlich/ })).toBeNull();
    expect(view.queryByRole("button", { name: "Ersetzen" })).toBeNull();
    expect(view.queryByRole("button", { name: "Widerrufen" })).toBeNull();
  });

  it("offers Ersetzen and Widerrufen for a decision in force that the member may end", async () => {
    openDecision();
    const view = await region();
    expect(view.getByRole("button", { name: "Ersetzen" })).toBeVisible();
    expect(view.getByRole("button", { name: "Widerrufen" })).toBeVisible();
  });

  it.each([
    ["a closed event", {}, { event: { ...EVENT, status: "closed" } }],
    ["a decision that was replaced", { status: "superseded", superseded_by: "rec-dec-002" }, {}],
    ["a decision that was revoked", { status: "revoked", revoked_reason: "Überholt" }, {}],
    ["a redacted decision", { redacted: true, wording: "" }, {}],
  ] as const)("offers no action in %s", async (_name, over, setup) => {
    openDecision(over, setup);
    const view = await region();
    expect(view.queryByRole("button", { name: "Ersetzen" })).toBeNull();
    expect(view.queryByRole("button", { name: "Widerrufen" })).toBeNull();
  });

  it("shows the tombstone for a redacted decision", async () => {
    openDecision({
      redacted: true,
      wording: "",
      decided_by: [{ ...clara, name: "", redacted: true }],
    });
    const view = await region("DEC-001");
    expect(view.getAllByText("entfernt").length).toBeGreaterThan(0);
    // A redacted party is the tombstone and its ID, with no avatar of a made-up initial.
    const party = within(view.getByRole("list")).getByText("PER-001").closest("li");
    expect(party).toHaveTextContent(/^entfernt\s*PER-001$/);
  });

  describe("a decision that was replaced", () => {
    function replaced() {
      const server = openDecision({ status: "superseded", superseded_by: "rec-dec-002" });
      server.work["/decisions"]?.push(
        r.decision("DEC-002", {
          wording: "Das Fest findet nur bei Sonne statt.",
          decided_by: [clara],
        }),
      );
      return server;
    }

    it("links to the successor and opens it in the panel", async () => {
      replaced();
      const view = await region();
      expect(view.getByText("ersetzt")).toBeVisible();
      const link = await view.findByRole("link", { name: "DEC-002" });
      expect(link).toHaveAttribute("href", "#DEC-002");
      await user.click(link);
      const successor = await region("DEC-002");
      expect(window.location.hash).toBe("#DEC-002");
      // The link left with the old record: focus is on the title of the new one.
      await waitFor(() => expect(successor.getByRole("heading", { level: 2 })).toHaveFocus());
    });

    it("lets Back return to the replaced decision, with focus on its title", async () => {
      replaced();
      const view = await region();
      await user.click(await view.findByRole("link", { name: "DEC-002" }));
      await region("DEC-002");
      window.history.back();
      const back = await region("DEC-001");
      expect(window.location.hash).toBe("#DEC-001");
      await waitFor(() => expect(back.getByRole("heading", { level: 2 })).toHaveFocus());
    });
  });

  it("shows the reason of a revoked decision", async () => {
    openDecision({ status: "revoked", revoked_reason: "Der Anlass fällt aus." });
    const view = await region();
    expect(view.getByText("widerrufen")).toBeVisible();
    expect(view.getByText("Grund des Widerrufs")).toBeVisible();
    expect(view.getByText("Der Anlass fällt aus.")).toBeVisible();
    expect(view.queryByRole("link")).toBeNull();
  });

  it("lists the sources", async () => {
    openDecision({
      evidence: [
        {
          proposal_id: "p1",
          record_version: 1,
          source_version_id: "s1",
          captured_at: "2030-03-01T10:00:00Z",
          start_offset: 0,
          end_offset: 5,
          quote: "Bei jedem Wetter",
          redacted: false,
        },
      ],
    });
    const view = await region();
    expect(view.getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
    expect(view.getByText("Bei jedem Wetter")).toBeVisible();
  });
});

describe("revoking a decision", () => {
  const revoked = (reason: string) =>
    dec({ status: "revoked", revoked_reason: reason, version: 2 });

  async function openDialog(answers: Setup["answers"] = {}) {
    const server = openDecision({}, {}, answers);
    const view = await region();
    await user.click(view.getByRole("button", { name: "Widerrufen" }));
    const dialog = within(await screen.findByRole("dialog", { name: "DEC-001 widerrufen" }));
    return { server, view, dialog };
  }

  it("asks for a reason in a dialog with the reason in focus", async () => {
    const { dialog } = await openDialog();
    expect(dialog.getByRole("textbox", { name: /^Grund/ })).toHaveFocus();
    expect(dialog.getByText(/Der Entscheid gilt danach als widerrufen/)).toBeVisible();
  });

  it("revokes with the reason and the version, and shows the decision as revoked", async () => {
    const { server, view, dialog } = await openDialog({
      "POST /decisions/rec-dec-001/revoke": (_url, body) => {
        const reason = (body as Body).reason as string;
        return json(200, revoked(reason));
      },
    });
    await user.type(dialog.getByRole("textbox", { name: /^Grund/ }), "Der Anlass fällt aus.");
    await user.click(dialog.getByRole("button", { name: "Widerrufen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(server.sent("POST")).toEqual([{ reason: "Der Anlass fällt aus.", expected_version: 1 }]);
    expect(await view.findByText("Der Anlass fällt aus.")).toBeVisible();
    expect(view.getByText("widerrufen")).toBeVisible();
    expect(view.queryByRole("button", { name: "Widerrufen" })).toBeNull();
    expect(screen.getByText("DEC-001 ist jetzt widerrufen.")).toBeInTheDocument();
    // The buttons are gone: focus stays in the panel.
    expect(screen.getByRole("complementary").contains(document.activeElement)).toBe(true);
  });

  it("needs a reason", async () => {
    const { server, dialog } = await openDialog();
    await user.click(dialog.getByRole("button", { name: "Widerrufen" }));
    expect(await dialog.findByText(/Geben Sie einen Grund an/)).toBeVisible();
    expect(server.sent("POST")).toEqual([]);
  });

  it("shows the refusal of the reason of the server at the field", async () => {
    const { dialog } = await openDialog({
      "POST /decisions/rec-dec-001/revoke": () =>
        problem(422, "validation-failed", [{ pointer: "/reason", code: "too-long" }]),
    });
    await user.type(dialog.getByRole("textbox", { name: /^Grund/ }), "Text");
    await user.click(dialog.getByRole("button", { name: "Widerrufen" }));
    expect(await dialog.findByText(/Geben Sie einen Grund an/)).toBeVisible();
    expect(dialog.getByRole("textbox", { name: /^Grund/ })).toHaveValue("Text");
  });

  it("tells a conflict in the dialog and loads the decisions again", async () => {
    const { server, dialog } = await openDialog({
      "POST /decisions/rec-dec-001/revoke": () => problem(409, "record-version-conflict"),
    });
    await user.type(dialog.getByRole("textbox", { name: /^Grund/ }), "Text");
    await user.click(dialog.getByRole("button", { name: "Widerrufen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/geändert|neu geladen/);
    expect(screen.getByRole("dialog")).toBeVisible();
    await waitFor(() =>
      expect(
        server.calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/decisions`).length,
      ).toBeGreaterThan(1),
    );
  });

  it("cancels with Escape and returns focus to the button", async () => {
    const { view } = await openDialog();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("complementary")).toBeInTheDocument();
    expect(view.getByRole("button", { name: "Widerrufen" })).toHaveFocus();
  });
});

describe("replacing a decision", () => {
  function supersedeAnswer(server: { work: Record<string, unknown[]> }) {
    return (_url: URL, body?: unknown) => {
      const { successor } = body as { successor: Body };
      const next = r.decision("DEC-002", {
        ...successor,
        id: "rec-dec-002",
        decided_by: [clara],
        can_end: true,
      });
      server.work["/decisions"] = [
        dec({ status: "superseded", superseded_by: "rec-dec-002", version: 2 }),
        next,
      ];
      return json(201, next);
    };
  }

  async function openDialog(answers: Setup["answers"] = {}) {
    const holder: { work: Record<string, unknown[]> } = { work: {} };
    const server = openDecision(
      {},
      {},
      {
        "POST /decisions/rec-dec-001/supersede": (url, body) => supersedeAnswer(holder)(url, body),
        ...answers,
      },
    );
    holder.work = server.work;
    // The fake server reads its lists from this object.
    const view = await region();
    await user.click(view.getByRole("button", { name: "Ersetzen" }));
    const dialog = within(await screen.findByRole("dialog", { name: "DEC-001 ersetzen" }));
    return { server, view, dialog };
  }

  it("starts with the wording and the parties of the decision, and a new day", async () => {
    const { dialog } = await openDialog();
    expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toHaveValue(WORDING);
    expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toHaveFocus();
    expect(dialog.getByLabelText(/^Entschieden am/)).toHaveValue("");
    const chosen = dialog.getByRole("list", { name: /^Beteiligte.*: gewählt$/ });
    expect(within(chosen).getByText("Cäcilia Probst (PER-001)")).toBeVisible();
    expect(within(chosen).getByText("Testwil Generatoren AG (INS-001)")).toBeVisible();
    expect(dialog.getByText(/Der neue Entscheid ersetzt DEC-001/)).toBeVisible();
    // The words say what the new decision takes over; the old one has no reason to give.
    expect(dialog.getByText(/Verantwortlich und Bereich sind die des bisherigen/)).toBeVisible();
    expect(dialog.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
    expect(dialog.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeVisible();
    expect(dialog.queryByRole("textbox", { name: /^Grund/ })).toBeNull();
  });

  it("needs a day, a wording and at least one party, and focuses the first error", async () => {
    const { server, dialog } = await openDialog();
    await user.clear(dialog.getByRole("textbox", { name: /^Wortlaut/ }));
    for (const remove of dialog.getAllByRole("button", { name: /entfernen$/ })) {
      await user.click(remove);
    }
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));

    expect(await dialog.findAllByText("Der Wortlaut hat 1 bis 2000 Zeichen.")).not.toHaveLength(0);
    expect(dialog.getByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(dialog.getByText("Wählen Sie 1 bis 50 Personen oder Institutionen.")).toBeVisible();
    await waitFor(() => expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toHaveFocus());
    expect(server.sent("POST")).toEqual([]);
  });

  it("supersedes with a new decision that has a client ID, the owner and the area of the old one", async () => {
    const { server, view, dialog } = await openDialog();
    await user.clear(dialog.getByRole("textbox", { name: /^Wortlaut/ }));
    await user.type(dialog.getByRole("textbox", { name: /^Wortlaut/ }), "Nur bei Sonne.");
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    // A search finds a person that the old decision did not name; both are chosen.
    await dialog.findByRole("option", { name: "Cäcilia Probst (PER-001)" }, SEARCH);
    await user.click(
      within(dialog.getByRole("list", { name: /gewählt$/ })).getAllByRole(
        "button",
      )[1] as HTMLElement,
    );
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(server.sent("POST")).toEqual([
      {
        successor: {
          id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
          wording: "Nur bei Sonne.",
          decided_on: "2030-04-01",
          decided_by: [{ kind: "person", id: "p1" }],
          owner_user_id: BERND,
          workstream_id: "w1",
        },
        expected_version: 1,
      },
    ]);
    // The old decision is replaced and links to the new one.
    expect(await view.findByText("ersetzt")).toBeVisible();
    expect(await view.findByRole("link", { name: "DEC-002" })).toBeVisible();
    expect(screen.getByText("DEC-002 ersetzt DEC-001.")).toBeInTheDocument();
    expect(view.queryByRole("button", { name: "Ersetzen" })).toBeNull();
    // The buttons are gone, and focus is on the heading, not on the body.
    expect(view.getByRole("heading", { level: 2 })).toHaveFocus();
  });

  it("moves focus to the heading at once, while the old decision still has its buttons", async () => {
    const { dialog, view } = await openDialog();
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(view.getByRole("heading", { level: 2 })).toHaveFocus();
  });

  it("says that the old decision is not up to date if it cannot be loaded again, and offers no second try", async () => {
    const ref: { posted?: () => boolean } = {};
    const { view, dialog, server } = await openDialog({
      "GET /decisions": () =>
        ref.posted?.() ? problem(500, "internal") : json(200, { items: [dec()] }),
    });
    ref.posted = () => server.sent("POST").length > 0;
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const words = /DEC-002 ersetzt DEC-001.*nicht aktuell/;
    const message = (await view.findAllByText(words)).find((node) => node.tagName === "P");
    expect(message).toBeVisible();
    expect(await screen.findByRole("alert")).toHaveTextContent(/nicht aktuell/);
    expect(view.queryByRole("button", { name: "Ersetzen" })).toBeNull();
    expect(view.queryByRole("button", { name: "Widerrufen" })).toBeNull();
    expect(view.getByRole("heading", { level: 2 })).toHaveFocus();
  });

  it("shows the refusal of the server at the fields of the successor", async () => {
    const { dialog } = await openDialog({
      "POST /decisions/rec-dec-001/supersede": () =>
        problem(422, "validation-failed", [{ pointer: "/decided_on", code: "invalid" }]),
    });
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    expect(await dialog.findByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toHaveValue(WORDING);
  });

  it("lets a manager change the owner and the area of the successor, or clear the area", async () => {
    const { server, dialog } = await openDialog();
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
    await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
    await user.click(dialog.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(server.sent("POST")[0]).toMatchObject({
      successor: { owner_user_id: ME, workstream_id: "w2" },
    });
  });

  it("sends no area for a successor whose area was cleared", async () => {
    const { server, dialog } = await openDialog();
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
    await user.click(await screen.findByRole("button", { name: "Kein Bereich" }));
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(server.sent("POST")[0]?.successor).not.toHaveProperty("workstream_id");
  });

  it.each([
    ["/workstream", "closed", "Bereich: Bodenbetrieb", "Dieser Arbeitsbereich ist geschlossen."],
    [
      "/owner",
      "not-assignable",
      "Verantwortlich: Bernd Beispiel",
      "Wählen Sie ein Mitglied des Anlasses mit Mitarbeit oder Anlassleitung.",
    ],
  ])("shows the refusal of %s at its field and focuses it", async (pointer, code, button, text) => {
    const { dialog } = await openDialog({
      "POST /decisions/rec-dec-001/supersede": () =>
        problem(422, "validation-failed", [{ pointer, code }]),
    });
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    expect(await dialog.findByText(text)).toBeVisible();
    await waitFor(() => expect(dialog.getByRole("button", { name: button })).toHaveFocus());
    expect(dialog.getByRole("textbox", { name: /^Wortlaut/ })).toHaveValue(WORDING);
  });

  it("tells a conflict, keeps the dialog and reloads the decisions", async () => {
    const { server, dialog } = await openDialog({
      "POST /decisions/rec-dec-001/supersede": () => problem(409, "record-version-conflict"),
    });
    await user.type(dialog.getByLabelText(/^Entschieden am/), "2030-04-01");
    await user.click(dialog.getByRole("button", { name: "Ersetzen" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeVisible();
    await waitFor(() =>
      expect(
        server.calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/decisions`).length,
      ).toBeGreaterThan(1),
    );
  });
});

describe("creating a decision", () => {
  const created = (body: Body) =>
    r.decision("DEC-020", { id: "rec-dec-020", ...body, decided_by: [clara], can_end: true });

  function openCreate(
    setup: Setup = {},
    post: (body: Body) => Response = (b) => json(201, created(b)),
  ) {
    const result = renderWork(
      (api) => (
        <Routes>
          <Route path="/events/:eventId/work">
            <WorkPage api={api} />
          </Route>
        </Routes>
      ),
      {
        url: `/events/${EVENT.id}/work?new=decision`,
        eventManager: true,
        lists: {
          ...r.sampleWork(),
          "/persons": [{ id: "p1", local_id: "PER-001", name: "Cäcilia Probst", version: 1 }],
          "/institutions": [],
        },
        workstreams: STREAMS,
        ...setup,
        answers: { "POST /decisions": (_url, body) => post(body as Body), ...setup.answers },
      },
    );
    return {
      ...result,
      posts: () => result.calls.filter((c) => c.call.startsWith("POST")).map((c) => c.body as Body),
    };
  }

  const form = async () =>
    within(
      await screen.findByRole("complementary", { name: "Neuer Entscheid" }, { timeout: 3000 }),
    );

  it("shows the fields to a manager, with the help on the wording", async () => {
    openCreate();
    const view = await form();
    expect(view.getByRole("textbox", { name: /^Wortlaut/ })).toBeVisible();
    expect(view.getByText(/Der Wortlaut lässt sich später nicht ändern/)).toBeVisible();
    expect(view.getByRole("combobox", { name: /^Beteiligte/ })).toBeVisible();
    expect(view.getByLabelText("Entschieden")).toHaveAttribute("type", "date");
    expect(
      view.getByRole("button", { name: "Verantwortlich (Pflichtfeld): Anna Muster" }),
    ).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Kein Bereich" })).toBeVisible();
  });

  it("says that only the event managers record a decision, to a contributor", async () => {
    openCreate({ eventManager: false });
    const view = await form();
    expect(view.getByText("Nur die Anlassleitung erfasst Entscheide.")).toBeVisible();
    expect(view.queryByRole("button", { name: "Erstellen" })).toBeNull();
  });

  it("needs a wording, a day and a party", async () => {
    const server = openCreate();
    const view = await form();
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    expect(await view.findByText("Der Wortlaut hat 1 bis 2000 Zeichen.")).toBeVisible();
    expect(view.getByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(view.getByText("Wählen Sie 1 bis 50 Personen oder Institutionen.")).toBeVisible();
    expect(view.getByRole("textbox", { name: /^Wortlaut/ })).toHaveFocus();
    expect(server.posts()).toEqual([]);
  });

  it("creates the decision with its parties and opens it", async () => {
    const server = openCreate();
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Wortlaut/ }), "Das Fest findet statt.");
    await user.type(view.getByLabelText("Entschieden"), "2030-03-12");
    await user.click(await view.findByRole("option", { name: "Cäcilia Probst (PER-001)" }, SEARCH));
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await screen.findByRole("complementary", { name: /DEC-020/ });
    expect(server.posts()).toEqual([
      {
        id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
        wording: "Das Fest findet statt.",
        decided_on: "2030-03-12",
        decided_by: [{ kind: "person", id: "p1" }],
        owner_user_id: ME,
      },
    ]);
    expect(window.location.hash).toBe("#DEC-020");
  });

  it("refuses a wording with a control character before it asks the server", async () => {
    const server = openCreate();
    const view = await form();
    await user.click(view.getByRole("textbox", { name: /^Wortlaut/ }));
    await user.paste("Text\u0007");
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    expect(await view.findByText(/enthält ein Zeichen, das nicht erlaubt ist/)).toBeVisible();
    expect(server.posts()).toEqual([]);
  });
});
