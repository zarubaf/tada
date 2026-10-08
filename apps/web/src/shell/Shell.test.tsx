import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { Shell } from "./Shell";

const first = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "member",
};
const second = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a2",
  name: "Segelflugclub Musterhausen",
  role: "admin",
};

function info(memberships: unknown[], organization: unknown) {
  return {
    user_id: "0199b8e0-0000-7000-8000-0000000000b1",
    display_name: "Anna Muster",
    organization,
    memberships,
  };
}

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function renderShellAt(path: string, ...responses: Response[]) {
  const calls: { method: string; path: string; body: string }[] = [];
  const fetch = vi.fn(async (request: Request) => {
    calls.push({
      method: request.method,
      path: new URL(request.url).pathname,
      body: await request.clone().text(),
    });
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", path);
  render(
    <Router>
      <SessionProvider api={api}>
        <Shell api={api}>
          <p>Seiteninhalt</p>
        </Shell>
      </SessionProvider>
    </Router>,
  );
  return calls;
}

function renderShell(...responses: Response[]) {
  return renderShellAt("/events", ...responses);
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("Shell", () => {
  it("shows the navigation, the page and no switcher for one membership", async () => {
    renderShell(json(200, info([first], first)));

    expect(await screen.findByText("Seiteninhalt")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Anlässe" })).toHaveAttribute("aria-current", "page");
    expect(screen.queryByRole("button", { name: /Organisation/ })).not.toBeInTheDocument();
  });

  it.each(["/settings/members", "/settings/telegram"])(
    "marks Einstellungen as current on %s",
    async (path) => {
      renderShellAt(path, json(200, info([first], first)));

      expect(await screen.findByRole("link", { name: "Einstellungen" })).toHaveAttribute(
        "aria-current",
        "page",
      );
      expect(screen.getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
    },
  );

  it("switches the organization and loads the session again", async () => {
    const calls = renderShell(
      json(200, info([first, second], first)),
      json(200, info([first, second], second)),
      json(200, info([first, second], second)),
    );

    const switcher = await screen.findByRole("button", { name: /Organisation/ });
    expect(switcher).toHaveTextContent(first.name);
    await userEvent.click(switcher);
    await userEvent.click(await screen.findByRole("option", { name: second.name }));

    await vi.waitFor(() =>
      expect(screen.getByRole("button", { name: /Organisation/ })).toHaveTextContent(second.name),
    );
    expect(calls[1]).toMatchObject({ method: "POST", path: "/api/v1/session/organization" });
    expect(JSON.parse(calls[1]?.body ?? "")).toEqual({ organization_id: second.organization_id });
  });

  it("signs out from the member menu", async () => {
    const calls = renderShell(json(200, info([first], first)), new Response(null, { status: 204 }));

    await userEvent.click(await screen.findByRole("button", { name: "Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Abmelden" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(calls[1]).toMatchObject({ method: "POST", path: "/api/v1/sign-out" });
  });
});
