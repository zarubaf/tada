import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { InboxProvider } from "../review/InboxProvider";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { Shell } from "./Shell";
import { CLIENT_BUILD_ID } from "./version";

const ME = "0199b8e0-0000-7000-8000-0000000000b1";
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
    user_id: ME,
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
function systemVersion(product = "0.0.0", commit = "1a2b3c4", web_build = CLIENT_BUILD_ID) {
  return {
    product,
    commit,
    web_build,
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
/** While true, the fake server does not answer the versions. */
let versionsPending = false;
/** The open changesets that the fake server reports; the inbox count shows their number. */
let openChangesets = 0;
/** The events that the fake server lists; the first page only. */
let eventItems: ReturnType<typeof event>[] = [];
/** While true, the page of the test has an h1, which the router focuses after a navigation. */
let pageHeading = false;
/** The event role of the signed-in member in the events that the fake server lists. */
let eventRole = "event-contributor";
/** The path of each request that the fake server received. */
let requested: string[] = [];
/** While set, the fake server holds back the answer for a single event until it resolves. */
let eventGate: Promise<void> | undefined;
/** The key of the remembered event of the first organization. */
const EVENT_KEY = `tada-event:${first.organization_id}`;

function event(id: string, name: string, key: string, status = "active") {
  return {
    id,
    key,
    name,
    time_zone: "Europe/Zurich",
    version: 1,
    created_at: "2030-01-01T08:00:00Z",
    status,
    redacted: false,
  };
}

function renderShellAt(path: string, ...responses: Response[]) {
  const calls: { method: string; path: string; body: string }[] = [];
  const fetch = vi.fn(async (request: Request) => {
    // The shell asks for the versions on its own; the other answers keep their order.
    if (new URL(request.url).pathname === "/api/v1/system/version") {
      return versionsPending ? new Promise<Response>(() => {}) : json(200, server);
    }
    // The inbox count, the event switcher and the palette ask on their own too.
    const { pathname } = new URL(request.url);
    requested.push(pathname);
    if (pathname === "/api/v1/changesets") {
      return json(200, {
        items: Array.from({ length: openChangesets }, (_, index) => ({ id: `c${index}` })),
        next_cursor: null,
      });
    }
    if (pathname === "/api/v1/events") {
      return json(200, { items: eventItems, next_cursor: null });
    }
    // What the palette reads to know which kinds the member may create in the event.
    if (pathname === "/api/v1/members") {
      return json(200, { items: [{ user_id: ME, display_name: "Anna Muster", role: "member" }] });
    }
    if (pathname.endsWith("/memberships")) {
      return json(200, {
        items: [{ user_id: ME, display_name: "Anna Muster", event_role: eventRole, version: 1 }],
      });
    }
    if (pathname.endsWith("/workstreams")) {
      return json(200, { items: [] });
    }
    const one = pathname.match(/^\/api\/v1\/events\/([^/]+)$/);
    if (one) {
      await eventGate;
      const found = eventItems.find((item) => item.id === one[1]);
      return found ? json(200, found) : json(404, { title: "Not found", status: 404 });
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
        <InboxProvider api={api}>
          <Shell api={api}>
            <p>Seiteninhalt</p>
            {pageHeading && <h1>Seitentitel</h1>}
          </Shell>
        </InboxProvider>
      </SessionProvider>
    </Router>,
  );
  return calls;
}

function renderShell(...responses: Response[]) {
  return renderShellAt("/events", ...responses);
}

/** Makes the layout narrow (below 40rem) or not, as `matchMedia` answers. */
function stubNarrow(narrow: boolean) {
  vi.stubGlobal("matchMedia", () => ({
    matches: narrow,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
  server = systemVersion();
  versionsPending = false;
  openChangesets = 0;
  eventItems = [];
  eventRole = "event-contributor";
  requested = [];
  eventGate = undefined;
  pageHeading = false;
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
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

  it.each(["/settings", "/settings?tab=telegram"])(
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
    await userEvent.click(screen.getByRole("button", { name: "Konto von Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Datenschutz" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/privacy"));
  });

  it("signs out from the member menu", async () => {
    const calls = renderShell(json(200, info([first], first)), new Response(null, { status: 204 }));

    await userEvent.click(await screen.findByRole("button", { name: "Konto von Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Abmelden" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(calls[1]).toMatchObject({ method: "POST", path: "/api/v1/sign-out" });
  });

  it("links to Kontakte in the navigation, and to no separate register", async () => {
    renderShell(json(200, info([first], first)));

    expect(await screen.findByRole("link", { name: "Kontakte" })).toHaveAttribute(
      "href",
      "/contacts",
    );
    expect(screen.queryByRole("link", { name: "Personen" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Institutionen" })).not.toBeInTheDocument();
  });

  it("says „Versionen“ in the footer until the server names its build", async () => {
    versionsPending = true;
    renderShell(json(200, info([first], first)));

    expect(await screen.findByRole("button", { name: "Versionen" })).toBeInTheDocument();
    expect(screen.queryByText(/tada \d/)).toBeNull();
  });

  it("shows the build in the footer and opens the list of the components", async () => {
    renderShell(json(200, info([first], first)));

    const footer = await screen.findByRole("button", {
      name: "tada 0.0.0 · 1a2b3c4",
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

    await userEvent.click(await screen.findByRole("button", { name: "Konto von Anna Muster" }));
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
    server = systemVersion("9.9.9", "fedcba9", "fedcba9876543210fedcba9876543210");
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

describe("sidebar", () => {
  it("lists the destinations in a named navigation, in this order", async () => {
    renderShell(json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    const names = within(nav)
      .getAllByRole("link")
      .map((link) => link.textContent);
    expect(names).toEqual(["Eingang", "Meine Arbeit", "Anlässe", "Kontakte"]);
  });

  it("adds Arbeit after Meine Arbeit inside an event, as the current page", async () => {
    renderShellAt(
      "/events/0199b8e0-0000-7000-8000-0000000000e1/work",
      json(200, info([first], first)),
    );

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    const links = within(nav).getAllByRole("link");
    expect(links.map((link) => link.textContent)).toEqual([
      "Eingang",
      "Meine Arbeit",
      "Arbeit",
      "Zeitplan",
      "Dokumente",
      "Anlässe",
      "Kontakte",
    ]);
    const work = within(nav).getByRole("link", { name: "Arbeit" });
    expect(work).toHaveAttribute("href", "/events/0199b8e0-0000-7000-8000-0000000000e1/work");
    expect(work).toHaveAttribute("aria-current", "page");
    // Two items are not current at once: Arbeit claims the page.
    expect(within(nav).getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
  });

  it("marks Zeitplan, not Arbeit, on the timeline view of the work page", async () => {
    renderShellAt(
      "/events/0199b8e0-0000-7000-8000-0000000000e1/work?view=timeline",
      json(200, info([first], first)),
    );

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    const timeline = within(nav).getByRole("link", { name: "Zeitplan" });
    expect(timeline).toHaveAttribute(
      "href",
      "/events/0199b8e0-0000-7000-8000-0000000000e1/work?view=timeline",
    );
    expect(timeline).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("link", { name: "Arbeit" })).not.toHaveAttribute("aria-current");
    expect(within(nav).getByRole("link", { name: "Anlässe" })).not.toHaveAttribute("aria-current");
  });

  it("has no Arbeit item outside an event", async () => {
    renderShellAt("/inbox", json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(within(nav).queryByRole("link", { name: "Arbeit" })).not.toBeInTheDocument();
  });

  it("shows the number of open proposals at Eingang for a screen reader as text", async () => {
    openChangesets = 3;
    renderShell(json(200, info([first], first)));

    const link = await screen.findByRole("link", { name: /^Eingang/ });
    await vi.waitFor(() => expect(link).toHaveTextContent("3 offen"));
  });

  it("puts the brand, the switchers and the links in one labelled region", async () => {
    renderShell(json(200, info([first, second], first)));

    const region = await screen.findByRole("region", { name: "Seitenleiste" });
    expect(within(region).getByText("tada")).toBeInTheDocument();
    expect(within(region).getByRole("button", { name: /Organisation/ })).toBeInTheDocument();
    expect(within(region).getByRole("button", { name: "Anlass wählen" })).toBeInTheDocument();
    expect(within(region).getByRole("navigation", { name: "Hauptnavigation" })).toBeInTheDocument();
    expect(within(region).getByRole("link", { name: "Datenschutz" })).toBeInTheDocument();
  });

  it("has Datenschutz, the build and the member at the foot", async () => {
    renderShell(json(200, info([first], first)));

    const foot = await screen.findByRole("link", { name: "Datenschutz" });
    expect(foot).toHaveAttribute("href", "/privacy");
    const settings = screen.getByRole("link", { name: "Einstellungen" });
    expect(settings).toHaveAttribute("href", "/settings");
    expect(settings.closest("nav")).toBeNull();
    expect(foot.closest("nav")).toBeNull();
    expect(await screen.findByRole("button", { name: /^tada / })).toBeInTheDocument();
    expect(screen.getByText("Mitglied")).toBeInTheDocument();
  });

  it("collapses and opens again with the toggle in the top bar, and remembers the choice", async () => {
    renderShell(json(200, info([first], first)));

    await screen.findByRole("navigation", { name: "Hauptnavigation" });
    const toggle = screen.getByRole("button", { name: "Seitenleiste ausblenden" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(toggle);

    expect(screen.queryByRole("navigation", { name: "Hauptnavigation" })).not.toBeInTheDocument();
    const reopen = screen.getByRole("button", { name: "Seitenleiste einblenden" });
    expect(reopen).toHaveAttribute("aria-expanded", "false");
    expect(localStorage.getItem("tada-sidebar")).toBe("collapsed");

    await userEvent.click(reopen);
    expect(screen.getByRole("navigation", { name: "Hauptnavigation" })).toBeInTheDocument();
    expect(localStorage.getItem("tada-sidebar")).toBeNull();
  });

  it("starts collapsed when the member collapsed it before", async () => {
    localStorage.setItem("tada-sidebar", "collapsed");
    renderShell(json(200, info([first], first)));

    await screen.findByRole("button", { name: "Seitenleiste einblenden" });
    expect(screen.queryByRole("navigation", { name: "Hauptnavigation" })).not.toBeInTheDocument();
  });
});

describe("top bar", () => {
  it("is the banner with the title, the command box, the theme toggle and the member menu", async () => {
    renderShellAt("/inbox", json(200, info([first], first)));

    const bar = await screen.findByRole("banner");
    expect(within(bar).getByText("Eingang")).toBeInTheDocument();
    expect(within(bar).getByRole("button", { name: "Suchen oder Befehl ausführen" })).toBeVisible();
    expect(within(bar).getByRole("button", { name: "Farbschema: System" })).toBeInTheDocument();
    expect(within(bar).getByRole("button", { name: "Konto von Anna Muster" })).toBeInTheDocument();
  });

  it("names the context in plain text: the page keeps the only h1", async () => {
    renderShellAt("/settings", json(200, info([first], first)));

    const bar = await screen.findByRole("banner");
    expect(within(bar).getByText("Einstellungen")).toBeInTheDocument();
    expect(within(bar).queryByRole("heading")).not.toBeInTheDocument();
  });

  it("changes the theme from the top bar", async () => {
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Farbschema: System" }));

    expect(screen.getByRole("button", { name: "Farbschema: Hell" })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
  });

  it("has no theme toggle in the sidebar", async () => {
    renderShell(json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(
      within(nav.parentElement as HTMLElement).queryByRole("button", { name: /Farbschema/ }),
    ).toBeNull();
  });
});

describe("event switcher", () => {
  const open = event("0199b8e0-0000-7000-8000-0000000000e1", "Sommerfest 2030", "SF30");
  const other = event("0199b8e0-0000-7000-8000-0000000000e2", "Herbstflug", "HF30");
  const closed = event("0199b8e0-0000-7000-8000-0000000000e3", "Jubiläum", "JB29", "closed");

  it("says that no event is chosen when the route names none and nothing is remembered", async () => {
    renderShellAt("/inbox", json(200, info([first], first)));

    expect(await screen.findByRole("button", { name: "Anlass wählen" })).toHaveTextContent(
      "Anlass wählen",
    );
  });

  it("shows the event of the route with its key, and names it in the top bar", async () => {
    eventItems = [open, other];
    renderShellAt(`/events/${open.id}`, json(200, info([first], first)));

    const switcher = await screen.findByRole("button", {
      name: "Anlass wechseln: Sommerfest 2030",
    });
    expect(switcher).toHaveTextContent("SF30");
    expect(within(screen.getByRole("banner")).getByText("Sommerfest 2030")).toBeInTheDocument();
  });

  it("marks a closed event in the subtitle", async () => {
    eventItems = [closed];
    renderShellAt(`/events/${closed.id}`, json(200, info([first], first)));

    expect(
      await screen.findByRole("button", { name: "Anlass wechseln: Jubiläum" }),
    ).toHaveTextContent("JB29 · abgeschlossen");
  });

  it("remembers the event of the route for other pages", async () => {
    eventItems = [open];
    renderShellAt(`/events/${open.id}`, json(200, info([first], first)));

    await screen.findByRole("button", { name: "Anlass wechseln: Sommerfest 2030" });
    expect(localStorage.getItem(EVENT_KEY)).toBe(open.id);
  });

  it("shows the remembered event away from the event pages", async () => {
    eventItems = [open];
    localStorage.setItem(EVENT_KEY, open.id);
    renderShellAt("/inbox", json(200, info([first], first)));

    expect(
      await screen.findByRole("button", { name: "Anlass wechseln: Sommerfest 2030" }),
    ).toBeInTheDocument();
    // The title of a page that is no event page stays the section.
    expect(within(screen.getByRole("banner")).getByText("Eingang")).toBeInTheDocument();
  });

  it("forgets a remembered event that the server does not find", async () => {
    localStorage.setItem(EVENT_KEY, "0199b8e0-0000-7000-8000-00000000dead");
    renderShellAt("/inbox", json(200, info([first], first)));

    await screen.findByRole("button", { name: "Anlass wählen" });
    await vi.waitFor(() => expect(localStorage.getItem(EVENT_KEY)).toBeNull());
  });

  it("keeps the remembered event when the server does not find another event of the route", async () => {
    eventItems = [open];
    localStorage.setItem(EVENT_KEY, open.id);
    renderShellAt("/events/0199b8e0-0000-7000-8000-00000000dead", json(200, info([first], first)));

    await vi.waitFor(() =>
      expect(requested).toContain("/api/v1/events/0199b8e0-0000-7000-8000-00000000dead"),
    );
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(localStorage.getItem(EVENT_KEY)).toBe(open.id);
  });

  it("remembers an event for each organization", async () => {
    eventItems = [open];
    localStorage.setItem(`tada-event:${second.organization_id}`, other.id);
    renderShellAt("/inbox", json(200, info([first, second], first)));

    // The event of the other organization does not show here, and the shell does not ask for it.
    await screen.findByRole("button", { name: "Anlass wählen" });
    expect(requested).not.toContain(`/api/v1/events/${other.id}`);
  });

  it.each(["/choose-organization", "/sign-in"])(
    "asks for no event on %s, with an event remembered",
    async (path) => {
      eventItems = [open];
      localStorage.setItem(EVENT_KEY, open.id);
      localStorage.setItem("tada-event", open.id);
      renderShellAt(path, json(200, info([first, second], undefined)));

      await screen.findByText("Seiteninhalt");
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(requested.filter((path) => path.startsWith("/api/v1/events"))).toEqual([]);
    },
  );

  it("loads the list again at each opening and shows a new event", async () => {
    eventItems = [open];
    renderShellAt("/inbox", json(200, info([first], first)));

    // With no remembered event the most recent open one is the scoped event.
    const trigger = await screen.findByRole("button", { name: /^Anlass (wählen|wechseln)/ });
    await userEvent.click(trigger);
    await screen.findByRole("menuitem", { name: /^Sommerfest 2030/ });
    await userEvent.keyboard("{Escape}");

    eventItems = [open, other];
    await userEvent.click(trigger);

    // The old list stays while the new one loads: no loading item replaces it.
    expect(screen.queryByRole("menuitem", { name: "Anlässe werden geladen …" })).toBeNull();
    expect(await screen.findByRole("menuitem", { name: "Herbstflug" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /^Sommerfest 2030/ })).toBeInTheDocument();
  });

  it("lists the events of the member, all events and no new event for a member", async () => {
    eventItems = [open, other];
    renderShellAt(`/events/${open.id}`, json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: /Anlass wechseln/ }));

    const menu = await screen.findByRole("menu");
    expect(await within(menu).findByRole("menuitem", { name: "Herbstflug" })).toBeInTheDocument();
    expect(
      within(menu).getByRole("menuitem", { name: "Sommerfest 2030, aktuell" }),
    ).toBeInTheDocument();
    expect(within(menu).getByRole("menuitem", { name: "Alle Anlässe" })).toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Neuer Anlass" })).not.toBeInTheDocument();
  });

  it("opens the chosen event", async () => {
    eventItems = [open, other];
    renderShellAt("/inbox", json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Anlass wählen" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Herbstflug" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${other.id}`));
  });

  it("leads to all events", async () => {
    renderShellAt("/inbox", json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Anlass wählen" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Alle Anlässe" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/events"));
  });

  it("offers a new event to an administration", async () => {
    renderShellAt("/inbox", json(200, info([second], second)));

    await userEvent.click(await screen.findByRole("button", { name: "Anlass wählen" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Neuer Anlass" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/events/new"));
  });
});

describe("narrow layout", () => {
  it("has a bottom bar with Eingang, Meine Arbeit, Anlässe and Mehr, and no sidebar", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(
      within(nav)
        .getAllByRole("link")
        .map((link) => link.textContent),
    ).toEqual(["Eingang", "Meine Arbeit", "Anlässe"]);
    expect(within(nav).getByRole("button", { name: "Mehr" })).toBeInTheDocument();
    expect(screen.getAllByRole("navigation")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Anlass wählen" })).not.toBeInTheDocument();
  });

  it.each([
    ["/contacts", "true"],
    ["/contacts?tab=institutions", "true"],
    ["/settings?tab=members", "true"],
    ["/inbox", null],
  ])("marks Mehr as current on %s when the page is one of its items", async (path, current) => {
    stubNarrow(true);
    renderShellAt(path, json(200, info([first], first)));

    const more = await screen.findByRole("button", { name: "Mehr" });
    expect(more.getAttribute("aria-current")).toBe(current);
  });

  it("marks Anlässe as current on the list of events, in the bottom bar", async () => {
    stubNarrow(true);
    renderShellAt("/events", json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(within(nav).getByRole("link", { name: "Anlässe" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("has Arbeit in the third place inside an event, as the current page", async () => {
    stubNarrow(true);
    renderShellAt(
      "/events/0199b8e0-0000-7000-8000-0000000000e1/work",
      json(200, info([first], first)),
    );

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(
      within(nav)
        .getAllByRole("link")
        .map((link) => link.textContent),
    ).toEqual(["Eingang", "Meine Arbeit", "Arbeit"]);
    expect(within(nav).getByRole("link", { name: "Arbeit" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // Anlässe is in Mehr, and the work page is not one of its pages.
    expect(within(nav).getByRole("button", { name: "Mehr" })).not.toHaveAttribute("aria-current");
  });

  it("moves Anlässe into Mehr inside an event, and marks Mehr on the other pages of the event", async () => {
    stubNarrow(true);
    renderShellAt("/events/0199b8e0-0000-7000-8000-0000000000e1", json(200, info([first], first)));

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(within(nav).getByRole("link", { name: "Arbeit" })).not.toHaveAttribute("aria-current");
    const more = within(nav).getByRole("button", { name: "Mehr" });
    expect(more).toHaveAttribute("aria-current", "true");
    await userEvent.click(more);
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent)
        .slice(0, 3),
    ).toEqual(["Anlässe", "Dokumente", "Kontakte"]);
  });

  it("shows the open proposals at Eingang in the bottom bar", async () => {
    stubNarrow(true);
    openChangesets = 2;
    renderShell(json(200, info([first], first)));

    const link = await screen.findByRole("link", { name: /^Eingang/ });
    await vi.waitFor(() => expect(link).toHaveTextContent("2 offen"));
  });

  it("holds the other destinations in the menu Mehr", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Mehr" }));

    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual([
      "Kontakte",
      "Einstellungen",
      "Datenschutz",
      expect.stringMatching(/^(tada|Versionen)/),
    ]);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Kontakte" }));
    await vi.waitFor(() => expect(window.location.pathname).toBe("/contacts"));
  });

  it("keeps the top bar with the theme toggle, the command box and the member menu", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    const bar = await screen.findByRole("banner");
    expect(within(bar).getByRole("button", { name: "Farbschema: System" })).toBeInTheDocument();
    expect(
      within(bar).getByRole("button", { name: "Suchen oder Befehl ausführen" }),
    ).toBeInTheDocument();
    expect(within(bar).getByRole("button", { name: "Konto von Anna Muster" })).toBeInTheDocument();
  });

  it("opens the sidebar as a drawer with the toggle, and closes it with Escape", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first, second], first)));

    const toggle = await screen.findByRole("button", { name: "Navigation öffnen" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await userEvent.click(toggle);

    const drawer = await screen.findByRole("dialog", { name: "Navigation" });
    const nav = within(drawer).getByRole("navigation", { name: "Hauptnavigation" });
    expect(within(nav).getAllByRole("link")).toHaveLength(4);
    expect(within(drawer).getByRole("button", { name: /Organisation/ })).toBeInTheDocument();
    expect(within(drawer).getByRole("button", { name: "Anlass wählen" })).toBeInTheDocument();
    expect(within(drawer).getByRole("link", { name: "Datenschutz" })).toBeInTheDocument();

    await userEvent.keyboard("{Escape}");
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await vi.waitFor(() => expect(toggle).toHaveFocus());
  });

  it("closes the drawer when a destination opens", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Navigation öffnen" }));
    const drawer = await screen.findByRole("dialog", { name: "Navigation" });
    await userEvent.click(within(drawer).getByRole("link", { name: "Kontakte" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe("/contacts"));
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("does not reopen the drawer when the layout gets narrow again after a resize", async () => {
    let narrow = true;
    const listeners = new Set<() => void>();
    vi.stubGlobal("matchMedia", () => ({
      get matches() {
        return narrow;
      },
      addEventListener: (_: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
    }));
    const resize = (value: boolean) => {
      narrow = value;
      act(() => {
        for (const listener of listeners) {
          listener();
        }
      });
    };
    renderShell(json(200, info([first], first)));
    await userEvent.click(await screen.findByRole("button", { name: "Navigation öffnen" }));
    await screen.findByRole("dialog", { name: "Navigation" });

    resize(false);
    await screen.findByRole("navigation", { name: "Hauptnavigation" });
    resize(true);

    await screen.findByRole("button", { name: "Navigation öffnen" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("leaves focus on the new page when a destination of the drawer opens", async () => {
    stubNarrow(true);
    pageHeading = true;
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Navigation öffnen" }));
    const drawer = await screen.findByRole("dialog", { name: "Navigation" });
    await userEvent.click(within(drawer).getByRole("link", { name: "Kontakte" }));

    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    // The drawer restores focus to the toggle on its own; the heading must keep it.
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(screen.getByRole("heading", { name: "Seitentitel" })).toHaveFocus();
  });

  it("closes the drawer with its close button", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Navigation öffnen" }));
    await userEvent.click(await screen.findByRole("button", { name: "Navigation schliessen" }));

    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("does not remember the drawer as the state of the sidebar", async () => {
    stubNarrow(true);
    renderShell(json(200, info([first], first)));

    await userEvent.click(await screen.findByRole("button", { name: "Navigation öffnen" }));
    await screen.findByRole("dialog", { name: "Navigation" });

    expect(localStorage.getItem("tada-sidebar")).toBeNull();
  });
});

describe("command palette", () => {
  const open = event("0199b8e0-0000-7000-8000-0000000000e1", "Sommerfest 2030", "SF30");
  const other = event("0199b8e0-0000-7000-8000-0000000000e2", "Herbstflug", "HF30");

  async function openPalette() {
    await userEvent.click(
      await screen.findByRole("button", { name: "Suchen oder Befehl ausführen" }),
    );
    return screen.findByRole("dialog", { name: "Befehle" });
  }

  it("opens from the command box and closes with Escape, and focus returns to the box", async () => {
    renderShell(json(200, info([first], first)));
    const box = await screen.findByRole("button", { name: "Suchen oder Befehl ausführen" });

    const palette = await openPalette();
    expect(within(palette).getByRole("searchbox")).toHaveFocus();
    await userEvent.keyboard("{Escape}");

    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await vi.waitFor(() => expect(box).toHaveFocus());
  });

  it.each([
    ["Ctrl", { ctrlKey: true }],
    ["Cmd", { metaKey: true }],
  ])("opens with %s+K", async (_name, modifier) => {
    renderShell(json(200, info([first], first)));
    await screen.findByRole("banner");

    fireEvent.keyDown(document.body, { key: "k", ...modifier });

    expect(await screen.findByRole("dialog", { name: "Befehle" })).toBeInTheDocument();
  });

  it("does not open with Ctrl+Shift+K or after another handler took the key", async () => {
    renderShell(json(200, info([first], first)));
    await screen.findByRole("banner");

    fireEvent.keyDown(document.body, { key: "K", ctrlKey: true, shiftKey: true });
    const taken = new KeyboardEvent("keydown", {
      key: "k",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    taken.preventDefault();
    document.body.dispatchEvent(taken);
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("does not open over another dialog", async () => {
    renderShell(json(200, info([first], first)));
    await userEvent.click(await screen.findByRole("button", { name: "Konto von Anna Muster" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: /^Versionen/ }));
    const versions = await screen.findByRole("dialog");

    fireEvent.keyDown(versions, { key: "k", ctrlKey: true });
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.queryByRole("dialog", { name: "Befehle" })).toBeNull();
  });

  it("lists the destinations and no record search", async () => {
    renderShell(json(200, info([first], first)));

    const palette = await openPalette();

    const names = within(palette)
      .getAllByRole("menuitem")
      .map((item) => item.textContent);
    expect(names).toEqual(
      expect.arrayContaining([
        "Eingang",
        "Meine Arbeit",
        "Anlässe",
        "Kontakte",
        "Einstellungen",
        "Datenschutz",
      ]),
    );
    expect(names).not.toContain("Neue Aufgabe");
  });

  it("filters while typing, announces the number of results and opens the choice with Enter", async () => {
    renderShell(json(200, info([first], first)));

    const palette = await openPalette();
    await userEvent.keyboard("kontak");

    expect(within(palette).getAllByRole("menuitem")).toHaveLength(1);
    await vi.waitFor(() =>
      expect(within(palette).getByRole("status")).toHaveTextContent("1 Treffer"),
    );
    await userEvent.keyboard("{Enter}");

    await vi.waitFor(() => expect(window.location.pathname).toBe("/contacts"));
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("moves with the arrow keys", async () => {
    renderShell(json(200, info([first], first)));

    await openPalette();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    await vi.waitFor(() => expect(window.location.pathname).not.toBe("/events"));
  });

  it("says that nothing matches", async () => {
    renderShell(json(200, info([first], first)));

    const palette = await openPalette();
    await userEvent.keyboard("zzzz");

    expect(await within(palette).findAllByText("Keine Treffer")).not.toHaveLength(0);
  });

  it("offers no new action away from the event pages, with an event remembered", async () => {
    eventItems = [open];
    localStorage.setItem(EVENT_KEY, open.id);
    renderShellAt("/inbox", json(200, info([first], first)));
    await screen.findByRole("button", { name: "Anlass wechseln: Sommerfest 2030" });

    const palette = await openPalette();

    expect(within(palette).queryByRole("menuitem", { name: "Neue Aufgabe" })).toBeNull();
  });

  const NEW = [
    "Neue Aufgabe",
    "Neue Zusage",
    "Neuer Meilenstein",
    "Neuer Entscheid",
    "Neues Risiko",
  ];
  const newItems = (palette: HTMLElement) =>
    within(palette)
      .queryAllByRole("menuitem")
      .map((item) => item.textContent)
      .filter((name): name is string => NEW.includes(name ?? ""));

  async function paletteInEvent(role: string, status = "active") {
    eventRole = role;
    eventItems = [event(open.id, open.name, open.key, status)];
    renderShellAt(`/events/${open.id}`, json(200, info([first], first)));
    await screen.findByRole("button", { name: "Anlass wechseln: Sommerfest 2030" });
    return openPalette();
  }

  it("offers a new record of a kind that a contributor may create, and no decision", async () => {
    const palette = await paletteInEvent("event-contributor");

    await vi.waitFor(() =>
      expect(newItems(palette)).toEqual([
        "Neue Aufgabe",
        "Neue Zusage",
        "Neuer Meilenstein",
        "Neues Risiko",
      ]),
    );
  });

  it("offers every kind to an event manager and opens the create state of the work page", async () => {
    const palette = await paletteInEvent("event-manager");
    await vi.waitFor(() => expect(newItems(palette)).toEqual(NEW));

    await userEvent.click(within(palette).getByRole("menuitem", { name: "Neuer Entscheid" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${open.id}/work`));
    expect(window.location.search).toBe("?new=decision");
  });

  it("opens the create state of an action", async () => {
    const palette = await paletteInEvent("event-contributor");
    await userEvent.click(await within(palette).findByRole("menuitem", { name: "Neue Aufgabe" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${open.id}/work`));
    expect(window.location.search).toBe("?new=action");
  });

  it("opens the dialog that manages the workstreams of the event on the work page", async () => {
    const palette = await paletteInEvent("event-viewer");
    await userEvent.click(
      await within(palette).findByRole("menuitem", { name: "Bereiche verwalten" }),
    );

    await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${open.id}/work`));
    expect(window.location.search).toBe("?manage=streams");
  });

  it("offers no workstream dialog outside an event", async () => {
    renderShell(json(200, info([first], first)));
    const palette = await openPalette();
    expect(within(palette).queryByRole("menuitem", { name: "Bereiche verwalten" })).toBeNull();
  });

  it("reads only the memberships of the event to know what a member may create", async () => {
    const palette = await paletteInEvent("event-contributor");
    await vi.waitFor(() => expect(newItems(palette)).toHaveLength(4));

    expect(requested.filter((path) => path.endsWith("/memberships"))).toHaveLength(1);
    expect(requested).not.toContain("/api/v1/members");
    expect(requested.some((path) => path.endsWith("/workstreams"))).toBe(false);
  });

  it("reads nothing for an owner or admin of the organization, who may create every kind", async () => {
    eventItems = [event(open.id, open.name, open.key)];
    renderShellAt(`/events/${open.id}`, json(200, info([second], second)));
    await screen.findByRole("button", { name: "Anlass wechseln: Sommerfest 2030" });

    const palette = await openPalette();

    await vi.waitFor(() => expect(newItems(palette)).toEqual(NEW));
    expect(requested.some((path) => path.endsWith("/memberships"))).toBe(false);
    expect(requested).not.toContain("/api/v1/members");
  });

  it("keeps the typed text when the event resolves after the palette opened", async () => {
    eventRole = "event-manager";
    eventItems = [event(open.id, open.name, open.key)];
    let release: () => void = () => {};
    eventGate = new Promise<void>((resolve) => {
      release = resolve;
    });
    renderShellAt(`/events/${open.id}`, json(200, info([first], first)));
    const palette = await openPalette();
    await userEvent.keyboard("pers");
    expect(newItems(palette)).toEqual([]);

    release();

    await vi.waitFor(() => expect(requested.some((p) => p.endsWith("/memberships"))).toBe(true));
    expect(within(palette).getByRole("searchbox")).toHaveValue("pers");
    expect(palette.isConnected).toBe(true);
  });

  it("offers no new record to a viewer or in a closed event", async () => {
    const viewer = await paletteInEvent("event-viewer");
    await vi.waitFor(() =>
      expect(requested.some((path) => path.endsWith("/memberships"))).toBe(true),
    );
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(newItems(viewer)).toEqual([]);
  });

  it("offers no new record in a closed event, also to a manager", async () => {
    const palette = await paletteInEvent("event-manager", "closed");
    await vi.waitFor(() =>
      expect(requested.some((path) => path.endsWith("/memberships"))).toBe(true),
    );
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(newItems(palette)).toEqual([]);
  });

  it("jumps to an event", async () => {
    eventItems = [open, other];
    renderShell(json(200, info([first], first)));

    const palette = await openPalette();
    await userEvent.keyboard("herbst");
    await userEvent.click(await within(palette).findByRole("menuitem", { name: "Herbstflug" }));

    await vi.waitFor(() => expect(window.location.pathname).toBe(`/events/${other.id}`));
  });

  it("switches the theme and the toggle in the top bar follows", async () => {
    renderShell(json(200, info([first], first)));

    await openPalette();
    await userEvent.click(await screen.findByRole("menuitem", { name: "Farbschema: Dunkel" }));

    await vi.waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    expect(await screen.findByRole("button", { name: "Farbschema: Dunkel" })).toBeInTheDocument();
  });
});
