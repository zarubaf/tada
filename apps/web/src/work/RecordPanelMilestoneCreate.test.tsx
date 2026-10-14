import { screen, waitFor, within } from "@testing-library/react";
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

function openWork(
  search: string,
  setup: Setup = {},
  post: (body: Body) => Response | Promise<Response> = (body) =>
    json(201, r.milestone("MIL-020", { id: "rec-mil-020", ...body })),
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
      url: `/events/${EVENT.id}/work${search}`,
      lists: r.sampleWork(),
      workstreams: STREAMS,
      ...setup,
      answers: { "POST /milestones": (_url, body) => post(body as Body), ...setup.answers },
    },
  );
  return {
    ...result,
    posts: () => result.calls.filter((c) => c.call.startsWith("POST")).map((c) => c.body as Body),
  };
}

const form = async () =>
  within(
    await screen.findByRole("complementary", { name: "Neuer Meilenstein" }, { timeout: 3000 }),
  );

describe("creating a milestone in the panel", () => {
  it("shows the fields of a milestone, with the signed-in member as the owner", async () => {
    openWork("?new=milestone");
    const view = await form();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toBeVisible();
    expect(view.getByLabelText("Datum")).toHaveAttribute("type", "date");
    expect(
      view.getByRole("button", { name: "Verantwortlich (Pflichtfeld): Anna Muster" }),
    ).toBeVisible();
    expect(view.getByRole("button", { name: "Bereich: Kein Bereich" })).toBeVisible();
    expect(view.getByRole("textbox", { name: "Beschreibung" })).toBeVisible();
    expect(view.queryByRole("button", { name: /^Status:/ })).toBeNull();
  });

  it("needs a title and a date, and focuses the first field with an error", async () => {
    const server = openWork("?new=milestone");
    const view = await form();
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const title = await view.findByRole("textbox", { name: /^Titel/ });
    expect(title).toBeInvalid();
    expect(title).toHaveFocus();
    expect(view.getByText("Der Titel hat 1 bis 200 Zeichen.")).toBeVisible();
    expect(view.getByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(server.posts()).toEqual([]);
  });

  it("creates the milestone with a client ID and shows it in the panel", async () => {
    const server = openWork("?view=table&new=milestone");
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Zelt steht");
    await user.type(view.getByLabelText("Datum"), "2030-06-01");
    await user.click(view.getByRole("button", { name: "Bereich: Kein Bereich" }));
    await user.click(await screen.findByRole("option", { name: "Festwirtschaft" }));
    await user.type(view.getByRole("textbox", { name: "Beschreibung" }), "Bis zum Abend");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    const open = await screen.findByRole("complementary", { name: /MIL-020/ });
    expect(server.posts()).toEqual([
      {
        id: expect.stringMatching(/^[0-9a-f]{8}-[0-9a-f]{4}-7/),
        title: "Zelt steht",
        date: "2030-06-01",
        owner_user_id: ME,
        workstream_id: "w2",
        description: "Bis zum Abend",
      },
    ]);
    expect(window.location.hash).toBe("#MIL-020");
    expect(window.location.search).toBe("?view=table");
    await waitFor(() =>
      expect(within(open).getByRole("button", { name: /^Titel:/ })).toHaveFocus(),
    );
    expect(screen.getByText("MIL-020 erfasst.")).toBeInTheDocument();
  });

  it("shows a refused date at the field and keeps the draft", async () => {
    openWork("?new=milestone", {}, () =>
      problem(422, "validation-failed", [{ pointer: "/date", code: "invalid" }]),
    );
    const view = await form();
    await user.type(view.getByRole("textbox", { name: /^Titel/ }), "Zelt steht");
    await user.type(view.getByLabelText("Datum"), "2030-06-01");
    await user.click(view.getByRole("button", { name: "Erstellen" }));

    expect(await view.findByText("Geben Sie ein gültiges Datum an.")).toBeVisible();
    expect(view.getByRole("textbox", { name: /^Titel/ })).toHaveValue("Zelt steht");
  });

  it("says that a viewer cannot create one", async () => {
    openWork("?new=milestone", { viewer: true });
    const view = await form();
    expect(view.getByText(/Sie können hier nichts erfassen/)).toBeVisible();
  });
});
