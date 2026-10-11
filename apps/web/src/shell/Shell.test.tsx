import { render, screen, within } from "@testing-library/react";
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

/** The versions that the fake server reports; by default the build of this client. */
function systemVersion(product = __TADA_VERSION__, commit = __TADA_COMMIT__) {
  return {
    product,
    commit,
    schema: 27,
    api: "v1",
    mcp: product,
    roles: [
      { role: "serve", version: product, commit, last_seen: "2030-05-18T08:00:00Z" },
      { role: "worker", version: "0.0.9", commit: "9f8e7d6", last_seen: "2030-05-18T07:59:30Z" },
    ],
    mismatch: true,
  };
}

let server = systemVersion();

function renderShellAt(path: string, ...responses: Response[]) {
  const calls: { method: string; path: string; body: string }[] = [];
  const fetch = vi.fn(async (request: Request) => {
    // The shell asks for the versions on its own; the other answers keep their order.
    if (new URL(request.url).pathname === "/api/v1/system/version") {
      return json(200, server);
    }
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

afterEach(() => {
  window.history.replaceState(null, "", "/");
  server = systemVersion();
});

describe("Shell", () => {
  it("shows the navigation, the page and no switcher for one membership", async () => {
    renderShell(json(200, info([first], first)));

    expect(await screen.findByText("Seiteninhalt")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Anlässe" })).toHaveAttribute("aria-current", "page");
    expect(screen.queryByRole("button", { name: /Organisation/ })).not.toBeInTheDocument();
  });

  it("links Meine Arbeit to / and marks it as current there only", async () => {
    renderShellAt("/", json(200, info([first], first)));

    const link = await screen.findByRole("link", { name: "Meine Arbeit" });
    expect(link).toHaveAttribute("href", "/");
    expect(link).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
  });

  it("does not mark Meine Arbeit as current on the events list", async () => {
    renderShell(json(200, info([first], first)));

    expect(await screen.findByRole("link", { name: "Meine Arbeit" })).not.toHaveAttribute(
      "aria-current",
    );
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

  it("links to the privacy notice in the footer and in the member menu", async () => {
    renderShell(json(200, info([first], first)));

    const footer = await screen.findByRole("link", { name: "Datenschutz" });
    expect(footer).toHaveAttribute("href", "/privacy");
    await userEvent.click(screen.getByRole("button", { name: "Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Datenschutz" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/privacy"));
  });

  it("signs out from the member menu", async () => {
    const calls = renderShell(json(200, info([first], first)), new Response(null, { status: 204 }));

    await userEvent.click(await screen.findByRole("button", { name: "Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Abmelden" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(calls[1]).toMatchObject({ method: "POST", path: "/api/v1/sign-out" });
  });

  it("links to the persons and institutions in the navigation", async () => {
    renderShell(json(200, info([first], first)));

    expect(await screen.findByRole("link", { name: "Personen" })).toHaveAttribute(
      "href",
      "/persons",
    );
    expect(screen.getByRole("link", { name: "Institutionen" })).toHaveAttribute(
      "href",
      "/institutions",
    );
  });

  it.each([
    ["Personen", "/persons"],
    ["Institutionen", "/institutions"],
  ])("leads to %s from the member menu", async (name, path) => {
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name }));

    await vi.waitFor(() => expect(window.location.pathname).toBe(path));
  });

  it("shows the build in the footer and opens the list of the components", async () => {
    renderShell(json(200, info([first], first)));

    const footer = await screen.findByRole("button", {
      name: `tada ${__TADA_VERSION__} · ${__TADA_COMMIT__}`,
    });
    await userEvent.click(footer);

    const sheet = await screen.findByRole("dialog", { name: "Versionen" });
    const roles = await within(sheet).findByRole("table", { name: "Laufende Prozesse" });
    expect(within(roles).getByRole("row", { name: /worker/ })).toHaveTextContent("Andere Version");
    expect(within(roles).getByRole("row", { name: /serve/ })).not.toHaveTextContent(
      "Andere Version",
    );
    expect(within(sheet).getByRole("row", { name: /Datenbankschema/ })).toHaveTextContent("27");
    expect(screen.queryByText("Eine neue Version von tada ist installiert.")).toBeNull();
  });

  it("opens the list of the components from the member menu", async () => {
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: /^Versionen \(tada / }));

    expect(await screen.findByRole("dialog", { name: "Versionen" })).toBeInTheDocument();
  });

  it("has an empty polite region while the builds match", async () => {
    renderShell(json(200, info([first], first)));

    await screen.findByRole("button", { name: /^tada / });
    for (const region of screen.getAllByRole("status")) {
      expect(region).toBeEmptyDOMElement();
    }
  });

  it("asks to load the page again when the server runs another build", async () => {
    server = systemVersion("9.9.9", "fedcba9");
    renderShell(json(200, info([first], first)));

    expect(await screen.findByText("Eine neue Version von tada ist installiert.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Seite neu laden" })).toBeInTheDocument();
    // The prompt appears without an action of the member, so the polite region announces it.
    const announced = screen
      .getAllByRole("status")
      .some((region) =>
        region.textContent?.startsWith("Eine neue Version von tada ist installiert. Diese Seite"),
      );
    expect(announced).toBe(true);
  });
});
