import { screen, waitFor, within } from "@testing-library/react";
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

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

type Body = Record<string, unknown>;

const RISK: Body = {
  title: "Regen ohne Ausweichraum",
  owner_user_id: BERND,
  workstream_id: "w1",
  impact: "high",
  likelihood: "medium",
  description: "Die Turnshow findet im Freien statt.",
  mitigation: "Zelt reservieren",
  next_statuses: ["mitigated", "occurred", "closed"],
};

/** Opens RSK-001 in the panel. `patch` answers each change; by default it applies the body. */
function openRisk(
  over: Body = {},
  setup: Setup = {},
  patch?: (body: Body, current: Body) => Response | Promise<Response>,
) {
  const work: Record<string, unknown[]> = r.sampleWork();
  let current: Body = r.risk("RSK-001", { ...RISK, ...over });
  work["/risks"] = [current];
  const result = renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work#RSK-001`,
      lists: work,
      workstreams: STREAMS,
      answers: {
        "PATCH /risks/rec-rsk-001": (_url, body) => {
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
    work,
    sent: () => result.calls.filter((c) => c.call.startsWith("PATCH")).map((c) => c.body as Body),
  };
}

const panel = () => screen.findByRole("complementary", { name: /RSK-001/ }, { timeout: 3000 });
async function region() {
  return within(await panel());
}

describe("a risk in the panel", () => {
  it("shows the title, the rating in words, the owner, the area and both long texts", async () => {
    openRisk();
    const view = await region();
    expect(view.getByRole("heading", { level: 2, name: "Regen ohne Ausweichraum" })).toBeVisible();
    expect(view.getByRole("button", { name: "Status: offen" })).toBeVisible();
    expect(view.getByRole("button", { name: "Wirkung: hoch" })).toBeVisible();
    expect(view.getByRole("button", { name: "Wahrscheinlichkeit: mittel" })).toBeVisible();
    expect(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeVisible();
    expect(view.getByText("Die Turnshow findet im Freien statt.")).toBeVisible();
    expect(view.getByRole("heading", { level: 3, name: "Gegenmassnahme" })).toBeVisible();
    expect(view.getByText("Zelt reservieren")).toBeVisible();
    // A risk has no date.
    expect(view.queryByText("Fällig")).toBeNull();
    expect(view.queryByText("Startdatum")).toBeNull();
  });

  it("shows the words for an empty description and an empty mitigation", async () => {
    openRisk({ description: null, mitigation: null });
    const view = await region();
    expect(view.getByText("Keine Beschreibung")).toBeVisible();
    expect(view.getByText("Keine Gegenmassnahme")).toBeVisible();
  });

  it("names the version that a passage of evidence supports", async () => {
    openRisk({
      evidence: [
        {
          proposal_id: "p1",
          record_version: 2,
          source_version_id: "s1",
          captured_at: "2030-03-01T10:00:00Z",
          start_offset: 0,
          end_offset: 5,
          quote: "Regen ist angesagt",
          redacted: false,
        },
      ],
    });
    const view = await region();
    expect(await view.findByText("Stützt die Version 2 des Risikos")).toBeVisible();
  });

  describe("the rating", () => {
    it("opens the three levels with the keyboard, without a search field, and saves one", async () => {
      const server = openRisk();
      const view = await region();
      const trigger = view.getByRole("button", { name: "Wirkung: hoch" });
      trigger.focus();
      await user.keyboard("{Enter}");
      const list = await screen.findByRole("listbox", { name: "Wirkung" });
      expect(
        within(list)
          .getAllByRole("option")
          .map((option) => option.textContent),
      ).toEqual(["niedrig", "mittel", "hoch"]);
      expect(screen.queryByRole("searchbox")).toBeNull();
      // Each level is a word, and the icon before it is a decoration.
      for (const option of within(list).getAllByRole("option")) {
        expect(option.querySelector("svg[aria-hidden='true']")).not.toBeNull();
      }
      await user.keyboard("{ArrowUp}{ArrowUp}{Enter}");

      await view.findByRole("button", { name: "Wirkung: niedrig" });
      expect(server.sent()).toEqual([{ impact: "low", expected_version: 1 }]);
      expect(screen.getByText("Wirkung von RSK-001 gespeichert.")).toBeInTheDocument();
      // Focus is back on the trigger.
      await waitFor(() => expect(view.getByRole("button", { name: /^Wirkung:/ })).toHaveFocus());
    });

    it("changes the likelihood by a click", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Wahrscheinlichkeit: mittel" }));
      await user.click(await screen.findByRole("option", { name: "hoch" }));

      await view.findByRole("button", { name: "Wahrscheinlichkeit: hoch" });
      expect(server.sent()).toEqual([{ likelihood: "high", expected_version: 1 }]);
    });

    it("cannot clear a level", async () => {
      openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Wirkung: hoch" }));
      await screen.findByRole("listbox", { name: "Wirkung" });
      expect(screen.queryByRole("button", { name: /Keine|Stufe/ })).toBeNull();
    });

    it("keeps the choice after a failure and offers to try again", async () => {
      let calls = 0;
      const server = openRisk({}, {}, (body, current) =>
        ++calls === 1 ? problem(500, "internal") : json(200, { ...current, ...body, version: 2 }),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: "Wirkung: hoch" }));
      await user.click(await screen.findByRole("option", { name: "mittel" }));

      await view.findByRole("button", { name: "Erneut versuchen" });
      await user.click(view.getByRole("button", { name: "Erneut versuchen" }));
      await view.findByRole("button", { name: "Wirkung: mittel" });
      expect(server.sent()).toHaveLength(2);
    });

    it("tells the member of a conflict and shows what the record holds now", async () => {
      const served: { lists?: Record<string, unknown[]> } = {};
      const server = openRisk({}, {}, (_body, current) => {
        const lists = served.lists ?? {};
        lists["/risks"] = [{ ...current, impact: "low", version: 2 }];
        return problem(409, "record-version-conflict");
      });
      served.lists = server.work;
      const view = await region();
      await user.click(view.getByRole("button", { name: "Wirkung: hoch" }));
      await user.click(await screen.findByRole("option", { name: "mittel" }));

      await view.findByText("Jetzt gespeichert: niedrig");
      expect(view.getAllByText(/Der Eintrag wurde inzwischen geändert/)[0]).toBeVisible();
      expect(view.getByRole("button", { name: "Erneut versuchen" })).toBeVisible();
      expect(view.getByRole("button", { name: "Verwerfen" })).toBeVisible();
    });

    it("shows the message of the server at the field", async () => {
      openRisk({}, {}, () =>
        problem(422, "validation-failed", [{ pointer: "/impact", code: "invalid" }]),
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: "Wirkung: hoch" }));
      await user.click(await screen.findByRole("option", { name: "niedrig" }));
      expect(await view.findByText("Wählen Sie eine Stufe für die Wirkung.")).toBeVisible();
    });
  });

  describe("the status", () => {
    it("moves among the next statuses of the record", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Status: offen" }));
      const list = await screen.findByRole("listbox", { name: "Status" });
      expect(
        within(list)
          .getAllByRole("option")
          .map((option) => option.textContent),
      ).toEqual(["offen", "gemindert", "eingetreten", "geschlossen"]);
      await user.click(within(list).getByRole("option", { name: "gemindert" }));

      await view.findByRole("button", { name: "Status: gemindert" });
      expect(server.sent()).toEqual([{ status: "mitigated", expected_version: 1 }]);
    });
  });

  describe("the texts", () => {
    it("edits the mitigation in place and saves it with Speichern", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Gegenmassnahme bearbeiten" }));
      const input = view.getByRole("textbox", { name: "Gegenmassnahme" });
      await user.clear(input);
      await user.type(input, "Zelt{Enter}und Helfer");
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByText(/und Helfer/);
      expect(server.sent()).toEqual([{ mitigation: "Zelt\nund Helfer", expected_version: 1 }]);
    });

    it("clears the mitigation when the text is empty", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Gegenmassnahme bearbeiten" }));
      await user.clear(view.getByRole("textbox", { name: "Gegenmassnahme" }));
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByText("Keine Gegenmassnahme");
      expect(server.sent()).toEqual([{ mitigation: null, expected_version: 1 }]);
    });

    it("edits the description", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: "Beschreibung bearbeiten" }));
      await user.type(view.getByRole("textbox", { name: "Beschreibung" }), " Neu.");
      await user.click(view.getByRole("button", { name: "Speichern" }));

      await view.findByText(/Neu\./);
      expect(server.sent()).toEqual([
        { description: "Die Turnshow findet im Freien statt. Neu.", expected_version: 1 },
      ]);
    });

    it("renames the risk", async () => {
      const server = openRisk();
      const view = await region();
      await user.click(view.getByRole("button", { name: /^Titel:/ }));
      const input = view.getByRole("textbox", { name: "Titel" });
      await user.clear(input);
      await user.type(input, "Starkregen{Enter}");

      await view.findByRole("button", { name: "Titel: Starkregen" });
      expect(server.sent()).toEqual([{ title: "Starkregen", expected_version: 1 }]);
    });
  });

  describe("who may change it", () => {
    it("shows plain words for a risk that the member may not change", async () => {
      openRisk({ can_change: false });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten/ })).toBeNull();
      expect(
        view.queryByRole("button", { name: /^(Status|Wirkung|Wahrscheinlichkeit):/ }),
      ).toBeNull();
      // The words stay, so that the rating is readable without a control.
      expect(view.getByText("hoch")).toBeVisible();
      expect(view.getByText("mittel")).toBeVisible();
      expect(view.getByText("Zelt reservieren")).toBeVisible();
    });

    it("shows plain words in a closed event", async () => {
      openRisk({}, { event: { ...EVENT, status: "closed" } });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten|^Wirkung:/ })).toBeNull();
      expect(view.getByText("hoch")).toBeVisible();
    });

    it("has no control and shows the tombstone for a redacted risk", async () => {
      openRisk({ redacted: true, title: "", description: null, mitigation: null });
      const view = await region();
      expect(view.queryByRole("button", { name: /bearbeiten|^Wirkung:/ })).toBeNull();
      expect(view.getByText("entfernt")).toBeVisible();
    });
  });

  describe("on the page", () => {
    it("closes with the button, removes the hash and keeps the query", async () => {
      openRisk({}, { url: `/events/${EVENT.id}/work?type=risk#RSK-001` });
      const view = await region();
      await user.click(view.getByRole("button", { name: "Schliessen" }));
      await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
      expect(window.location.hash).toBe("");
      expect(window.location.search).toBe("?type=risk");
    });

    it("steps from a risk to the next one that opens here", async () => {
      const work = r.sampleWork();
      work["/risks"] = [
        r.risk("RSK-001", { status: "open" }),
        r.risk("RSK-002", { status: "open" }),
      ];
      renderWork(
        (api) => (
          <Routes>
            <Route path="/events/:eventId/work">
              <WorkPage api={api} />
            </Route>
          </Routes>
        ),
        { url: `/events/${EVENT.id}/work?type=risk#RSK-001`, lists: work, workstreams: STREAMS },
      );
      const view = await region();
      await user.click(view.getByRole("button", { name: "Nächster Eintrag" }));
      expect(window.location.hash).toBe("#RSK-002");
    });

    it("links the card of the risk to the panel", async () => {
      openRisk({}, { url: `/events/${EVENT.id}/work` });
      const link = await screen.findByRole("link", { name: /RSK-001/ });
      expect(link).toHaveAttribute("href", `/events/${EVENT.id}/work#RSK-001`);
    });
  });
});

describe("the create state of a risk", () => {
  const created = (body: Body) => r.risk("RSK-002", { id: "rec-rsk-002", ...body });

  function openCreate(
    setup: Setup = {},
    post: (body: Body) => Response | Promise<Response> = (body) => json(201, created(body)),
  ) {
    const work: Record<string, unknown[]> = r.sampleWork();
    const result = renderWork(
      (api) => (
        <Routes>
          <Route path="/events/:eventId/work">
            <WorkPage api={api} />
          </Route>
        </Routes>
      ),
      {
        url: `/events/${EVENT.id}/work?new=risk`,
        lists: work,
        workstreams: STREAMS,
        ...setup,
        answers: { "POST /risks": (_url, body) => post(body as Body), ...setup.answers },
      },
    );
    return {
      ...result,
      work,
      posts: () => result.calls.filter((c) => c.call.startsWith("POST")).map((c) => c.body as Body),
    };
  }
  const form = async () =>
    within(await screen.findByRole("complementary", { name: "Neues Risiko" }, { timeout: 3000 }));

  it("shows the fields of a risk and no date, with the member as the owner", async () => {
    openCreate();
    const view = await form();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toBeVisible();
    expect(view.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" })).toBeVisible();
    expect(
      view.getByRole("button", { name: "Wahrscheinlichkeit (Pflichtfeld): Stufe wählen" }),
    ).toBeVisible();
    expect(
      view.getByRole("button", { name: "Verantwortlich (Pflichtfeld): Anna Muster" }),
    ).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Kein Bereich" })).toBeVisible();
    expect(view.getByRole("textbox", { name: "Beschreibung" })).toBeVisible();
    expect(view.getByRole("textbox", { name: "Gegenmassnahme" })).toBeVisible();
    expect(view.queryByLabelText("Fällig")).toBeNull();
  });

  it("needs a title and both levels, and says which", async () => {
    const server = openCreate();
    const view = await form();
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByText("Der Titel hat 1 bis 200 Zeichen.")).toBeVisible();
    expect(view.getByText("Wählen Sie eine Stufe für die Wirkung.")).toBeVisible();
    expect(view.getByText("Wählen Sie eine Stufe für die Wahrscheinlichkeit.")).toBeVisible();
    expect(server.posts()).toEqual([]);
  });

  it("ties the error of a required level to its picker for assistive technology", async () => {
    openCreate();
    const view = await form();
    const impact = view.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" });
    expect(impact).toHaveAccessibleDescription("");
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Stromausfall");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await view.findByText("Wählen Sie eine Stufe für die Wirkung.");
    expect(impact).toHaveAccessibleDescription("Wählen Sie eine Stufe für die Wirkung.");
    const likelihood = view.getByRole("button", {
      name: "Wahrscheinlichkeit (Pflichtfeld): Stufe wählen",
    });
    expect(likelihood).toHaveAccessibleDescription(
      "Wählen Sie eine Stufe für die Wahrscheinlichkeit.",
    );
    await waitFor(() => expect(impact).toHaveFocus());
  });

  it("creates the risk with one client ID and switches the panel to it", async () => {
    const server = openCreate();
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Stromausfall");
    await user.click(view.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" }));
    await user.click(await screen.findByRole("option", { name: "hoch" }));
    await user.click(
      view.getByRole("button", { name: "Wahrscheinlichkeit (Pflichtfeld): Stufe wählen" }),
    );
    await user.click(await screen.findByRole("option", { name: "niedrig" }));
    await user.click(view.getByRole("button", { name: "Bereich: Kein Bereich" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await user.type(view.getByRole("textbox", { name: "Gegenmassnahme" }), "Aggregat mieten");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await screen.findByRole("complementary", { name: /RSK-002/ });
    expect(server.posts()).toEqual([
      {
        id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
        title: "Stromausfall",
        impact: "high",
        likelihood: "low",
        owner_user_id: ME,
        workstream_id: "w2",
        mitigation: "Aggregat mieten",
      },
    ]);
    expect(window.location.search).toBe("");
    expect(window.location.hash).toBe("#RSK-002");
    expect(screen.getByText("RSK-002 erfasst.")).toBeInTheDocument();
  });

  it("shows the message of the server at the owner and keeps the draft", async () => {
    const server = openCreate({}, () =>
      problem(422, "validation-failed", [{ pointer: "/owner", code: "unknown-member" }]),
    );
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Stromausfall");
    await user.click(view.getByRole("button", { name: "Wirkung (Pflichtfeld): Stufe wählen" }));
    await user.click(await screen.findByRole("option", { name: "hoch" }));
    await user.click(
      view.getByRole("button", { name: "Wahrscheinlichkeit (Pflichtfeld): Stufe wählen" }),
    );
    await user.click(await screen.findByRole("option", { name: "mittel" }));
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByText(/Wählen Sie ein Mitglied des Anlasses/)).toBeVisible();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toHaveValue("Stromausfall");
    expect(view.getByRole("button", { name: "Wirkung (Pflichtfeld): hoch" })).toBeVisible();
    expect(server.posts()).toHaveLength(1);
  });

  it("says why nothing can be created for a viewer", async () => {
    openCreate({ viewer: true });
    const region = await screen.findByRole("complementary", { name: "Neues Risiko" });
    expect(within(region).getByText(/Sie können hier nichts erfassen/)).toBeVisible();
  });
});
