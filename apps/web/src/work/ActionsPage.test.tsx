import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { EVENT, json, ME, pagedList, problem, renderWork } from "../test/fakeWorkServer";
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
    can_change: true,
    next_statuses: ["in-progress", "blocked", "done", "canceled"],
    ...over,
  };
}

function milestone(over: Record<string, unknown> = {}) {
  return {
    id: "m1",
    local_id: "MIL-001",
    event_id: EVENT.id,
    title: "Festzelt steht",
    redacted: false,
    date: "2030-05-20",
    owner_user_id: ME,
    status: "open",
    warnings: [],
    version: 1,
    evidence: [],
    can_change: true,
    next_statuses: ["reached", "canceled"],
    ...over,
  };
}

/** An answer that the test sends when it calls `release`. */
function held() {
  let release: (response: Response) => void = () => {};
  const response = new Promise<Response>((resolve) => {
    release = resolve;
  });
  return { response, release };
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

  it("opens the row named by the hash of the address and focuses it", async () => {
    render({
      hash: "#ACT-002",
      lists: {
        "/actions": [
          action({ id: "a1", local_id: "ACT-001" }),
          action({ id: "a2", local_id: "ACT-002", title: "Zelt aufbauen" }),
        ],
      },
    });

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    const row = within(table).getByText("Zelt aufbauen").closest("tr");
    expect(row).toHaveAttribute("id", "ACT-002");
    await waitFor(() => expect(row).toHaveFocus());
    expect(row).toHaveAttribute("data-linked");
    expect(within(table).getByText("Generator bestellen").closest("tr")).not.toHaveAttribute(
      "data-linked",
    );
  });

  it("says that a hash which names no row is not found, and marks no row", async () => {
    render({ hash: "#ACT-099", lists: { "/actions": [action()] } });
    await screen.findByRole("table", { name: "Aufgaben" });
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("ACT-099 nicht gefunden"),
    );
    expect(document.querySelector("[data-linked]")).toBeNull();
  });

  it("does not search for a fragment that is no row ID, such as the skip link", async () => {
    const { calls } = render({
      hash: "#main",
      answers: {
        "GET /actions": pagedList([
          [action({ id: "a1", local_id: "ACT-001" })],
          [action({ id: "a2", local_id: "ACT-002" })],
        ]),
      },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    await act(async () => {});
    expect(calls.filter((c) => c.call.endsWith("/actions"))).toHaveLength(1);
    expect(screen.queryByText(/nicht gefunden/)).not.toBeInTheDocument();
  });

  it("shows the button as pending while the search runs and appends the page of the search once", async () => {
    let release: (response: Response) => void = () => {};
    const second = new Promise<Response>((resolve) => {
      release = resolve;
    });
    let served = 0;
    const { calls } = render({
      hash: "#ACT-002",
      answers: {
        "GET /actions": () => {
          served += 1;
          if (served === 1) {
            return json(200, {
              items: [action({ id: "a1", local_id: "ACT-001" })],
              next_cursor: "p1",
            });
          }
          return second;
        },
      },
    });

    const more = await screen.findByRole("button", { name: "Weitere Einträge laden" });
    await waitFor(() => expect(more).toHaveAttribute("aria-disabled", "true"));
    await user.click(more);
    release(json(200, { items: [action({ id: "a2", local_id: "ACT-002" })], next_cursor: null }));

    await screen.findByText("ACT-002");
    expect(screen.getAllByText("ACT-002")).toHaveLength(1);
    expect(calls.filter((c) => c.call.endsWith("/actions"))).toHaveLength(2);
  });

  it("does not fetch a page twice when the search continues during an open request", async () => {
    let release: (response: Response) => void = () => {};
    const second = new Promise<Response>((resolve) => {
      release = resolve;
    });
    let served = 0;
    const { calls } = render({
      answers: {
        "GET /actions": () => {
          served += 1;
          if (served === 1) {
            return json(200, {
              items: [action({ id: "a1", local_id: "ACT-001" })],
              next_cursor: "p1",
            });
          }
          return second;
        },
      },
    });

    const more = await screen.findByRole("button", { name: "Weitere Einträge laden" });
    await user.click(more);
    act(() => {
      window.history.pushState(null, "", `${window.location.pathname}#ACT-002`);
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    release(json(200, { items: [action({ id: "a2", local_id: "ACT-002" })], next_cursor: null }));

    await screen.findByText("ACT-002");
    await act(async () => {});
    expect(screen.getAllByText("ACT-002")).toHaveLength(1);
    expect(calls.filter((c) => c.call.endsWith("/actions"))).toHaveLength(2);
    // The last page also moves focus to the heading, but the row of the address wins.
    expect(screen.getByText("ACT-002").closest("tr")).toHaveFocus();
    expect(screen.getByRole("heading", { level: 2, name: "Aufgaben" })).not.toHaveFocus();
  });

  it("stops the search when the server repeats a cursor", async () => {
    const { calls } = render({
      hash: "#ACT-099",
      answers: {
        "GET /actions": () =>
          json(200, { items: [action({ id: "a1", local_id: "ACT-001" })], next_cursor: "same" }),
      },
    });

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("ACT-099 nicht gefunden"),
    );
    expect(calls.filter((c) => c.call.endsWith("/actions"))).toHaveLength(2);
  });

  it("searches again for a row after a failed search, a non-row fragment and a return", async () => {
    let served = 0;
    const { calls } = render({
      hash: "#ACT-050",
      answers: {
        "GET /actions": () => {
          served += 1;
          if (served === 1) {
            return json(200, {
              items: [action({ id: "a1", local_id: "ACT-001" })],
              next_cursor: "p1",
            });
          }
          if (served === 2) {
            return problem(500, "internal");
          }
          return json(200, {
            items: [action({ id: "a50", local_id: "ACT-050", title: "Zelt aufbauen" })],
            next_cursor: null,
          });
        },
      },
    });
    // The alert region is always there: wait until it holds the failure of the second request.
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(/\S/));
    const go = (hash: string) =>
      act(() => {
        window.history.pushState(null, "", `${window.location.pathname}${hash}`);
        window.dispatchEvent(new PopStateEvent("popstate"));
      });
    go("#main");
    go("#ACT-050");

    const row = (await screen.findByText("Zelt aufbauen")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
    expect(screen.queryByText(/nicht gefunden/)).not.toBeInTheDocument();
    expect(calls.filter((c) => c.call.endsWith("/actions"))).toHaveLength(3);
  });

  for (const order of ["before", "after"] as const) {
    it(`searches the new list when a conflict reloads it during the search (old page ${order})`, async () => {
      const old = held();
      const reloaded = held();
      let first = 0;
      render({
        hash: "#ACT-050",
        answers: {
          "PATCH /actions/a1": () => problem(409, "record-version-conflict"),
          "GET /actions": (url) => {
            const cursor = url.searchParams.get("cursor");
            if (cursor === null) {
              first += 1;
              const page = json(200, { items: [action()], next_cursor: "c1" });
              return first === 1 ? page : reloaded.response;
            }
            if (cursor === "c1") {
              return first === 1
                ? old.response
                : json(200, {
                    items: [action({ id: "a2", local_id: "ACT-002" })],
                    next_cursor: "c2",
                  });
            }
            return json(200, {
              items: [action({ id: "a50", local_id: "ACT-050", title: "Zelt aufbauen" })],
            });
          },
        },
      });
      await user.click(await screen.findByRole("button", { name: "ACT-001 bearbeiten" }));
      await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), " neu");
      await user.click(screen.getByRole("button", { name: "Speichern" }));
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen.");
      const page = () => json(200, { items: [action()], next_cursor: "c1" });
      const late = () =>
        json(200, { items: [action({ id: "a9", local_id: "ACT-009", title: "Spät" })] });
      if (order === "before") {
        old.release(late());
        await act(async () => {});
        reloaded.release(page());
      } else {
        reloaded.release(page());
        await screen.findByText("ACT-002");
        old.release(late());
      }

      const row = (await screen.findByText("Zelt aufbauen")).closest("tr");
      await waitFor(() => expect(row).toHaveFocus());
      await act(async () => {});
      expect(screen.queryByText("Spät")).not.toBeInTheDocument();
      expect(screen.queryByText(/nicht gefunden/)).not.toBeInTheDocument();
    });
  }

  it("loads further pages until the row named by the hash appears", async () => {
    const { calls } = render({
      hash: "#ACT-051",
      answers: {
        "GET /actions": pagedList([
          [action({ id: "a1", local_id: "ACT-001" })],
          [action({ id: "a51", local_id: "ACT-051", title: "Zelt aufbauen" })],
        ]),
      },
    });

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    const row = await within(table).findByText("Zelt aufbauen");
    await waitFor(() => expect(row.closest("tr")).toHaveFocus());
    expect(
      calls.filter((c) => c.call === "GET /api/v1/events/" + EVENT.id + "/actions"),
    ).toHaveLength(2);
  });

  it("says in the live region that the row of the hash is on no page", async () => {
    render({
      hash: "#ACT-099",
      answers: {
        "GET /actions": pagedList([
          [action({ id: "a1", local_id: "ACT-001" })],
          [action({ id: "a2", local_id: "ACT-002" })],
        ]),
      },
    });

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("ACT-099 nicht gefunden"),
    );
    expect(screen.getByText("ACT-002")).toBeInTheDocument();
    expect(document.querySelector("[data-linked]")).toBeNull();
  });

  it("does not move focus to the row again when more rows load", async () => {
    render({
      hash: "#ACT-001",
      answers: {
        "GET /actions": pagedList([
          [action({ id: "a1", local_id: "ACT-001" })],
          [action({ id: "a2", local_id: "ACT-002" })],
          [action({ id: "a3", local_id: "ACT-003" })],
        ]),
      },
    });

    const row = (await screen.findByText("Generator bestellen")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
    const more = screen.getByRole("button", { name: "Weitere Einträge laden" });
    more.focus();
    await user.click(more);
    await screen.findByText("ACT-002");
    // The button stays, because a third page follows: focus stays on it.
    expect(screen.getByRole("button", { name: "Weitere Einträge laden" })).toHaveFocus();
    expect(row).not.toHaveFocus();
  });

  it("opens the row of the hash after a retry of the first load", async () => {
    let served = 0;
    render({
      hash: "#ACT-001",
      answers: {
        "GET /actions": () => {
          served += 1;
          return served === 1
            ? problem(500, "internal")
            : json(200, { items: [action()], next_cursor: null });
        },
      },
    });

    await user.click(await screen.findByRole("button", { name: "Erneut versuchen" }));
    const row = (await screen.findByText("Generator bestellen")).closest("tr");
    await waitFor(() => expect(row).toHaveAttribute("data-linked"));
    // The row that the address names wins focus over the heading that a retry moves it to.
    await waitFor(() => expect(row).toHaveFocus());
    // The heading would take focus after the next commit: let it pass first.
    await act(async () => {});
    expect(row).toHaveFocus();
    expect(screen.getByRole("heading", { level: 2, name: "Aufgaben" })).not.toHaveFocus();
    expect(screen.queryByText(/nicht gefunden/)).not.toBeInTheDocument();
  });

  it("does not focus the row again when a conflict loads the rows again", async () => {
    const { calls } = render({
      hash: "#ACT-001",
      answers: { "PATCH /actions/a1": () => problem(409, "record-version-conflict") },
      lists: { "/actions": [action()] },
    });
    const row = (await screen.findByText("Generator bestellen")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    await user.clear(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }));
    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Neu");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.filter((c) => c.call === `GET /api/v1/events/${EVENT.id}/actions`)).toHaveLength(
        2,
      ),
    );
    await act(async () => {});
    expect(screen.getByRole("heading", { name: "Aufgabe erfassen" })).toBeInTheDocument();
    expect(row).not.toHaveFocus();
  });

  it("focuses the new row when the hash changes on the mounted page", async () => {
    render({
      hash: "#ACT-002",
      lists: {
        "/actions": [
          action({ id: "a2", local_id: "ACT-002", title: "Zelt aufbauen" }),
          action({ id: "a3", local_id: "ACT-003", title: "Bar planen" }),
        ],
      },
    });
    const first = (await screen.findByText("Zelt aufbauen")).closest("tr");
    await waitFor(() => expect(first).toHaveFocus());

    act(() => {
      window.history.pushState(null, "", `${window.location.pathname}#ACT-003`);
      window.dispatchEvent(new PopStateEvent("popstate"));
    });

    const second = screen.getByText("Bar planen").closest("tr");
    await waitFor(() => expect(second).toHaveFocus());
    expect(second).toHaveAttribute("data-linked");
  });

  it("shows the edit control where the server says the caller can change the action", async () => {
    render({
      lists: {
        "/actions": [
          action({ id: "a1", local_id: "ACT-001" }),
          // An owner who became a viewer: the server refuses, so the page offers no edit.
          action({ id: "a2", local_id: "ACT-002", can_change: false, next_statuses: [] }),
        ],
      },
    });

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.getByRole("button", { name: "ACT-001 bearbeiten" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "ACT-002 bearbeiten" })).not.toBeInTheDocument();
  });

  it("the action form offers only allowed statuses", async () => {
    render({ lists: { "/actions": [action({ status: "done", next_statuses: ["open"] })] } });
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

  it("offers the contributors and managers of the event as owners to a contributor", async () => {
    render({ lists: { "/actions": [] } });
    await screen.findByText("Noch keine Aufgaben");

    await user.click(screen.getByRole("button", { name: /Verantwortlich/ }));

    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["Anna Muster", "Bernd Beispiel"]);
  });

  it("shows entfernt for a redacted action and offers no edit", async () => {
    render({
      lists: { "/actions": [action({ title: "", redacted: true })] },
    });

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    expect(within(table).getByText("entfernt")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "ACT-001 bearbeiten" })).not.toBeInTheDocument();
  });

  it("sends the start date and the milestone of a new action", async () => {
    const created = action({ id: "a2", local_id: "ACT-002", title: "Zelt aufbauen" });
    const { calls } = render({
      answers: { "POST /actions": () => json(201, created) },
      lists: {
        "/actions": [],
        "/milestones": [
          milestone(),
          milestone({ id: "m2", local_id: "MIL-002", title: "Abgesagt", status: "canceled" }),
        ],
      },
    });
    await screen.findByText("Noch keine Aufgaben");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Zelt aufbauen");
    await user.type(screen.getByLabelText("Startdatum"), "2030-05-01");
    await user.type(screen.getByLabelText("Fällig am"), "2030-05-18");
    await user.click(screen.getByRole("combobox", { name: "Meilenstein" }));
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    // A canceled milestone takes no new action.
    expect(options.map((option) => option.textContent)).toEqual([
      "Kein Meilenstein",
      "MIL-001 Festzelt steht",
    ]);
    await user.click(screen.getByRole("option", { name: "MIL-001 Festzelt steht" }));
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("ACT-002")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST"))?.body).toEqual({
      title: "Zelt aufbauen",
      owner_user_id: ME,
      start_date: "2030-05-01",
      due_date: "2030-05-18",
      milestone_id: "m1",
    });
  });

  it("searches the milestones and keeps the canceled milestone of the action", async () => {
    render({
      lists: {
        "/actions": [action({ milestone_id: "m2" })],
        "/milestones": [
          milestone(),
          milestone({ id: "m2", local_id: "MIL-002", title: "Abgesagt", status: "canceled" }),
          milestone({ id: "m3", local_id: "MIL-003", title: "Bühne steht" }),
        ],
      },
    });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    const input = screen.getByRole("combobox", { name: "Meilenstein" });
    expect(input).toHaveValue("MIL-002 Abgesagt");
    await user.clear(input);
    await user.type(input, "bühne");

    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["MIL-003 Bühne steht"]);
  });

  it("clears the milestone and the start date of an action", async () => {
    const { calls } = render({
      answers: { "PATCH /actions/a1": () => json(200, action({ version: 3 })) },
      lists: {
        "/actions": [action({ start_date: "2030-05-01", milestone_id: "m1" })],
        "/milestones": [milestone()],
      },
    });
    await screen.findByRole("table", { name: "Aufgaben" });

    await user.click(screen.getByRole("button", { name: "ACT-001 bearbeiten" }));
    expect(screen.getByLabelText("Startdatum")).toHaveValue("2030-05-01");
    await user.clear(screen.getByLabelText("Startdatum"));
    await user.click(screen.getByRole("combobox", { name: "Meilenstein" }));
    await user.click(await screen.findByRole("option", { name: "Kein Meilenstein" }));
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call.startsWith("PATCH"))?.body).toEqual({
        start_date: null,
        milestone_id: null,
        expected_version: 2,
      }),
    );
  });

  it("shows the start-after-due error at the start date", async () => {
    render({
      answers: {
        "POST /actions": () =>
          problem(422, "validation-failed", [{ pointer: "/start_date", code: "start-after-due" }]),
      },
      lists: { "/actions": [] },
    });
    await screen.findByText("Noch keine Aufgaben");

    await user.type(screen.getByRole("textbox", { name: "Titel (Pflichtfeld)" }), "Zelt");
    await user.type(screen.getByLabelText("Startdatum"), "2030-06-01");
    await user.type(screen.getByLabelText("Fällig am"), "2030-05-18");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    const start = screen.getByLabelText("Startdatum");
    await waitFor(() => expect(start).toHaveAccessibleDescription(/Das Startdatum liegt nach/));
    expect(start).toHaveFocus();
  });
});
