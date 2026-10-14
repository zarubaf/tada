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

/**
 * Opens MIL-001 in the panel. `patch` answers each change; by default it applies the body to the
 * record and raises the version, as the server does.
 */
function openMilestone(
  over: Body = {},
  setup: Setup = {},
  patch?: (body: Body, current: Body) => Response | Promise<Response>,
) {
  const work: Record<string, unknown[]> = r.sampleWork();
  let current: Body = r.milestone("MIL-001", {
    title: "Programm steht",
    date: "2030-07-01",
    owner_user_id: BERND,
    workstream_id: "w1",
    description: "Alte Beschreibung",
    next_statuses: ["reached", "canceled"],
    ...over,
  });
  work["/milestones"] = [current];
  work["/actions"] = [r.action("ACT-002", { id: "a2", title: "Sponsorenliste" })];
  const result = renderWork(
    (api) => (
      <Routes>
        <Route path="/events/:eventId/work">
          <WorkPage api={api} />
        </Route>
      </Routes>
    ),
    {
      url: `/events/${EVENT.id}/work#MIL-001`,
      lists: work,
      workstreams: STREAMS,
      answers: {
        "PATCH /milestones/rec-mil-001": (_url, body) => {
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

async function region() {
  return within(await screen.findByRole("complementary", { name: /MIL-001/ }, { timeout: 3000 }));
}

describe("a milestone in the panel", () => {
  it("shows the fields of a milestone in order", async () => {
    openMilestone();
    const view = await region();
    expect(view.getByText("MIL-001")).toBeVisible();
    expect(view.getByText("Meilenstein")).toBeVisible();
    expect(view.getByRole("heading", { level: 2, name: "Programm steht" })).toBeVisible();
    const labels = Array.from(document.querySelectorAll("aside dt")).map((dt) => dt.textContent);
    expect(labels).toEqual(["Status", "Datum", "Verantwortlich", "Bereich"]);
    expect(view.getByRole("button", { name: "Status: offen" })).toBeVisible();
    expect(view.getByRole("button", { name: /^Datum:/ })).toHaveTextContent("01.07.2030");
    expect(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" })).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Bodenbetrieb" })).toBeVisible();
    expect(view.getByText("Alte Beschreibung")).toBeVisible();
  });

  it("renames the milestone", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: "Titel: Programm steht" }));
    const input = view.getByRole("textbox", { name: "Titel" });
    await user.clear(input);
    await user.type(input, "Programm veröffentlicht{Enter}");
    await view.findByRole("button", { name: "Titel: Programm veröffentlicht" });
    expect(server.sent()).toEqual([{ title: "Programm veröffentlicht", expected_version: 1 }]);
  });

  it("offers the next statuses of the milestone and changes the status", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: "Status: offen" }));
    const list = await screen.findByRole("listbox", { name: "Status" });
    expect(
      within(list)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["offen", "erreicht", "abgesagt"]);
    await user.click(within(list).getByRole("option", { name: "erreicht" }));
    await view.findByRole("button", { name: "Status: erreicht" });
    expect(server.sent()).toEqual([{ status: "reached", expected_version: 1 }]);
  });

  it("has no status picker for a canceled milestone, which is final", async () => {
    openMilestone({ status: "canceled", next_statuses: [] });
    const view = await region();
    expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
    expect(view.getByText("abgesagt")).toBeVisible();
  });

  it("changes the date", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: /^Datum:/ }));
    const input = view.getByLabelText("Datum");
    await user.clear(input);
    await user.type(input, "2030-08-15");
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(server.sent()).toEqual([{ date: "2030-08-15", expected_version: 1 }]),
    );
    await view.findByText("15.08.2030");
  });

  it("cannot clear the date: it asks for one and sends nothing", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: /^Datum:/ }));
    await user.clear(view.getByLabelText("Datum"));
    await user.keyboard("{Enter}");
    expect(await view.findByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(server.sent()).toEqual([]);
  });

  it("changes the owner and the area, and clears the area", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: "Verantwortlich: Bernd Beispiel" }));
    await user.click(await screen.findByRole("option", { name: "Anna Muster" }));
    await view.findByRole("button", { name: "Verantwortlich: Anna Muster" });
    await user.click(view.getByRole("button", { name: "Bereich: Bodenbetrieb" }));
    await user.click(await screen.findByRole("button", { name: "Kein Bereich" }));
    await view.findByRole("button", { name: "Bereich: Kein Bereich" });
    expect(server.sent()).toEqual([
      { owner_user_id: ME, expected_version: 1 },
      { workstream_id: null, expected_version: 2 },
    ]);
  });

  it("edits the description and can empty it", async () => {
    const server = openMilestone();
    const view = await region();
    await user.click(view.getByRole("button", { name: "Beschreibung bearbeiten" }));
    const input = view.getByRole("textbox", { name: "Beschreibung" });
    await user.clear(input);
    await user.click(view.getByRole("button", { name: "Speichern" }));
    await view.findByText("Keine Beschreibung");
    expect(server.sent()).toEqual([{ description: null, expected_version: 1 }]);
  });

  describe("the hints", () => {
    it("lists the warnings with the readable ID of the late action", async () => {
      openMilestone({ warnings: [{ kind: "overdue" }, { kind: "late-action", action_id: "a2" }] });
      const view = await region();
      const section = view.getByRole("region", { name: "Hinweise" });
      expect(
        within(section)
          .getAllByRole("listitem")
          .map((item) => item.textContent),
      ).toEqual(["Datum überschritten", "ACT-002 ist erst danach fällig"]);
    });

    it("has no section without a warning", async () => {
      openMilestone();
      const view = await region();
      expect(view.queryByRole("region", { name: "Hinweise" })).toBeNull();
    });
  });

  describe("who may change it", () => {
    it("shows plain values when the server says the member cannot change it", async () => {
      openMilestone({ can_change: false });
      const view = await region();
      expect(view.getByRole("heading", { level: 2, name: "Programm steht" })).toBeVisible();
      expect(view.queryByRole("button", { name: /^Titel:/ })).toBeNull();
      expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
      expect(view.queryByRole("button", { name: /^Datum:/ })).toBeNull();
      expect(view.getByText("01.07.2030")).toBeVisible();
    });

    it("shows plain values in a closed event", async () => {
      openMilestone({}, { event: { ...EVENT, status: "closed" } });
      const view = await region();
      expect(view.queryByRole("button", { name: /^Titel:/ })).toBeNull();
    });

    it("shows the tombstone for a redacted milestone and no description", async () => {
      openMilestone({ redacted: true, title: "", description: null });
      const view = await region();
      expect(view.getByText("entfernt")).toBeVisible();
      expect(view.queryByRole("heading", { level: 3, name: "Beschreibung" })).toBeNull();
      expect(view.queryByRole("button", { name: /^Titel:/ })).toBeNull();
    });
  });

  it("keeps the draft and reloads the milestones after a conflict", async () => {
    const server = openMilestone({}, {}, () => problem(409, "record-version-conflict"));
    const view = await region();
    await user.click(view.getByRole("button", { name: /^Titel:/ }));
    await user.clear(view.getByRole("textbox", { name: "Titel" }));
    await user.type(view.getByRole("textbox", { name: "Titel" }), "Mein Titel{Enter}");
    const input = await view.findByRole("textbox", { name: "Titel" });
    await waitFor(() => expect(input).toHaveAccessibleDescription(/inzwischen geändert/));
    expect(input).toHaveValue("Mein Titel");
    await waitFor(() =>
      expect(
        server.calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/milestones`).length,
      ).toBeGreaterThan(1),
    );
  });

  it("lists the sources", async () => {
    openMilestone({
      evidence: [
        {
          proposal_id: "p1",
          record_version: 1,
          source_version_id: "s1",
          captured_at: "2030-03-01T10:00:00Z",
          start_offset: 0,
          end_offset: 5,
          quote: "Das Programm steht bis Juli.",
          redacted: false,
        },
      ],
    });
    const view = await region();
    expect(view.getByRole("heading", { level: 3, name: "Quellen" })).toBeVisible();
    expect(view.getByText(/Das Programm steht bis Juli/)).toBeVisible();
  });
});
