import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { TokensPage } from "./TokensPage";

const SECRET = "tada_pat_EXAMPLE-SECRET-0123456789";

const token = {
  id: "t1",
  name: "Claude Code",
  scope: "read",
  expires_at: "2099-01-01T00:00:00Z",
  notice_version: 1,
  created_at: "2030-05-18T08:00:00Z",
  last_used_at: null,
  revoked_at: null,
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: unknown) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1", errors };
  return json(status, body, "application/problem+json");
}

interface Setup {
  tokens?: unknown[];
  enabled?: boolean;
  answers?: Record<string, () => Response>;
}

/** A fake server. It records `METHOD path` and the JSON body of each call. */
function setup({ tokens = [token], enabled = true, answers = {} }: Setup = {}) {
  const calls: { call: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    const text = request.method === "POST" ? await request.clone().text() : "";
    const key = `${request.method} ${pathname}`;
    calls.push({ call: key, body: text === "" ? undefined : JSON.parse(text) });
    const answer = answers[key];
    if (answer) {
      return answer();
    }
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (key === "GET /api/v1/token-notice") {
      return json(200, { version: 1 });
    }
    if (key === "GET /api/v1/tokens") {
      return json(200, { items: tokens });
    }
    if (key === "GET /api/v1/organization/features") {
      return json(200, { items: [{ feature: "mcp-tokens", enabled, version: 2 }] });
    }
    if (key === "POST /api/v1/tokens") {
      return json(201, { token, secret: SECRET });
    }
    if (key === "POST /api/v1/tokens/t1/revoke") {
      return new Response(null, { status: 204 });
    }
    throw new Error(`unexpected ${key}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/settings/tokens");
  const view = render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/settings/tokens">
            <TokensPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls, view, count: (call: string) => calls.filter((c) => c.call === call).length };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

const createButton = () => screen.findByRole("button", { name: "Token erstellen" });

async function fillAndConfirm(name = "Codex") {
  await user.type(await screen.findByRole("textbox", { name: /Name/ }), name);
  await user.click(screen.getByRole("checkbox", { name: /Hinweis gelesen/ }));
}

describe("TokensPage", () => {
  it("shows the notice and keeps „Token erstellen“ disabled until the member confirms it", async () => {
    setup();

    expect(await screen.findByText(/Zugriff auf die Personendaten/)).toBeTruthy();
    expect(await createButton()).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("checkbox", { name: /Hinweis gelesen/ }));
    expect(await createButton()).toHaveProperty("disabled", false);
  });

  it("sends the notice version with the token and shows the secret once, with its warning", async () => {
    const { calls } = setup();
    await fillAndConfirm();

    await user.click(await createButton());

    expect(await screen.findByText(SECRET)).toBeTruthy();
    expect(screen.getByText("Wird nur einmal angezeigt")).toBeTruthy();
    const post = calls.find((c) => c.call === "POST /api/v1/tokens");
    expect(post?.body).toMatchObject({
      name: "Codex",
      scope: "read",
      notice_version_confirmed: 1,
    });
    // Focus goes to the secret, so that a screen reader reads it.
    await waitFor(() => expect(document.activeElement?.textContent).toContain(SECRET));
    // The next token needs its own confirmation.
    expect(screen.getByRole("checkbox", { name: /Hinweis gelesen/ })).toHaveProperty(
      "checked",
      false,
    );
  });

  it("keeps the secret out of storage and drops it when the member leaves the page", async () => {
    const { view } = setup();
    await fillAndConfirm();
    await user.click(await createButton());
    await screen.findByText(SECRET);

    expect(JSON.stringify({ ...localStorage, ...sessionStorage })).not.toContain(SECRET);
    expect(window.location.href).not.toContain(SECRET);
    view.unmount();
    expect(document.body.textContent).not.toContain(SECRET);
  });

  it("moves focus to the first invalid field when the name is empty", async () => {
    setup();
    await user.click(await screen.findByRole("checkbox", { name: /Hinweis gelesen/ }));

    await user.click(await createButton());

    const name = screen.getByRole("textbox", { name: /Name/ });
    expect(name.getAttribute("aria-invalid")).toBe("true");
    await waitFor(() => expect(document.activeElement).toBe(name));
  });

  it("shows clearly that an owner switched off MCP tokens, and does not offer to create", async () => {
    const { count } = setup({ enabled: false });

    expect(await screen.findByText(/hat die MCP-Token ausgeschaltet/)).toBeTruthy();
    expect(await createButton()).toHaveProperty("disabled", true);
    expect(count("POST /api/v1/tokens")).toBe(0);
  });

  it("explains a refusal of the create call when the switch turned off meanwhile", async () => {
    let enabled = true;
    setup({
      answers: {
        "GET /api/v1/organization/features": () =>
          json(200, { items: [{ feature: "mcp-tokens", enabled, version: 2 }] }),
        "POST /api/v1/tokens": () => {
          enabled = false;
          return problem(403, "forbidden");
        },
      },
    });
    await fillAndConfirm();

    await user.click(await createButton());

    const messages = await screen.findAllByText(/hat die MCP-Token ausgeschaltet/);
    expect(messages.length).toBeGreaterThan(0);
    expect(screen.queryByText(SECRET)).toBeNull();
  });

  it("says that the right to propose is missing when the switch is on", async () => {
    setup({ answers: { "POST /api/v1/tokens": () => problem(403, "forbidden") } });
    await fillAndConfirm();

    await user.click(await createButton());

    expect(await screen.findByText(/Das Recht, Vorschläge zu machen/)).toBeTruthy();
  });

  it("shows a failed notice call with a retry that loads the version again", async () => {
    let failing = true;
    setup({
      answers: {
        "GET /api/v1/token-notice": () =>
          failing ? problem(503, "unavailable") : json(200, { version: 1 }),
      },
    });

    const message = await screen.findByText(/nicht erreichbar/);
    const alert = message.closest("[role=alert]") as HTMLElement;
    failing = false;
    await user.click(within(alert).getByRole("button", { name: "Erneut versuchen" }));

    await waitFor(() => expect(screen.queryByText(/nicht erreichbar/)).toBeNull());
    await user.click(screen.getByRole("checkbox", { name: /Hinweis gelesen/ }));
    expect(await createButton()).toHaveProperty("disabled", false);
  });

  it("shows a neutral refusal when a 403 comes and the switch cannot be read", async () => {
    setup({
      answers: {
        "POST /api/v1/tokens": () => problem(403, "forbidden"),
        "GET /api/v1/organization/features": () => problem(503, "unavailable"),
      },
    });
    await fillAndConfirm();

    await user.click(await createButton());

    expect(await screen.findByText("Sie haben keine Berechtigung für diese Aktion.")).toBeTruthy();
    expect(screen.queryByText(/Das Recht, Vorschläge zu machen/)).toBeNull();
  });

  it("revokes a token after the member confirms the dialog", async () => {
    const { count } = setup();
    await user.click(await screen.findByRole("button", { name: "Claude Code widerrufen" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(count("POST /api/v1/tokens/t1/revoke")).toBe(0);
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    await waitFor(() => expect(count("POST /api/v1/tokens/t1/revoke")).toBe(1));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("shows the client example with the address of the page", async () => {
    setup();

    const examples = await screen.findAllByText(new RegExp(`${window.location.origin}/mcp`));
    expect(examples.length).toBe(2);
    expect(document.body.textContent).toContain("Authorization: Bearer");
  });
});
