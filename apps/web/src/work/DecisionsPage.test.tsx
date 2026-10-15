import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { EVENT, json, ME, problem, renderWork } from "../test/fakeWorkServer";
import { DecisionsPage } from "./DecisionsPage";

const user = userEvent.setup({ delay: null });
// The party search waits 250 ms after the last key, then fetches persons and institutions.
const SEARCH_TIMEOUT = { timeout: 3000 };

const generators = { id: "i1", local_id: "INS-001", name: "Testwil Generatoren AG", version: 1 };
const clara = { id: "p1", local_id: "PER-001", name: "Cäcilia Probst", version: 1 };

const WORDING = "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK am Vortag.";

function decision(over: Record<string, unknown> = {}) {
  return {
    id: "d1",
    local_id: "DEC-001",
    event_id: EVENT.id,
    wording: WORDING,
    redacted: false,
    decided_on: "2030-03-12",
    decided_by: [
      {
        kind: "institution",
        id: "i1",
        local_id: "INS-001",
        name: generators.name,
        redacted: false,
      },
    ],
    owner_user_id: ME,
    workstream_id: "w1",
    status: "in-force",
    version: 2,
    evidence: [],
    can_change: true,
    can_end: true,
    ...over,
  };
}

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <DecisionsPage api={api} />, {
    eventManager: true,
    ...setup,
    lists: { "/persons": [clara], "/institutions": [generators], ...setup.lists },
  });

afterEach(() => window.history.replaceState(null, "", "/"));

describe("DecisionsPage", () => {
  it("lists decisions with date, parties, owner, workstream and status", async () => {
    render({ lists: { "/decisions": [decision()] } });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    const row = within(table).getByText("DEC-001").closest("tr") as HTMLElement;
    expect(within(row).getByText(/Das Fest findet bei jedem Wetter statt/)).toBeInTheDocument();
    expect(within(row).getByText("12.03.2030")).toBeInTheDocument();
    expect(within(row).getByText("Testwil Generatoren AG")).toBeInTheDocument();
    expect(within(row).getByText("Anna Muster")).toBeInTheDocument();
    expect(within(row).getByText("Bodenbetrieb")).toBeInTheDocument();
    expect(within(row).getByText("in Kraft")).toBeInTheDocument();
  });

  it("shortens a long wording and shows all of it on request", async () => {
    const long = `${"Sehr lange Entscheidung ".repeat(20)}Ende`;
    render({ lists: { "/decisions": [decision({ wording: long })] } });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    expect(within(table).queryByText(/Ende$/)).not.toBeInTheDocument();
    await user.click(
      within(table).getByRole("button", { name: "Wortlaut von DEC-001 ganz zeigen" }),
    );
    expect(within(table).getByText(/Ende$/)).toBeInTheDocument();
    await user.click(within(table).getByRole("button", { name: "Wortlaut von DEC-001 kürzen" }));
    expect(within(table).queryByText(/Ende$/)).not.toBeInTheDocument();
  });

  it("shows the end of a decision with its link or its reason", async () => {
    render({
      lists: {
        "/decisions": [
          decision({ status: "superseded", superseded_by: "d2", can_end: false }),
          decision({ id: "d2", local_id: "DEC-002", wording: "Neu" }),
          decision({
            id: "d3",
            local_id: "DEC-003",
            status: "revoked",
            revoked_reason: "Die Gemeinde verbietet das Feuerwerk.",
            can_end: false,
          }),
        ],
      },
    });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    const first = within(table).getByText("DEC-001").closest("tr") as HTMLElement;
    expect(within(first).getByText("ersetzt")).toBeInTheDocument();
    expect(within(first).getByRole("link", { name: "DEC-002" })).toHaveAttribute(
      "href",
      "#DEC-002",
    );
    const third = within(table).getByText("DEC-003").closest("tr") as HTMLElement;
    expect(within(third).getByText("widerrufen")).toBeInTheDocument();
    expect(within(third).getByText(/Die Gemeinde verbietet das Feuerwerk/)).toBeInTheDocument();
    expect(within(first).queryByRole("button", { name: /ersetzen|widerrufen/ })).toBeNull();
  });

  it("opens the decision named by the hash of the address and focuses it", async () => {
    render({
      hash: "#DEC-002",
      lists: { "/decisions": [decision(), decision({ id: "d2", local_id: "DEC-002" })] },
    });

    const row = (await screen.findByText("DEC-002")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
  });

  it("creates a decision with a multi-line wording and two parties", async () => {
    const created = decision({ id: "d9", local_id: "DEC-009" });
    const { calls } = render({
      answers: { "POST /decisions": () => json(201, created) },
      lists: { "/decisions": [] },
    });
    await screen.findByText("Noch keine Entscheide");

    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    expect(wording.tagName).toBe("TEXTAREA");
    await user.type(wording, "Erste Zeile{Enter}Zweite Zeile");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-03-12");
    const parties = screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" });
    await user.type(parties, "Testwil");
    await user.click(
      await screen.findByRole(
        "option",
        { name: "Testwil Generatoren AG (INS-001)" },
        SEARCH_TIMEOUT,
      ),
    );
    await user.type(parties, "Cäcilia");
    await user.click(
      await screen.findByRole("option", { name: "Cäcilia Probst (PER-001)" }, SEARCH_TIMEOUT),
    );
    // The picker is empty and closed after each choice.
    expect(parties).toHaveValue("");
    expect(parties).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("list", { name: "Beschlossen von" }).children).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("DEC-009")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST /api/v1/events"))?.body).toEqual({
      wording: "Erste Zeile\nZweite Zeile",
      decided_on: "2030-03-12",
      decided_by: [
        { kind: "institution", id: "i1" },
        { kind: "person", id: "p1" },
      ],
      owner_user_id: ME,
    });
  });

  it("asks for a wording, a date and a party before it sends", async () => {
    const { calls } = render({ lists: { "/decisions": [] } });
    await screen.findByText("Noch keine Entscheide");

    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("Der Wortlaut hat 1 bis 2000 Zeichen.")).toBeInTheDocument();
    expect(screen.getByText("Geben Sie ein gültiges Datum an.")).toBeInTheDocument();
    expect(
      screen.getByText("Wählen Sie 1 bis 50 Personen oder Institutionen."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });

  it("shows the refusals of the server at the fields", async () => {
    render({
      answers: {
        "POST /decisions": () =>
          problem(422, "validation-failed", [
            { pointer: "/wording", code: "too-long" },
            { pointer: "/decided_by/0", code: "unknown-record" },
          ]),
      },
      lists: { "/decisions": [] },
    });
    await screen.findByText("Noch keine Entscheide");
    await user.type(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" }), "Text");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-03-12");
    await user.type(
      screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" }),
      "Test",
    );
    await user.click(
      await screen.findByRole(
        "option",
        { name: "Testwil Generatoren AG (INS-001)" },
        SEARCH_TIMEOUT,
      ),
    );
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("Der Wortlaut hat höchstens 2000 Zeichen.")).toBeInTheDocument();
    expect(screen.getByText("Diese Person oder Institution gibt es nicht.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveFocus();
  });

  it("has no edit of the wording", async () => {
    render({ lists: { "/decisions": [decision()] } });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).toBeNull();
    // The row shows the wording as text only; the page has no field that holds it.
    expect(within(table).queryByRole("textbox")).toBeNull();
  });

  it("supersedes a decision with one request and the prefilled wording", async () => {
    const successor = decision({ id: "d2", local_id: "DEC-002", wording: "Neuer Wortlaut" });
    let listed = [decision()];
    const { calls } = render({
      answers: {
        "POST /supersede": () => {
          listed = [
            decision({ status: "superseded", superseded_by: "d2", can_end: false, version: 3 }),
            successor,
          ];
          return json(201, successor);
        },
        "GET /decisions": () => json(200, { items: listed }),
      },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));

    expect(await screen.findByRole("heading", { name: "DEC-001 ersetzen" })).toHaveFocus();
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    expect(wording).toHaveValue(WORDING);
    expect(screen.getByRole("list", { name: "Beschlossen von" })).toHaveTextContent(
      "Testwil Generatoren AG",
    );
    await user.clear(wording);
    await user.type(wording, "Neuer Wortlaut");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    const table = await screen.findByRole("table", { name: "Entscheide" });
    await waitFor(() => expect(within(table).getByText("ersetzt")).toBeInTheDocument());
    const requests = calls.filter((c) => c.call.startsWith("POST"));
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({
      call: "POST /api/v1/events/0199b8e0-0000-7000-8000-000000000001/decisions/d1/supersede",
      body: {
        expected_version: 2,
        successor: {
          wording: "Neuer Wortlaut",
          decided_on: "2030-04-01",
          decided_by: [{ kind: "institution", id: "i1" }],
          owner_user_id: ME,
          workstream_id: "w1",
        },
      },
    });
    expect(screen.getByRole("heading", { name: "Entscheid erfassen" })).toBeInTheDocument();
  });

  it("maps the refusals of a successor to the fields of the form", async () => {
    render({
      answers: {
        "POST /supersede": () =>
          problem(422, "validation-failed", [{ pointer: "/successor/wording", code: "required" }]),
      },
      lists: { "/decisions": [decision()] },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    expect(await screen.findByText("Der Wortlaut hat 1 bis 2000 Zeichen.")).toBeInTheDocument();
  });

  it("keeps the typed decision when somebody changed the replaced one meanwhile", async () => {
    let version = 2;
    let refused = false;
    const { calls } = render({
      answers: {
        "POST /supersede": () => {
          if (!refused) {
            refused = true;
            version = 3;
            return problem(409, "record-version-conflict");
          }
          return json(201, decision({ id: "d2", local_id: "DEC-002" }));
        },
        "GET /decisions/d1": () => json(200, decision({ version })),
        "GET /decisions": () => json(200, { items: [decision({ version })] }),
      },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    await user.clear(wording);
    await user.type(wording, "Neuer Wortlaut");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).toHaveFocus();
    expect(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveValue(
      "Neuer Wortlaut",
    );
    expect(screen.getByLabelText("Beschlossen am (Pflichtfeld)")).toHaveValue("2030-04-01");

    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Entscheid erfassen" })).toBeInTheDocument(),
    );
    const bodies = calls.filter((c) => c.call.endsWith("/supersede")).map((c) => c.body);
    expect(bodies).toMatchObject([{ expected_version: 2 }, { expected_version: 3 }]);
  });

  it("takes the reloaded row when the lookup throws before the list is back", async () => {
    let version = 2;
    let refused = false;
    const { calls } = render({
      answers: {
        "POST /supersede": () => {
          if (!refused) {
            refused = true;
            version = 3;
            return problem(409, "record-version-conflict");
          }
          return json(201, decision({ id: "d2", local_id: "DEC-002" }));
        },
        "GET /decisions/d1": () => {
          throw new TypeError("network");
        },
        "GET /decisions": async () => {
          if (version === 3) {
            await new Promise((resolve) => setTimeout(resolve, 50));
          }
          return json(200, { items: [decision({ version })] });
        },
      },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    await user.clear(wording);
    await user.type(wording, "Neuer Wortlaut");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveValue(
      "Neuer Wortlaut",
    );
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Entscheid erfassen" })).toBeInTheDocument(),
    );
    const bodies = calls.filter((c) => c.call.endsWith("/supersede")).map((c) => c.body);
    expect(bodies).toMatchObject([{ expected_version: 2 }, { expected_version: 3 }]);
  });

  it("keeps the text and says so when the replaced decision is no longer in force", async () => {
    const { calls } = render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": () =>
          json(200, decision({ status: "revoked", can_end: false, version: 3 })),
      },
      lists: { "/decisions": [decision()] },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    await user.clear(wording);
    await user.type(wording, "Neuer Wortlaut");
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "DEC-001 lässt sich nicht mehr ersetzen. Kopieren Sie den Text, und brechen Sie dann ab.",
    );
    // The reason stays at the form while the button is off, also after another action.
    expect(screen.getByText("DEC-001 lässt sich nicht mehr ersetzen.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveValue(
      "Neuer Wortlaut",
    );
    expect(screen.getByRole("button", { name: "Ersetzen" })).toBeDisabled();
    expect(calls.filter((c) => c.call.endsWith("/supersede"))).toHaveLength(1);
  });

  /** An answer that waits until the test lets it go. */
  function gate() {
    let open: () => void = () => undefined;
    const opened = new Promise<void>((resolve) => {
      open = resolve;
    });
    return { opened, open };
  }

  async function sendReplacement() {
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
  }

  it("keeps the form busy while it loads the changed decision again", async () => {
    const wait = gate();
    let version = 2;
    const { calls } = render({
      answers: {
        "POST /supersede": () => {
          if (version === 2) {
            version = 3;
            return problem(409, "record-version-conflict");
          }
          return json(201, decision({ id: "d9", local_id: "DEC-009" }));
        },
        "GET /decisions/d1": async () => {
          await wait.opened;
          return json(200, decision({ version }));
        },
      },
      lists: { "/decisions": [decision()] },
    });
    await sendReplacement();
    await waitFor(() => expect(calls.some((c) => c.call.endsWith("/decisions/d1"))).toBe(true));

    // The window of the reload: nothing of the form works, so no late answer meets a changed form.
    expect(screen.getByRole("button", { name: "Abbrechen" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Abbrechen" }));
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
    expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).toBeInTheDocument();
    expect(calls.filter((c) => c.call.endsWith("/supersede"))).toHaveLength(1);

    wait.open();
    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));
    await waitFor(() => expect(calls.filter((c) => c.call.endsWith("/supersede"))).toHaveLength(2));
    expect(calls.filter((c) => c.call.endsWith("/supersede")).map((c) => c.body)).toMatchObject([
      { expected_version: 2 },
      { expected_version: 3 },
    ]);
  });

  it("ignores the late decision when the member opened another replacement meanwhile", async () => {
    const wait = gate();
    render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": async () => {
          await wait.opened;
          return json(200, decision({ version: 3 }));
        },
      },
      lists: {
        "/decisions": [decision(), decision({ id: "d2", local_id: "DEC-002", version: 1 })],
      },
    });
    await sendReplacement();
    await user.click(await screen.findByRole("button", { name: "DEC-002 ersetzen" }));
    expect(screen.getByRole("heading", { name: "DEC-002 ersetzen" })).toBeInTheDocument();

    wait.open();
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen.",
      ),
    );
    expect(screen.getByRole("heading", { name: "DEC-002 ersetzen" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "DEC-001 ersetzen" })).toBeNull();
  });

  it.each([
    ["the server refuses it", () => problem(500, "internal")],
    ["the network is gone", () => Promise.reject(new TypeError("network"))],
  ])("blocks the send until the changed decision loaded, when %s", async (_name, answer) => {
    const { calls } = render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": answer,
      },
      lists: { "/decisions": [decision()] },
    });
    await sendReplacement();

    // The version that the server refused is still the one in the form: it asks to copy the text.
    const note = await screen.findByText(/Die Fassung von DEC-001 ist nicht mehr aktuell/);
    expect(note).toHaveTextContent("Kopieren Sie den Text");
    expect(screen.getByRole("alert")).toHaveTextContent(/\S/);
    const heading = screen.getByRole("heading", { name: "DEC-001 ersetzen" });
    await waitFor(() => expect(heading).toHaveFocus());
    const send = screen.getByRole("button", { name: "Ersetzen" });
    expect(send).toBeDisabled();
    expect(send).not.toHaveFocus();
    expect(calls.filter((c) => c.call.endsWith("/supersede"))).toHaveLength(1);
  });

  it("never disables the button that has focus while the lookup of the changed decision runs", async () => {
    const wait = gate();
    render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": async () => {
          await wait.opened;
          return problem(500, "internal");
        },
      },
      lists: { "/decisions": [decision()] },
    });
    await sendReplacement();
    const send = screen.getByRole("button", { name: "Ersetzen" });
    // The 409 is in, the lookup runs: the form is busy, and the button keeps focus and is not off.
    await waitFor(() => expect(send).toHaveAttribute("aria-disabled", "true"));
    expect(send).not.toBeDisabled();
    expect(send).toHaveFocus();

    wait.open();
    const heading = screen.getByRole("heading", { name: "DEC-001 ersetzen" });
    await waitFor(() => expect(heading).toHaveFocus());
    expect(document.activeElement).not.toBeDisabled();
  });

  it("falls back to the reloaded row when the lookup fails, and keeps the typed text", async () => {
    let version = 2;
    const { calls } = render({
      answers: {
        "POST /supersede": () => {
          if (version === 2) {
            version = 3;
            return problem(409, "record-version-conflict");
          }
          return json(201, decision({ id: "d9", local_id: "DEC-009" }));
        },
        "GET /decisions/d1": () => problem(500, "internal"),
        "GET /decisions": () => json(200, { items: [decision({ version })] }),
      },
    });
    await sendReplacement();

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    const heading = screen.getByRole("heading", { name: "DEC-001 ersetzen" });
    await waitFor(() => expect(heading).toHaveFocus());
    expect(screen.getByLabelText("Beschlossen am (Pflichtfeld)")).toHaveValue("2030-04-01");
    const send = screen.getByRole("button", { name: "Ersetzen" });
    expect(send).toBeEnabled();
    await user.click(send);
    await waitFor(() => expect(calls.filter((c) => c.call.endsWith("/supersede"))).toHaveLength(2));
    expect(calls.filter((c) => c.call.endsWith("/supersede")).map((c) => c.body)).toMatchObject([
      { expected_version: 2 },
      { expected_version: 3 },
    ]);
  });

  it("does not let a late answer interrupt the typing in a reopened form of the same decision", async () => {
    const wait = gate();
    render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": async () => {
          await wait.opened;
          return json(200, decision({ version: 3 }));
        },
      },
      lists: {
        "/decisions": [decision(), decision({ id: "d2", local_id: "DEC-002", version: 1 })],
      },
    });
    await sendReplacement();
    await user.click(await screen.findByRole("button", { name: "DEC-002 ersetzen" }));
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    await user.click(wording);
    await user.keyboard("!");

    wait.open();
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(/\S/));
    expect(wording).toHaveFocus();
    expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).not.toHaveFocus();
  });

  it("keeps the replace form when the revoke of another row conflicts", async () => {
    render({
      answers: { "POST /revoke": () => problem(409, "record-version-conflict") },
      lists: {
        "/decisions": [decision(), decision({ id: "d2", local_id: "DEC-002", version: 1 })],
      },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    const wording = screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" });
    await user.clear(wording);
    await user.type(wording, "Neuer Wortlaut");
    await user.click(screen.getByRole("button", { name: "DEC-002 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }), "Grund");
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" })).toHaveValue(
      "Neuer Wortlaut",
    );
  });

  it("does not clear the polite region when Enter arrives while the form is busy", async () => {
    const wait = gate();
    render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": async () => {
          await wait.opened;
          return json(200, decision({ version: 3 }));
        },
      },
      lists: { "/decisions": [decision()] },
    });
    await sendReplacement();
    await user.type(
      screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" }),
      "Cäcilia",
    );
    await user.click(await screen.findByRole("option", { name: /Cäcilia Probst/ }, SEARCH_TIMEOUT));
    const note = await screen.findByText("Cäcilia Probst (PER-001) hinzugefügt");
    // An Enter in a field submits the form, also while the send runs.
    fireEvent.submit(screen.getByRole("button", { name: "Ersetzen" }).closest("form") as Element);

    expect(note).toHaveTextContent("Cäcilia Probst (PER-001) hinzugefügt");
    // The end of the failed send is part of the test: the form is free again before it ends.
    wait.open();
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "DEC-001 ersetzen" })).toHaveFocus(),
    );
  });

  it("blocks the send when the member may no longer end the replaced decision", async () => {
    render({
      answers: {
        "POST /supersede": () => problem(409, "record-version-conflict"),
        "GET /decisions/d1": () => json(200, decision({ can_end: false, version: 3 })),
      },
      lists: {
        "/decisions": [decision(), decision({ id: "d2", local_id: "DEC-002", version: 1 })],
      },
    });
    await sendReplacement();

    expect(await screen.findByRole("alert")).toHaveTextContent(/lässt sich nicht mehr ersetzen/);
    expect(screen.getByRole("button", { name: "Ersetzen" })).toBeDisabled();
    // Another action clears the message; the reason stays at the form.
    await user.click(screen.getByRole("button", { name: "DEC-002 widerrufen" }));
    expect(screen.getByText("DEC-001 lässt sich nicht mehr ersetzen.")).toBeInTheDocument();
    expect(screen.queryByText(/Der neue Entscheid ersetzt DEC-001/)).toBeNull();
  });

  it.each([
    ["a mouse choice", false, "Testwil Generatoren AG (INS-001)"],
    ["a keyboard choice", true, "Cäcilia Probst (PER-001)"],
  ])(
    "says a party was added and removed in the polite region, with %s",
    async (_name, keyboard, party) => {
      render({ lists: { "/decisions": [] } });
      await screen.findByText("Noch keine Entscheide");
      const search = screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" });
      await user.type(search, keyboard ? "C" : "Testwil");
      const option = await screen.findByRole("option", { name: party }, SEARCH_TIMEOUT);
      if (keyboard) {
        await user.keyboard("{ArrowDown}{Enter}");
      } else {
        await user.click(option);
      }
      const added = await screen.findByText(`${party} hinzugefügt`);
      expect(added).toHaveAttribute("role", "status");

      await user.click(screen.getByRole("button", { name: `${party} entfernen` }));
      const removed = await screen.findByText(`${party} entfernt`);
      expect(removed).toHaveAttribute("role", "status");
    },
  );

  it("names a successor that is not in the loaded rows", async () => {
    render({
      answers: {
        "GET /decisions/d9": () => json(200, decision({ id: "d9", local_id: "DEC-051" })),
      },
      lists: {
        "/decisions": [decision({ status: "superseded", superseded_by: "d9", can_end: false })],
      },
    });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    expect(await within(table).findByRole("link", { name: "DEC-051" })).toHaveAttribute(
      "href",
      "#DEC-051",
    );
  });

  it("counts the characters of the trimmed wording like the server", async () => {
    const { calls } = render({
      answers: { "POST /decisions": () => json(201, decision({ id: "d9", local_id: "DEC-009" })) },
      lists: { "/decisions": [] },
    });
    await screen.findByText("Noch keine Entscheide");
    // 1500 emoji are 3000 UTF-16 units but 1500 characters; the spaces around do not count.
    await user.click(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" }));
    await user.paste(`  ${"😀".repeat(1500)}  `);
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-03-12");
    await user.type(
      screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" }),
      "Testwil",
    );
    await user.click(
      await screen.findByRole("option", { name: /Testwil Generatoren AG/ }, SEARCH_TIMEOUT),
    );
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    await screen.findByText("DEC-009");
    expect(calls.some((c) => c.call.startsWith("POST /api/v1/events"))).toBe(true);
  });

  it("refuses a control character of the wording with its own message", async () => {
    const { calls } = render({ lists: { "/decisions": [] } });
    await screen.findByText("Noch keine Entscheide");
    await user.click(screen.getByRole("textbox", { name: "Wortlaut (Pflichtfeld)" }));
    await user.paste("Text\u000Bmit Steuerzeichen");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(
      await screen.findByText("Der Wortlaut enthält ein Zeichen, das nicht erlaubt ist."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });

  it("shows the closed workstream of a successor at the workstream field", async () => {
    render({
      answers: {
        "POST /supersede": () =>
          problem(422, "validation-failed", [
            { pointer: "/successor/workstream_id", code: "closed" },
          ]),
      },
      lists: { "/decisions": [decision()] },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));
    await user.type(screen.getByLabelText("Beschlossen am (Pflichtfeld)"), "2030-04-01");
    await user.click(screen.getByRole("button", { name: "Ersetzen" }));

    expect(await screen.findByText("Dieser Arbeitsbereich ist geschlossen.")).toBeInTheDocument();
  });

  it("removes a party with a text button", async () => {
    render({ lists: { "/decisions": [] } });
    await screen.findByText("Noch keine Entscheide");
    const parties = screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" });
    await user.type(parties, "Testwil");
    await user.click(
      await screen.findByRole("option", { name: /Testwil Generatoren AG/ }, SEARCH_TIMEOUT),
    );

    const list = screen.getByRole("list", { name: "Beschlossen von" });
    const remove = within(list).getByRole("button", {
      name: "Testwil Generatoren AG (INS-001) entfernen",
    });
    expect(remove).toHaveTextContent("Entfernen");
    await user.click(remove);
    expect(screen.queryByRole("list", { name: "Beschlossen von" })).toBeNull();
    expect(parties).toHaveFocus();
  });

  it("does not offer a party again that the list already holds", async () => {
    render({ lists: { "/decisions": [] } });
    await screen.findByText("Noch keine Entscheide");
    const parties = screen.getByRole("combobox", { name: "Beschlossen von (Pflichtfeld)" });
    await user.type(parties, "Testwil");
    await user.click(
      await screen.findByRole("option", { name: /Testwil Generatoren AG/ }, SEARCH_TIMEOUT),
    );
    await user.type(parties, "e");

    expect(
      await screen.findByRole("option", { name: /Cäcilia Probst/ }, SEARCH_TIMEOUT),
    ).toBeVisible();
    expect(screen.queryByRole("option", { name: /Testwil Generatoren AG/ })).toBeNull();
  });

  it("shows Ersetzen with its form when the server allows it to a non-manager", async () => {
    render({ eventManager: false, lists: { "/decisions": [decision()] } });
    await user.click(await screen.findByRole("button", { name: "DEC-001 ersetzen" }));

    expect(await screen.findByRole("heading", { name: "DEC-001 ersetzen" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "Ersetzen" })).toBeInTheDocument();
  });

  it("revokes only with a reason", async () => {
    const { calls } = render({
      answers: {
        "POST /revoke": () =>
          json(
            200,
            decision({
              status: "revoked",
              revoked_reason: "Die Gemeinde verbietet das Feuerwerk.",
              can_end: false,
              version: 3,
            }),
          ),
      },
      lists: { "/decisions": [decision()] },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog", { name: "DEC-001 widerrufen" });

    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));
    expect(
      within(dialog).getByText("Geben Sie einen Grund an. Er hat 1 bis 500 Zeichen."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.endsWith("/revoke"))).toBe(false);

    await user.type(
      within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }),
      "Die Gemeinde verbietet das Feuerwerk.",
    );
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(calls.find((c) => c.call.endsWith("/revoke"))?.body).toEqual({
      reason: "Die Gemeinde verbietet das Feuerwerk.",
      expected_version: 2,
    });
    expect(screen.getByText("widerrufen")).toBeInTheDocument();
    expect(screen.getByText("DEC-001 ist jetzt widerrufen.")).toBeInTheDocument();
  });

  it("closes the revoke dialog with Escape and returns focus to its button", async () => {
    const { calls } = render({ lists: { "/decisions": [decision()] } });
    const button = await screen.findByRole("button", { name: "DEC-001 widerrufen" });
    await user.click(button);
    await screen.findByRole("dialog", { name: "DEC-001 widerrufen" });

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(button).toHaveFocus();
    expect(calls.some((c) => c.call.endsWith("/revoke"))).toBe(false);
  });

  it("takes the reason as one line and refuses a control character", async () => {
    const { calls } = render({ lists: { "/decisions": [decision()] } });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    const reason = within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" });
    expect(reason.tagName).toBe("INPUT");

    await user.click(reason);
    await user.paste("Erste Zeile\nZweite\r\n Zeile\u000Bnoch");
    expect(reason).toHaveValue("Erste Zeile Zweite Zeile\u000Bnoch");
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    expect(
      await within(dialog).findByText("Der Grund enthält ein Zeichen, das nicht erlaubt ist."),
    ).toBeInTheDocument();
    expect(calls.some((c) => c.call.endsWith("/revoke"))).toBe(false);
  });

  it("keeps the caret after a paste with line breaks in the middle of the reason", async () => {
    render({ lists: { "/decisions": [decision()] } });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    const reason = within(dialog).getByRole("textbox", {
      name: "Grund (Pflichtfeld)",
    }) as HTMLInputElement;
    await user.type(reason, "Anfang Ende");
    reason.setSelectionRange(7, 7);
    await user.paste("a\nb");

    expect(reason).toHaveValue("Anfang a bEnde");
    expect(reason.selectionStart).toBe(10);
    expect(reason.selectionEnd).toBe(10);
  });

  it("drops the caret target when a paste leaves the reason as it was", async () => {
    render({ lists: { "/decisions": [decision()] } });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    const reason = within(dialog).getByRole("textbox", {
      name: "Grund (Pflichtfeld)",
    }) as HTMLInputElement;
    await user.type(reason, "Ab c");
    reason.setSelectionRange(2, 4);
    await user.paste("\nc");
    expect(reason).toHaveValue("Ab c");

    reason.setSelectionRange(0, 0);
    await user.keyboard("X");
    expect(reason).toHaveValue("XAb c");
    expect(reason.selectionStart).toBe(1);
  });

  it("reloads the list when somebody changed the decision before the revoke", async () => {
    let loads = 0;
    render({
      answers: {
        "POST /revoke": () => problem(409, "record-version-conflict"),
        "GET /decisions": () => {
          loads += 1;
          return json(200, { items: [decision({ version: 1 + loads })] });
        },
      },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }), "Grund");
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    expect(
      await screen.findByText("Der Eintrag wurde inzwischen geändert. Die Liste ist neu geladen."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(loads).toBe(2));
  });

  it("shows the standard message when the server forbids a write", async () => {
    render({
      answers: { "POST /revoke": () => problem(403, "forbidden") },
      lists: { "/decisions": [decision()] },
    });
    await user.click(await screen.findByRole("button", { name: "DEC-001 widerrufen" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Grund (Pflichtfeld)" }), "Grund");
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    expect(
      await screen.findByText("Sie haben keine Berechtigung für diese Aktion."),
    ).toBeInTheDocument();
  });

  it("shows a contributor the decisions and the evidence but no write action", async () => {
    render({
      eventManager: false,
      lists: {
        "/decisions": [
          decision({
            can_change: false,
            can_end: false,
            evidence: [
              {
                record_version: 1,
                proposal_id: "p1",
                source_version_id: "s1",
                captured_at: "2030-03-12T10:00:00Z",
                start_offset: 0,
                end_offset: 10,
                quote: "Beschluss im Protokoll",
                redacted: false,
              },
            ],
          }),
        ],
      },
    });

    const table = await screen.findByRole("table", { name: "Entscheide" });
    expect(within(table).getByRole("button", { name: "Beleg zu DEC-001" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /ersetzen|widerrufen|Erfassen/ })).toBeNull();
    expect(screen.queryByRole("textbox", { name: /Wortlaut/ })).toBeNull();
  });
});
