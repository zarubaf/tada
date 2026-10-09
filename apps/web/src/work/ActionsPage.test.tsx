import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { BERND, EVENT, json, ME, problem, renderWork } from "../test/fakeWorkServer";
import { ActionsPage } from "./ActionsPage";

const user = userEvent.setup({ delay: null });

function action(over: Record<string, unknown> = {}) {
  return {
    id: "a1",
    local_id: "ACT-001",
    event_id: EVENT.id,
    title: "Generator bestellen",
    owner_user_id: ME,
    workstream_id: "w1",
    due_date: "2030-05-18",
    status: "open",
    version: 2,
    ...over,
  };
}

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <ActionsPage api={api} />, setup);

afterEach(() => window.history.replaceState(null, "", "/"));

describe("ActionsPage", () => {
  it("lists actions with owner, workstream, due date and status", async () => {
    render({ lists: { "/actions": [action()] } });

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    expect(within(table).getByText("ACT-001")).toBeInTheDocument();
    expect(within(table).getByText("Generator bestellen")).toBeInTheDocument();
    expect(within(table).getByText("Anna Muster")).toBeInTheDocument();
    expect(within(table).getByText("Bodenbetrieb")).toBeInTheDocument();
    expect(within(table).getByText("18.05.2030")).toBeInTheDocument();
    expect(within(table).getByText("offen")).toBeInTheDocument();
  });

  it("shows the edit control to the owner, the lead and a manager only", async () => {
    render({
      lists: {
        "/actions": [
          action({ id: "a1", local_id: "ACT-001", owner_user_id: ME, workstream_id: null }),
          action({ id: "a2", local_id: "ACT-002", owner_user_id: BERND, workstream_id: "w1" }),
        ],
      },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.getByRole("button", { name: "ACT-001 bearbeiten" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "ACT-002 bearbeiten" })).not.toBeInTheDocument();
  });

  it("shows every edit control to an event manager", async () => {
    render({
      eventManager: true,
      lists: { "/actions": [action({ owner_user_id: BERND, workstream_id: null })] },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.getByRole("button", { name: "ACT-001 bearbeiten" })).toBeInTheDocument();
  });

  it("shows the edit control to the lead of the workstream", async () => {
    render({
      workstreams: [
        { id: "w1", name: "Bodenbetrieb", lead_user_id: ME, status: "active", version: 1 },
      ],
      lists: { "/actions": [action({ owner_user_id: BERND })] },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.getByRole("button", { name: "ACT-001 bearbeiten" })).toBeInTheDocument();
  });

  it("the action form offers only allowed statuses", async () => {
    render({ lists: { "/actions": [action({ status: "done" })] } });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Status/ }));

    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["erledigt", "offen"]);
  });

  it("creates an action for the member as owner", async () => {
    const created = action({ id: "a2", local_id: "ACT-002", title: "Zelt aufbauen" });
    const { calls } = render({
      answers: { "POST /actions": () => json(201, created) },
      lists: { "/actions": [] },
    });
    await screen.findByText("Noch keine Aufgaben");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Zelt aufbauen");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("ACT-002")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST"))?.body).toEqual({
      title: "Zelt aufbauen",
      owner_user_id: ME,
    });
  });

  it("sends only the changed fields with the version", async () => {
    const { calls } = render({
      answers: { "PATCH /actions/a1": () => json(200, action({ title: "Neu", version: 3 })) },
      lists: { "/actions": [action()] },
    });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    const title = screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" });
    await user.clear(title);
    await user.type(title, "Neu");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("Neu", { selector: "td" })).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      title: "Neu",
      expected_version: 2,
    });
  });

  it("a version conflict shows the reload message", async () => {
    const { calls } = render({
      answers: { "PATCH /actions/a1": () => problem(409, "record-version-conflict") },
      lists: { "/actions": [action()] },
    });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    await user.clear(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }));
    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Neu");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    // The conflict loads the rows again.
    await waitFor(() =>
      expect(calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/actions`)).toHaveLength(
        2,
      ),
    );
    expect(screen.getByRole("heading", { name: "Aufgabe erfassen" })).toBeInTheDocument();
  });

  it("shows the message of a forbidden change", async () => {
    render({
      answers: { "PATCH /actions/a1": () => problem(403, "forbidden") },
      lists: { "/actions": [action()] },
    });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    await user.clear(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }));
    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Neu");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(
      await screen.findByText("Sie haben keine Berechtigung für diese Aktion."),
    ).toBeInTheDocument();
  });

  it("shows the field error of an unknown owner", async () => {
    render({
      answers: {
        "POST /actions": () =>
          problem(422, "validation-failed", [{ pointer: "/owner", code: "unknown-member" }]),
      },
      lists: { "/actions": [] },
    });
    await screen.findByText("Noch keine Aufgaben");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Zelt");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(
      await screen.findByText(
        "Wählen Sie ein Mitglied des Anlasses mit Mitarbeit oder Anlassleitung.",
      ),
    ).toBeInTheDocument();
  });

  it("offers the members with a role as owners to a manager", async () => {
    render({ eventManager: true, lists: { "/actions": [] } });
    await screen.findByText("Noch keine Aufgaben");

    await user.click(screen.getByRole("button", { name: /Verantwortlich/ }));

    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["Anna Muster", "Bernd Beispiel"]);
  });
});
