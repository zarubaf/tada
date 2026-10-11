import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import PersonDataProtection from "./PersonDataProtection";

const mentions = [
  {
    kind: "source-item",
    id: "s1",
    event_id: "e1",
    excerpt: "Beat Muster fliegt mit.",
    redacted: false,
    disputed: false,
  },
  {
    kind: "fact-version",
    id: "f1",
    event_id: "e1",
    excerpt: '{"text":"Beat Muster"}',
    redacted: false,
    disputed: true,
  },
  {
    kind: "action",
    id: "a1",
    event_id: "e1",
    excerpt: "Beat Muster anrufen",
    redacted: false,
    disputed: false,
  },
  { kind: "document-version", id: "d1", event_id: "e1", redacted: true, disputed: false },
];

function json(status: number, body: unknown, type = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": type } });
}

function problem(status: number, code: string) {
  return json(
    status,
    { type: "", code, title: "", status, instance: "", request_id: "r1" },
    "application/problem+json",
  );
}

const session = {
  user_id: "u1",
  display_name: "Olga Owner",
  organization: { organization_id: "o1", name: "Fliegergruppe Testwil", role: "owner" },
  memberships: [{ organization_id: "o1", name: "Fliegergruppe Testwil", role: "owner" }],
};

function setup(answers: Record<string, () => Response> = {}) {
  const calls: { call: string; body: unknown }[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const text = request.method === "GET" ? "" : await request.clone().text();
    const call = `${request.method} ${url.pathname}`;
    calls.push({ call, body: text === "" ? undefined : JSON.parse(text) });
    if (answers[call]) {
      return answers[call]();
    }
    if (call === "GET /api/v1/persons/p1/mentions") {
      return json(200, { items: mentions });
    }
    if (call === "GET /api/v1/session") {
      return json(200, session);
    }
    throw new Error(`unexpected ${call}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  render(
    <Router>
      <SessionProvider api={api}>
        <PersonDataProtection api={api} personId="p1" />
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => vi.restoreAllMocks());

describe("PersonDataProtection", () => {
  it("export button downloads json", async () => {
    const createObjectURL = vi.fn(() => "blob:export");
    const revokeObjectURL = vi.fn();
    Object.assign(URL, { createObjectURL, revokeObjectURL });
    const saved: string[] = [];
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      saved.push(`${this.download} ${this.href}`);
    });
    const { calls } = setup({
      "GET /api/v1/persons/p1/export": () =>
        new Response(JSON.stringify({ format_version: 1 }), {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Content-Disposition": 'attachment; filename="person-PER-001.json"',
          },
        }),
    });

    await user.click(await screen.findByRole("button", { name: "Export herunterladen" }));

    await waitFor(() => expect(saved).toEqual(["person-PER-001.json blob:export"]));
    expect(calls.map((c) => c.call)).toContain("GET /api/v1/persons/p1/export");
    // The URL goes in a later task, after the browser started the download.
    await waitFor(() => expect(revokeObjectURL).toHaveBeenCalledWith("blob:export"));
  });

  it("offers a new sign-in when the export needs a recent one", async () => {
    setup({
      "GET /api/v1/persons/p1/export": () => problem(403, "recent-sign-in-required"),
    });

    await user.click(await screen.findByRole("button", { name: "Export herunterladen" }));

    expect(await screen.findByText(/Melden Sie sich für diesen Schritt neu an/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Neu anmelden" })).toBeVisible();
  });

  it("offers a new sign-in when a redaction needs a recent one", async () => {
    setup({ "POST /api/v1/redactions": () => problem(403, "recent-sign-in-required") });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("button", { name: "Entfernen" }));
    const dialog = await screen.findByRole("alertdialog");

    await user.click(within(dialog).getByRole("button", { name: "Endgültig entfernen" }));

    expect(await screen.findByText(/Melden Sie sich für diesen Schritt neu an/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Neu anmelden" })).toBeVisible();
    expect(screen.getByRole("checkbox", { name: /Beat Muster fliegt mit/ })).toBeChecked();
  });

  it("lists only the redaction targets as selectable and links the other kinds", async () => {
    setup();

    expect(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ })).toBeVisible();
    expect(screen.getByRole("checkbox", { name: /Angabe/ })).toBeVisible();
    expect(screen.queryByRole("checkbox", { name: /Beat Muster anrufen/ })).toBeNull();
    expect(screen.getByRole("link", { name: /Beat Muster anrufen/ })).toHaveAttribute(
      "href",
      "/events/e1/actions",
    );
    // A redacted mention shows „entfernt“ and cannot be selected.
    expect(screen.getByText("entfernt")).toBeVisible();
    expect(screen.queryByRole("checkbox", { name: /Dokumentversion/ })).toBeNull();
    expect(screen.getByText("bestritten")).toBeVisible();
  });

  it("mention list selects and redacts after confirmation naming count and irreversibility", async () => {
    let listed = 0;
    const { calls } = setup({
      "GET /api/v1/persons/p1/mentions": () =>
        json(200, { items: listed++ === 0 ? mentions : mentions.slice(2) }),
      "POST /api/v1/redactions": () => json(200, { applied: ["s1", "f1"], already: 0 }),
    });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("checkbox", { name: /Angabe/ }));

    await user.click(screen.getByRole("button", { name: "Entfernen" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/2 Einträge/)).toBeVisible();
    expect(within(dialog).getByText(/nicht rückgängig/)).toBeVisible();
    expect(calls.some((c) => c.call === "POST /api/v1/redactions")).toBe(false);

    await user.click(within(dialog).getByRole("button", { name: "Endgültig entfernen" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call === "POST /api/v1/redactions")?.body).toEqual({
        targets: [
          { kind: "source-item", id: "s1" },
          { kind: "fact-version", id: "f1" },
        ],
        reason: "request",
      }),
    );
    // The list loads again.
    await waitFor(() => expect(screen.queryByRole("checkbox", { name: /Angabe/ })).toBeNull());
  });

  it("bestreiten dialog posts note 1–1000", async () => {
    const { calls } = setup({
      "POST /api/v1/dispute-notes": () =>
        json(201, {
          id: "n1",
          target_kind: "source-item",
          target_id: "s1",
          text: "Falsch.",
          created_at: "2026-10-10T08:00:00Z",
        }),
    });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("button", { name: "Bestreiten" }));

    const dialog = await screen.findByRole("dialog");
    const field = within(dialog).getByRole("textbox", { name: "Einwand" });
    await user.click(within(dialog).getByRole("button", { name: "Einwand speichern" }));
    expect(within(dialog).getByText(/1 bis 1000 Zeichen/)).toBeVisible();

    await user.click(field);
    await user.paste("x".repeat(1001));
    await user.click(within(dialog).getByRole("button", { name: "Einwand speichern" }));
    expect(within(dialog).getByText(/1 bis 1000 Zeichen/)).toBeVisible();
    expect(calls.some((c) => c.call === "POST /api/v1/dispute-notes")).toBe(false);

    await user.clear(field);
    await user.type(field, "Falsch.");
    await user.click(within(dialog).getByRole("button", { name: "Einwand speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call === "POST /api/v1/dispute-notes")?.body).toEqual({
        target_kind: "source-item",
        target_id: "s1",
        text: "Falsch.",
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it.each([
    [403, "forbidden", "Sie haben keine Berechtigung für diese Aktion."],
    [404, "not-found", "Das gibt es nicht, oder Sie dürfen es nicht sehen."],
    [409, "invalid-transition", "Diese Änderung ist im aktuellen Zustand nicht möglich."],
  ])("shows the refusal %i of a redaction and keeps the list", async (status, code, message) => {
    setup({ "POST /api/v1/redactions": () => problem(status, code) });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("button", { name: "Entfernen" }));
    const dialog = await screen.findByRole("alertdialog");

    await user.click(within(dialog).getByRole("button", { name: "Endgültig entfernen" }));

    expect(await screen.findByText(message)).toBeVisible();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(screen.getByRole("checkbox", { name: /Beat Muster fliegt mit/ })).toBeChecked();
  });

  it.each([
    [403, "forbidden", "Sie haben keine Berechtigung für diese Aktion."],
    [404, "not-found", "Das gibt es nicht, oder Sie dürfen es nicht sehen."],
    [409, "invalid-transition", "Dieser Eintrag ist schon entfernt."],
  ])("shows the refusal %i of a dispute in the dialog", async (status, code, message) => {
    setup({ "POST /api/v1/dispute-notes": () => problem(status, code) });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("button", { name: "Bestreiten" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Einwand" }), "Falsch.");

    await user.click(within(dialog).getByRole("button", { name: "Einwand speichern" }));

    expect(await within(dialog).findByText(message)).toBeVisible();
  });

  it("shows the failure of the mention list", async () => {
    setup({ "GET /api/v1/persons/p1/mentions": () => problem(500, "internal") });

    expect(await screen.findByText("Ein unerwarteter Fehler ist aufgetreten.")).toBeVisible();
    expect(screen.queryByRole("checkbox")).toBeNull();
    // The export does not depend on the list.
    expect(screen.getByRole("button", { name: "Export herunterladen" })).toBeVisible();
  });

  it("moves focus to the list heading after a redaction and after a dispute", async () => {
    setup({
      "POST /api/v1/redactions": () => json(200, { applied: ["s1"], already: 0 }),
      "POST /api/v1/dispute-notes": () =>
        json(201, {
          id: "n",
          target_kind: "fact-version",
          target_id: "f1",
          text: "x",
          created_at: "2026-10-10T08:00:00Z",
        }),
    });
    await user.click(await screen.findByRole("checkbox", { name: /Beat Muster fliegt mit/ }));
    await user.click(screen.getByRole("button", { name: "Entfernen" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Endgültig entfernen",
      }),
    );
    await waitFor(() => expect(screen.getByRole("heading", { name: "Erwähnungen" })).toHaveFocus());

    await user.click(screen.getByRole("checkbox", { name: /Angabe/ }));
    await user.click(screen.getByRole("button", { name: "Bestreiten" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: "Einwand" }), "x");
    await user.click(within(dialog).getByRole("button", { name: "Einwand speichern" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(screen.getByRole("heading", { name: "Erwähnungen" })).toHaveFocus());
  });
});
