import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { OrganizationPage } from "./OrganizationPage";

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function setup({ role = "owner", answers = {} as Record<string, () => Response> } = {}) {
  const calls: { call: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
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
    if (key === "GET /api/v1/organization/privacy-notice") {
      return json(200, { markdown: null, version: 1 });
    }
    if (key === "GET /api/v1/organization/features") {
      return json(200, { items: [{ feature: "mcp-tokens", enabled: true, version: 2 }] });
    }
    if (key === "POST /api/v1/organization/features/mcp-tokens/set") {
      return json(200, { feature: "mcp-tokens", enabled: false, version: 3 });
    }
    throw new Error(`unexpected ${key}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/settings/organization");
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/settings/organization">
            <OrganizationPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

const mcpSwitch = () => screen.findByRole("switch", { name: /MCP-Token/ });

describe("OrganizationPage", () => {
  it("lets an owner switch off MCP tokens with the version of the record", async () => {
    const { calls } = setup();
    const control = await mcpSwitch();
    expect(control).toHaveProperty("checked", true);

    await user.click(control);

    await waitFor(() => expect(control).toHaveProperty("checked", false));
    const post = calls.find((c) => c.call.startsWith("POST"));
    expect(post?.body).toEqual({ enabled: false, expected_version: 2 });
    // The switch stays and keeps focus; the result goes to the live region.
    expect(document.activeElement).toBe(control);
    expect(await screen.findByText("MCP-Token sind ausgeschaltet.")).toBeTruthy();
  });

  it("ignores a feature that it does not know, because the list of features is open", async () => {
    setup({
      answers: {
        "GET /api/v1/organization/features": () =>
          json(200, {
            items: [
              { feature: "model-calls", enabled: false, version: 1 },
              { feature: "mcp-tokens", enabled: true, version: 2 },
            ],
          }),
      },
    });
    expect(await mcpSwitch()).toHaveProperty("checked", true);
  });

  it("shows a member the state but does not let the member change it", async () => {
    const { calls } = setup({ role: "member" });

    expect(await mcpSwitch()).toHaveProperty("disabled", true);
    expect(calls.some((c) => c.call.startsWith("POST"))).toBe(false);
  });

  it("shows an owner the form of the privacy notice and a member only the link", async () => {
    setup();
    expect(
      await screen.findByRole("textbox", { name: "Text der Datenschutzerklärung" }),
    ).toBeTruthy();
  });

  it("does not show a member the form of the privacy notice", async () => {
    setup({ role: "member" });
    await mcpSwitch();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByRole("link", { name: "Datenschutzerklärung lesen" })).toBeTruthy();
  });
});
