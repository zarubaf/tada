import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { EVENT, ground, json, problem, renderWork } from "../test/fakeWorkServer";
import { WorkstreamsPage } from "./WorkstreamsPage";

const user = userEvent.setup({ delay: null });

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <WorkstreamsPage api={api} />, setup);

afterEach(() => window.history.replaceState(null, "", "/"));

describe("WorkstreamsPage", () => {
  it("lists workstreams with their lead and status", async () => {
    render();

    const table = await screen.findByRole("table", { name: "Arbeitsbereiche" });
    expect(within(table).getByText("Bodenbetrieb")).toBeInTheDocument();
    expect(within(table).getByText("Cäcilia Probst")).toBeInTheDocument();
    expect(within(table).getByText("aktiv")).toBeInTheDocument();
  });

  it("offers no form and no edit control to a member", async () => {
    render();
    await screen.findByRole("table", { name: "Arbeitsbereiche" });
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Arbeitsbereich anlegen" }),
    ).not.toBeInTheDocument();
  });

  it("lets an event manager create a workstream", async () => {
    // The server keeps the rows: the page loads them again after the save.
    let rows: unknown[] = [];
    const { calls } = render({
      eventManager: true,
      answers: {
        "GET /workstreams": () => json(200, { items: rows }),
        "POST /workstreams": () => {
          rows = [{ ...ground, id: "w2", name: "Gelände" }];
          return json(201, rows[0]);
        },
      },
    });
    await screen.findByText("Noch keine Arbeitsbereiche");

    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Gelände");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("Gelände", { selector: "td" })).toBeInTheDocument();
    expect(
      calls.find((c) => c.call === `POST /api/v1/events/${EVENT.id}/workstreams`)?.body,
    ).toEqual({
      name: "Gelände",
      lead_user_id: "0199b8e0-0000-7000-8000-0000000000b1",
    });
  });

  it("lets an organization admin close a workstream", async () => {
    let rows: unknown[] = [ground];
    const { calls } = render({
      role: "admin",
      answers: {
        "GET /workstreams": () => json(200, { items: rows }),
        "PATCH /workstreams/w1": () => {
          rows = [{ ...ground, status: "closed", version: 2 }];
          return json(200, rows[0]);
        },
      },
    });
    await screen.findByRole("table", { name: "Arbeitsbereiche" });

    await user.click(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Status/ }));
    await user.click(await screen.findByRole("option", { name: "geschlossen" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("geschlossen", { selector: "td" })).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      status: "closed",
      expected_version: 1,
    });
  });

  it("shows the field error of a name that is taken", async () => {
    render({
      eventManager: true,
      answers: {
        "POST /workstreams": () =>
          problem(422, "validation-failed", [{ pointer: "/name", code: "taken" }]),
      },
    });
    await screen.findByRole("table", { name: "Arbeitsbereiche" });

    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Bodenbetrieb");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(
      await screen.findByText("Diesen Namen gibt es in diesem Anlass schon."),
    ).toBeInTheDocument();
  });
});
