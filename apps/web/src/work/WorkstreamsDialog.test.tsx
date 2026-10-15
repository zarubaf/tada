import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EVENT, ground, json, problem, renderWork } from "../test/fakeWorkServer";
import { DirectoryGate } from "./DirectoryGate";
import { useDirectory } from "./directory";
import { WorkstreamsDialog } from "./WorkstreamsDialog";

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

/** The dialog over a directory that it reloads after a save, as the „Arbeit“ page does. */
function render(setup: Parameters<typeof renderWork>[1] = {}, writable = true) {
  const onClose = vi.fn();
  const rendered = renderWork(
    (api) => <Harness api={api} writable={writable} onClose={onClose} />,
    setup,
  );
  return { ...rendered, onClose };
}

function Harness({
  api,
  writable,
  onClose,
}: {
  api: Parameters<Parameters<typeof renderWork>[0]>[0];
  writable: boolean;
  onClose: () => void;
}) {
  const { state, reload } = useDirectory(api, EVENT.id);
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => (
        <WorkstreamsDialog
          api={api}
          eventId={EVENT.id}
          directory={directory}
          writable={writable}
          reloadDirectory={reload}
          onClose={onClose}
        />
      )}
    </DirectoryGate>
  );
}

const list = () => screen.findByRole("list", { name: "Bereiche" });

describe("WorkstreamsDialog", () => {
  it("is a dialog named Bereiche verwalten that lists the workstreams with lead and status", async () => {
    render();

    const dialog = await screen.findByRole("dialog", { name: "Bereiche verwalten" });
    const row = within(await list()).getByRole("listitem");
    expect(within(row).getByText("Bodenbetrieb")).toBeInTheDocument();
    expect(within(row).getByText("Cäcilia Probst")).toBeInTheDocument();
    expect(within(row).getByText("aktiv")).toBeInTheDocument();
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
  });

  it("offers a member no form and no edit control", async () => {
    render();
    await list();
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Bereich anlegen" })).not.toBeInTheDocument();
  });

  it("offers a viewer of the event the list only", async () => {
    render({ viewer: true });
    await list();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("says so when the event has no workstream yet", async () => {
    render({ workstreams: [] });
    expect(await screen.findByText("Noch keine Bereiche")).toBeInTheDocument();
  });

  it("closes with Escape and with the close button", async () => {
    const { onClose } = render();
    await list();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Schliessen" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("lets an event manager create a workstream and loads the list again", async () => {
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
    await screen.findByText("Noch keine Bereiche");

    const name = screen.getByRole("textbox", { name: "Name (Pflichtfeld)" });
    await user.type(name, "Gelände");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await within(await list()).findByText("Gelände")).toBeInTheDocument();
    expect(
      calls.find((c) => c.call === `POST /api/v1/events/${EVENT.id}/workstreams`)?.body,
    ).toEqual({ name: "Gelände", lead_user_id: "0199b8e0-0000-7000-8000-0000000000b1" });
    expect(await screen.findByText("„Gelände“ gespeichert.")).toBeInTheDocument();
    // The form is ready for the next name.
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" })).toHaveFocus(),
    );
    expect(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" })).toHaveValue("");
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
    await list();

    await user.click(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Status/ }));
    await user.click(await screen.findByRole("option", { name: "geschlossen" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await within(await list()).findByText("geschlossen")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
      status: "closed",
      expected_version: 1,
    });
    // The form is gone and focus is back on the edit button of the row.
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" })).toHaveFocus(),
    );
  });

  it("reopens a closed workstream", async () => {
    const closed = { ...ground, status: "closed", version: 3 };
    const { calls } = render({
      eventManager: true,
      workstreams: [closed],
      answers: { "PATCH /workstreams/w1": () => json(200, { ...closed, status: "active" }) },
    });
    await list();

    await user.click(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    await user.click(screen.getByRole("button", { name: /Status/ }));
    await user.click(await screen.findByRole("option", { name: "aktiv" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
        status: "active",
        expected_version: 3,
      }),
    );
  });

  it("renames a workstream and sends only the name", async () => {
    const { calls } = render({
      eventManager: true,
      answers: {
        "PATCH /workstreams/w1": () => json(200, { ...ground, name: "Flugplatz", version: 2 }),
      },
    });
    await list();

    await user.click(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    expect(screen.getByRole("heading", { name: "„Bodenbetrieb“ bearbeiten" })).toBeInTheDocument();
    const name = screen.getByRole("textbox", { name: "Name (Pflichtfeld)" });
    await user.clear(name);
    await user.type(name, "Flugplatz");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
        name: "Flugplatz",
        expected_version: 1,
      }),
    );
  });

  it("shows the field error of a name that is taken", async () => {
    render({
      eventManager: true,
      answers: {
        "POST /workstreams": () =>
          problem(422, "validation-failed", [{ pointer: "/name", code: "taken" }]),
      },
    });
    await list();

    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Bodenbetrieb");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(
      await screen.findByText("Diesen Namen gibt es in diesem Anlass schon."),
    ).toBeInTheDocument();
  });

  it("asks for a name before it sends", async () => {
    const { calls } = render({ eventManager: true });
    await list();
    await user.click(screen.getByRole("button", { name: "Erfassen" }));
    expect(await screen.findByText("Der Name hat 1 bis 200 Zeichen.")).toBeInTheDocument();
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });

  it("says a failure that no field shows, as an alert", async () => {
    render({
      eventManager: true,
      answers: { "POST /workstreams": () => problem(500, "internal") },
    });
    await list();

    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Gelände");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/\S/);
  });

  it("closes the form and loads the list again after a version conflict", async () => {
    let rows: unknown[] = [ground];
    const { calls } = render({
      eventManager: true,
      answers: {
        "GET /workstreams": () => json(200, { items: rows }),
        "PATCH /workstreams/w1": () => {
          rows = [{ ...ground, name: "Boden neu", version: 2 }];
          return problem(409, "record-version-conflict");
        },
      },
    });
    await list();

    await user.click(screen.getByRole("button", { name: "Bodenbetrieb bearbeiten" }));
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "x");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("neu geladen");
    expect(await within(await list()).findByText("Boden neu")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Speichern" })).not.toBeInTheDocument();
    expect(
      calls.filter((c) => c.call.endsWith("/workstreams") && c.call.startsWith("GET")),
    ).toHaveLength(2);
  });

  it("shows entfernt for a redacted workstream and offers no edit", async () => {
    render({ role: "admin", workstreams: [{ ...ground, name: "", redacted: true }] });
    expect(await within(await list()).findByText("entfernt")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
  });

  it("hides the edit control and the form in a closed event", async () => {
    render({ eventManager: true }, false);
    await list();
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
