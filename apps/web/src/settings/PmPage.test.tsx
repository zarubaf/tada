import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { type AiStatus, createApi, type Digest, type PmPolicy } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { PmPage } from "./PmPage";

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: unknown[]) {
  return json(
    status,
    { type: "", code, title: "", status, instance: "", request_id: "r1", errors },
    "application/problem+json",
  );
}

const policy: PmPolicy = {
  local_time: "08:30:00",
  time_zone: "Europe/Zurich",
  weekdays: 0b001_1111,
  quiet_start: "20:00:00",
  quiet_end: "08:00:00",
  cooldown_hours: 20,
  follow_up_working_days: 2,
  look_ahead_working_days: 2,
  paused: false,
  ai_enabled: false,
  version: 3,
};

const ai: AiStatus = {
  enabled: false,
  configured: true,
  cap_chf: "25.00",
  used_chf: "3.20",
  version: 1,
};

const digest: Digest = {
  id: "0199b8e0-0000-7000-8000-0000000000d1",
  user_display_name: "Anna Muster",
  created_at: "2030-05-13T06:30:00Z",
  sent_at: null,
  status: "unknown",
  item_count: 3,
};

type Answers = Record<string, (() => Response)[]>;

/** Answers each request from the queue of its method and path; a GET without a queue repeats. */
function setup(answers: Answers, role = "admin") {
  const calls: { call: string; body: unknown }[] = [];
  const defaults: Record<string, () => Response> = {
    "GET /api/v1/organization/pm-policy": () => json(200, policy),
    "GET /api/v1/organization/ai": () => json(200, ai),
    "GET /api/v1/organization/digests": () => json(200, { items: [digest] }),
  };
  const fetch = async (request: Request) => {
    const path = new URL(request.url).pathname;
    if (path === "/api/v1/session") {
      const membership = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
      return json(200, {
        user_id: "u1",
        display_name: "Olga Owner",
        organization: membership,
        memberships: [membership],
      });
    }
    const call = `${request.method} ${path}`;
    const text = request.method === "POST" ? await request.clone().text() : "";
    calls.push({ call, body: text === "" ? undefined : JSON.parse(text) });
    const answer = answers[call]?.shift() ?? defaults[call];
    if (!answer) {
      throw new Error(`no answer for ${call}`);
    }
    return answer();
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  render(
    <Router>
      <SessionProvider api={api}>
        <PmPage api={api} />
      </SessionProvider>
    </Router>,
  );
  return calls;
}

const user = userEvent.setup({ delay: null });
const cooldown = () => screen.findByRole("textbox", { name: "Abklingzeit (Stunden)" });
const save = () => user.click(screen.getByRole("button", { name: "Einstellungen speichern" }));
const posts = (calls: { call: string; body: unknown }[]) =>
  calls.filter((call) => call.call.startsWith("POST"));

describe("PmPage", () => {
  it("saves the policy with the expected version", async () => {
    const calls = setup({
      "POST /api/v1/organization/pm-policy/set": [
        () => json(200, { ...policy, cooldown_hours: 24, local_time: "07:15:00", version: 4 }),
      ],
    });

    await user.clear(await cooldown());
    await user.type(await cooldown(), "24");
    const time = screen.getByRole("textbox", { name: "Uhrzeit" });
    await user.clear(time);
    await user.type(time, "7:15");
    await save();

    expect(await screen.findByText("Einstellungen des AI-PM gespeichert.")).toBeInTheDocument();
    const { version: _version, ...values } = policy;
    expect(posts(calls)).toEqual([
      {
        call: "POST /api/v1/organization/pm-policy/set",
        body: { ...values, cooldown_hours: 24, local_time: "07:15:00", expected_version: 3 },
      },
    ]);
    expect(time).toHaveValue("07:15");
  });

  it("checks the ranges before it sends and focuses the first wrong field", async () => {
    const calls = setup({});

    await user.clear(await cooldown());
    await user.type(await cooldown(), "169");
    const lookAhead = screen.getByRole("textbox", { name: "Vorschau (Arbeitstage)" });
    await user.clear(lookAhead);
    await user.type(lookAhead, "11");
    await save();

    expect(await screen.findByText("Geben Sie eine Zahl von 1 bis 168 ein.")).toBeInTheDocument();
    expect(screen.getByText("Geben Sie eine Zahl von 0 bis 10 ein.")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveFocus());
    expect(posts(calls)).toEqual([]);
  });

  it("needs at least one weekday", async () => {
    setup({});
    await cooldown();
    for (const day of ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag"]) {
      await user.click(screen.getByRole("checkbox", { name: day }));
    }
    await save();

    expect(await screen.findByText("Wählen Sie mindestens einen Wochentag.")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Montag" })).toHaveFocus());
  });

  it("shows the field error that the server names", async () => {
    setup({
      "POST /api/v1/organization/pm-policy/set": [
        () =>
          problem(422, "validation-failed", [{ pointer: "/time_zone", code: "invalid-time-zone" }]),
      ],
    });
    await cooldown();
    await save();

    expect(
      await screen.findByText(
        "tada kennt diese Zeitzone nicht. Wählen Sie eine Zeitzone aus der Liste.",
      ),
    ).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Zeitzone" })).toHaveFocus());
  });

  it("loads the policy again after a version conflict", async () => {
    const calls = setup({
      "GET /api/v1/organization/pm-policy": [
        () => json(200, policy),
        () => json(200, { ...policy, cooldown_hours: 48, version: 5 }),
      ],
      "POST /api/v1/organization/pm-policy/set": [() => problem(409, "record-version-conflict")],
    });

    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");
    await save();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Jemand hat die Einstellungen inzwischen geändert.",
    );
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveValue("48"),
    );
    expect(calls.filter((call) => call.call === "GET /api/v1/organization/pm-policy")).toHaveLength(
      2,
    );
  });

  it("pauses all messages at once and keeps the edits of the form", async () => {
    const calls = setup({
      "POST /api/v1/organization/pm-policy/set": [
        () => json(200, { ...policy, paused: true, version: 4 }),
      ],
    });
    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");

    await user.click(screen.getByRole("switch", { name: "Pause für alle Nachrichten" }));

    expect(
      await screen.findByText("Pause für alle Nachrichten ist eingeschaltet."),
    ).toBeInTheDocument();
    expect(posts(calls)[0]?.body).toMatchObject({
      paused: true,
      cooldown_hours: 20,
      expected_version: 3,
    });
    expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveValue("30");
  });

  it("asks for a new sign-in before a change of the cap", async () => {
    setup({
      "POST /api/v1/organization/ai/set-cap": [() => problem(403, "recent-sign-in-required")],
    });
    const cap = await screen.findByRole("textbox", { name: "Monatslimit (CHF)" });
    expect(cap).toHaveValue("25.00");
    await user.clear(cap);
    await user.type(cap, "40,50");
    await user.click(screen.getByRole("button", { name: "Monatslimit speichern" }));

    expect(await screen.findByRole("button", { name: "Neu anmelden" })).toBeInTheDocument();
  });

  it("asks for a new sign-in before a change of the AI switch", async () => {
    const calls = setup({
      "POST /api/v1/organization/pm-policy/set": [() => problem(403, "recent-sign-in-required")],
    });
    const aiSwitch = await screen.findByRole("switch", { name: "KI formuliert die Erinnerungen" });
    await user.click(aiSwitch);

    expect(await screen.findByRole("button", { name: "Neu anmelden" })).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Melden Sie sich für diesen Schritt neu an",
    );
    expect(aiSwitch).not.toBeChecked();
    expect(posts(calls)).toHaveLength(1);
  });

  it("saves the cap and shows the usage", async () => {
    const calls = setup({
      "POST /api/v1/organization/ai/set-cap": [
        () => json(200, { ...ai, cap_chf: "40.50", version: 2 }),
      ],
    });
    const cap = await screen.findByRole("textbox", { name: "Monatslimit (CHF)" });
    expect(screen.getByText("CHF 3.20 von CHF 25.00")).toBeInTheDocument();
    await user.clear(cap);
    await user.type(cap, "40,50");
    await user.click(screen.getByRole("button", { name: "Monatslimit speichern" }));

    expect(await screen.findByText("Monatslimit gespeichert.")).toBeInTheDocument();
    expect(posts(calls)[0]?.body).toEqual({ cap_chf: "40.50", expected_version: 1 });
    expect(screen.getByText("CHF 3.20 von CHF 40.50")).toBeInTheDocument();
  });

  it("refuses a cap of zero", async () => {
    const calls = setup({});
    const cap = await screen.findByRole("textbox", { name: "Monatslimit (CHF)" });
    await user.clear(cap);
    await user.type(cap, "0");
    await user.click(screen.getByRole("button", { name: "Monatslimit speichern" }));

    expect(await screen.findByText("Geben Sie einen Betrag über 0 ein.")).toBeInTheDocument();
    expect(posts(calls)).toEqual([]);
  });

  it("says when the provider is not set up", async () => {
    setup({ "GET /api/v1/organization/ai": [() => json(200, { ...ai, configured: false })] });
    expect(await screen.findByText("nicht eingerichtet")).toBeInTheDocument();
  });

  it("lists the digests with their outcome", async () => {
    setup({});
    const table = await screen.findByRole("table", { name: "Letzte Erinnerungen" });
    const row = within(table).getAllByRole("row")[1];
    expect(row).toHaveTextContent("Anna Muster");
    expect(row).toHaveTextContent("13.05.2030, 08:30");
    expect(row).toHaveTextContent("unbekannt");
    // The labels of the column share one text edge (doc/design/components.md, StatusLabel).
    expect(within(row as HTMLElement).getByText("unbekannt")).toHaveAttribute("data-column");
    expect(screen.getByText(/Unbekannt heisst/)).toBeInTheDocument();
  });

  it("shows a member the no-access state and sends no request", async () => {
    const calls = setup({}, "member");
    expect(
      await screen.findByText("Sie haben keinen Zugriff auf diesen Bereich."),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Die Organisationsleitung oder die Administration/),
    ).toBeInTheDocument();
    expect(calls).toEqual([]);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("shows the no-access state when the server refuses, without a retry", async () => {
    setup({
      "GET /api/v1/organization/pm-policy": [() => problem(403, "forbidden")],
      "GET /api/v1/organization/ai": [() => problem(403, "forbidden")],
    });
    expect(
      await screen.findByText("Sie haben keinen Zugriff auf diesen Bereich."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Erneut versuchen" })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("shows the pause as a banner while it is on", async () => {
    setup({
      "GET /api/v1/organization/pm-policy": [() => json(200, { ...policy, paused: true })],
    });
    expect(await screen.findByText("Der AI-PM ist pausiert.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Pause für alle Nachrichten" })).toBeChecked();
  });

  it("shows no pause banner while tada sends reminders", async () => {
    setup({});
    await cooldown();
    expect(screen.queryByText("Der AI-PM ist pausiert.")).not.toBeInTheDocument();
  });

  it("names the switches, not the form, after a conflict of a switch with unsaved edits", async () => {
    setup({
      "GET /api/v1/organization/pm-policy": [
        () => json(200, policy),
        () => json(200, { ...policy, ai_enabled: true, version: 5 }),
      ],
      "POST /api/v1/organization/pm-policy/set": [() => problem(409, "record-version-conflict")],
    });
    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");

    await user.click(screen.getByRole("switch", { name: "Pause für alle Nachrichten" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Jemand hat die Einstellungen inzwischen geändert. Die Schalter zeigen jetzt die neuen Werte.",
    );
    expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveValue("30");
    expect(screen.getByRole("switch", { name: "Pause für alle Nachrichten" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "KI formuliert die Erinnerungen" })).toBeChecked();
  });

  it("says when the new values of a conflict did not load", async () => {
    setup({
      "GET /api/v1/organization/pm-policy": [
        () => json(200, policy),
        () => problem(500, "internal"),
      ],
      "POST /api/v1/organization/pm-policy/set": [() => problem(409, "record-version-conflict")],
    });
    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");
    await save();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "tada konnte die neuen Werte nicht laden. Laden Sie die Seite neu.",
    );
    expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveValue("30");
  });

  it("loads the cap again after a version conflict", async () => {
    setup({
      "GET /api/v1/organization/ai": [
        () => json(200, ai),
        () => json(200, { ...ai, cap_chf: "60.00", version: 2 }),
      ],
      "POST /api/v1/organization/ai/set-cap": [() => problem(409, "record-version-conflict")],
    });
    const cap = await screen.findByRole("textbox", { name: "Monatslimit (CHF)" });
    await user.clear(cap);
    await user.type(cap, "40");
    await user.click(screen.getByRole("button", { name: "Monatslimit speichern" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Das Feld zeigt jetzt den neuen Wert.",
    );
    expect(cap).toHaveValue("60.00");
  });

  it("says when the new cap of a conflict did not load", async () => {
    setup({
      "GET /api/v1/organization/ai": [() => json(200, ai), () => problem(500, "internal")],
      "POST /api/v1/organization/ai/set-cap": [() => problem(409, "record-version-conflict")],
    });
    const cap = await screen.findByRole("textbox", { name: "Monatslimit (CHF)" });
    await user.clear(cap);
    await user.type(cap, "40");
    await user.click(screen.getByRole("button", { name: "Monatslimit speichern" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "tada konnte den neuen Wert nicht laden. Laden Sie die Seite neu.",
    );
    expect(cap).toHaveValue("40");
  });
  it("never saves the edits of the form over a change of the form from someone else", async () => {
    const calls = setup({
      "GET /api/v1/organization/pm-policy": [
        () => json(200, policy),
        () => json(200, { ...policy, follow_up_working_days: 5, version: 4 }),
      ],
      "POST /api/v1/organization/pm-policy/set": [
        () => problem(409, "record-version-conflict"),
        () => problem(409, "record-version-conflict"),
      ],
    });
    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");
    // The switch fails: someone else changed the follow-up in version 4.
    await user.click(screen.getByRole("switch", { name: "Pause für alle Nachrichten" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Die Schalter zeigen jetzt");
    expect(screen.getByRole("textbox", { name: /Abklingzeit/ })).toHaveValue("30");

    await save();

    // The edits are based on version 3, so the server refuses them instead of resetting the follow-up.
    await waitFor(() => expect(posts(calls)).toHaveLength(2));
    expect(posts(calls)[1]?.body).toMatchObject({ cooldown_hours: 30, expected_version: 3 });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ihre Änderungen im Formular sind nicht gespeichert.",
    );
  });

  it("saves the edits of the form on the version of an own switch", async () => {
    const calls = setup({
      "POST /api/v1/organization/pm-policy/set": [
        () => json(200, { ...policy, paused: true, version: 4 }),
        () => json(200, { ...policy, paused: true, cooldown_hours: 30, version: 5 }),
      ],
    });
    await user.clear(await cooldown());
    await user.type(await cooldown(), "30");
    await user.click(screen.getByRole("switch", { name: "Pause für alle Nachrichten" }));
    expect(
      await screen.findByText("Pause für alle Nachrichten ist eingeschaltet."),
    ).toBeInTheDocument();

    await save();

    expect(await screen.findByText("Einstellungen des AI-PM gespeichert.")).toBeInTheDocument();
    expect(posts(calls)[1]?.body).toMatchObject({
      cooldown_hours: 30,
      paused: true,
      expected_version: 4,
    });
    expect(screen.queryByRole("alert")).toBeEmptyDOMElement();
  });
});
