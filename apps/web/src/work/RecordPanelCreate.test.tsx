import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Route, Routes } from "../router/Router";
import { EVENT, json, ME, problem, renderWork, type Setup } from "../test/fakeWorkServer";
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

/** The created action as the server returns it. */
const created = (body: Body) =>
  r.action("ACT-020", { id: "rec-act-020", ...body, owner_user_id: body.owner_user_id });

function openWork(
  search: string,
  setup: Setup = {},
  post: (body: Body) => Response | Promise<Response> = (body) => json(201, created(body)),
) {
  const work: Record<string, unknown[]> = r.sampleWork();
  work["/milestones"] = [r.milestone("MIL-001", { title: "Programm steht", id: "m1" })];
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
      answers: { "POST /actions": (_url, body) => post(body as Body) },
      ...setup,
    },
  );
  return {
    ...result,
    posts: () => result.calls.filter((c) => c.call.startsWith("POST")).map((c) => c.body as Body),
  };
}

const form = async () =>
  within(await screen.findByRole("complementary", { name: "Neue Aufgabe" }, { timeout: 3000 }));

describe("the Neu button", () => {
  it("opens the create state and makes a history entry", async () => {
    openWork("?view=table");
    const before = window.history.length;
    await user.click(await screen.findByRole("button", { name: "Neu" }));

    await form();
    expect(window.location.search).toBe("?view=table&new=action");
    expect(window.history.length).toBe(before + 1);
  });

  it("is absent in a closed event", async () => {
    openWork("", { event: { ...EVENT, status: "closed" } });
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(screen.queryByRole("button", { name: "Neu" })).toBeNull();
  });

  it("is offered to an organization admin who is not a member of the event", async () => {
    // The admin is only a viewer of the event, yet has manager level there.
    openWork("", { role: "admin", viewer: true });
    expect(await screen.findByRole("button", { name: "Neu" })).toBeVisible();
  });

  it("is absent for a viewer, who cannot own a record", async () => {
    openWork("", { viewer: true });
    await screen.findByRole("heading", { level: 2, name: /^Offen/ });
    expect(screen.queryByRole("button", { name: "Neu" })).toBeNull();
  });
});

describe("the create state", () => {
  it("shows the fields of an action, with the signed-in member as the owner", async () => {
    openWork("?new=action");
    const view = await form();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toBeVisible();
    expect(view.getByRole("button", { name: "Verantwortlich: Anna Muster" })).toBeVisible();
    expect(view.getByLabelText("Fällig")).toHaveAttribute("type", "date");
    expect(view.getByLabelText("Startdatum")).toHaveAttribute("type", "date");
    expect(view.getByRole("button", { name: "Bereich: Kein Bereich" })).toBeVisible();
    expect(view.getByRole("button", { name: "Meilenstein: Kein Meilenstein" })).toBeVisible();
    expect(view.getByRole("textbox", { name: "Beschreibung" })).toBeVisible();
    expect(view.getByRole("button", { name: "Erstellen" })).toBeVisible();
    expect(view.getByRole("button", { name: "Abbrechen" })).toBeVisible();
  });

  it("starts without an owner when the member cannot own a record, and asks for one", async () => {
    const server = openWork("?new=action", { role: "admin", viewer: true });
    const view = await form();
    expect(view.getByRole("button", { name: "Verantwortlich: Person wählen" })).toBeVisible();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByText(/Wählen Sie ein Mitglied des Anlasses/)).toBeVisible();
    expect(server.posts()).toEqual([]);
  });

  it("needs a title and focuses the field", async () => {
    const server = openWork("?new=action");
    const view = await form();
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const title = await view.findByRole("textbox", { name: /^Titel/ });
    expect(title).toBeInvalid();
    expect(title).toHaveFocus();
    expect(view.getByText("Der Titel hat 1 bis 200 Zeichen.")).toBeVisible();
    expect(server.posts()).toEqual([]);
  });

  it("creates the action and switches the panel to it, with focus on its title", async () => {
    const server = openWork("?view=table&new=action");
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner aufhängen");
    await user.type(view.getByLabelText("Fällig"), "2030-06-01");
    await user.click(view.getByRole("button", { name: "Bereich: Kein Bereich" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await user.type(view.getByRole("textbox", { name: "Beschreibung" }), "Mit zwei Helfern");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const open = await screen.findByRole("complementary", { name: /ACT-020/ });
    expect(server.posts()).toEqual([
      {
        id: expect.any(String),
        title: "Banner aufhängen",
        owner_user_id: ME,
        due_date: "2030-06-01",
        workstream_id: "w2",
        description: "Mit zwei Helfern",
      },
    ]);
    expect(window.location.search).toBe("?view=table");
    expect(window.location.hash).toBe("#ACT-020");
    // The new action is in the list, and its title has focus.
    expect(screen.getByRole("link", { name: "Banner aufhängen" })).toBeInTheDocument();
    await waitFor(() =>
      expect(within(open).getByRole("button", { name: /^Titel bearbeiten/ })).toHaveFocus(),
    );
    expect(screen.getByText("ACT-020 erfasst.")).toBeInTheDocument();
  });

  it("has the live region before the record is created, so that its text is spoken", async () => {
    openWork("?new=action");
    const view = await form();
    const region = view.getByRole("status");
    expect(region).toBeEmptyDOMElement();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const open = await screen.findByRole("complementary", { name: /ACT-020/ });
    // The same element, which only gained its text: a region that remounts with its text is not read.
    expect(within(open).getByRole("status")).toBe(region);
    expect(region).toHaveTextContent("ACT-020 erfasst.");
  });

  it("sends one ID for the record, also on a retry, so that a retry cannot create it twice", async () => {
    let calls = 0;
    const server = openWork("?new=action", {}, (body) =>
      ++calls === 1 ? problem(500, "internal") : json(201, created(body)),
    );
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.click(view.getByRole("button", { name: "Erstellen" }));
    await view.findByRole("alert");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await screen.findByRole("complementary", { name: /ACT-020/ });
    const ids = server.posts().map((body) => body.id);
    expect(ids[0]).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    expect(ids[1]).toBe(ids[0]);
  });

  it("sends one request for a double submit in the same moment", async () => {
    let release: (response: Response) => void = () => undefined;
    const server = openWork(
      "?new=action",
      {},
      (body) =>
        new Promise<Response>((resolve) => {
          release = () => resolve(json(201, created(body)));
        }),
    );
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    const submit = view.getByRole("button", { name: "Erstellen" });
    // Two presses before React has re-rendered.
    act(() => {
      submit.click();
      submit.click();
    });
    await waitFor(() => expect(server.posts()).toHaveLength(1));
    release(json(201, {}));
    await screen.findByRole("complementary", { name: /ACT-020/ });
    expect(server.posts()).toHaveLength(1);
  });

  it("shows the invalid fields that the server names, and keeps the draft", async () => {
    const server = openWork("?new=action", {}, () =>
      problem(422, "validation-failed", [
        { pointer: "/due_date", code: "invalid" },
        { pointer: "/start_date", code: "start-after-due" },
      ]),
    );
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.type(view.getByLabelText("Fällig"), "2031-06-01");
    await user.type(view.getByLabelText("Startdatum"), "2030-01-01");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const due = await view.findByLabelText("Fällig");
    await waitFor(() => expect(due).toBeInvalid());
    expect(view.getByText("Das ist kein gültiges Datum.")).toBeVisible();
    expect(view.getByText(/Das Startdatum liegt nach dem Fälligkeitsdatum/)).toBeVisible();
    expect(due).toHaveFocus();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toHaveValue("Banner");
    expect(server.posts()).toHaveLength(1);
  });

  it("sends nothing when the start date is after the due date, and says so at both dates", async () => {
    const server = openWork("?new=action");
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.type(view.getByLabelText("Fällig"), "2030-06-01");
    await user.type(view.getByLabelText("Startdatum"), "2031-01-01");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    await waitFor(() =>
      expect(view.getByLabelText("Fällig")).toHaveAccessibleDescription(
        "Fälligkeit liegt vor dem Start.",
      ),
    );
    expect(view.getByLabelText("Startdatum")).toHaveAccessibleDescription(
      /Das Startdatum liegt nach dem Fälligkeitsdatum/,
    );
    expect(server.posts()).toEqual([]);
  });

  it("shows another failure in the panel and keeps the draft", async () => {
    openWork("?new=action", {}, () => problem(500, "internal"));
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Banner");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByRole("alert")).toBeVisible();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toHaveValue("Banner");
    expect(screen.getByRole("complementary", { name: "Neue Aufgabe" })).toBeVisible();
  });

  it("closes with Abbrechen and returns focus to the Neu button", async () => {
    openWork("?new=action");
    const view = await form();
    await user.click(view.getByRole("button", { name: "Abbrechen" }));

    await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
    expect(window.location.search).toBe("");
  });

  it("closes with Escape and brings focus back to the button that opened it", async () => {
    openWork("");
    const neu = await screen.findByRole("button", { name: "Neu" });
    await user.click(neu);
    await form();
    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("complementary")).toBeNull());
    expect(screen.getByRole("button", { name: "Neu" })).toHaveFocus();
  });

  it("says that the people did not load, and does not blame the event", async () => {
    openWork("?new=action", { answers: { "GET /api/v1/members": () => problem(500, "internal") } });
    const view = await form();
    expect(view.getByText(/Personen sind nicht geladen/)).toBeVisible();
    expect(view.queryByText(/abgeschlossenen Anlass/)).toBeNull();
    expect(view.queryByRole("button", { name: "Erstellen" })).toBeNull();
  });

  it("explains to a member who may not create that the form is not for them", async () => {
    openWork("?new=action", { event: { ...EVENT, status: "closed" } });
    const view = await form();
    expect(view.getByText(/nichts erfassen/)).toBeVisible();
    expect(view.queryByRole("button", { name: "Erstellen" })).toBeNull();
  });
});
