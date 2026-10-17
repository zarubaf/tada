import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { createApi } from "./api/client";

const membership = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "member",
};
const session = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  organization: membership,
  memberships: [membership],
};

function signedInApi() {
  const fetch = async (request: Request) => {
    const body = new URL(request.url).pathname.endsWith("/session")
      ? session
      : { items: [], actions: [], commitments: [] };
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  };
  return createApi(fetch as unknown as typeof globalThis.fetch);
}

function renderAt(path: string) {
  window.history.replaceState(null, "", path);
  return render(<App api={signedInApi()} />);
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("App", () => {
  it("shows My Work at /", async () => {
    renderAt("/");
    expect(await screen.findByRole("heading", { name: "Meine Arbeit" })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/");
  });

  // First to use /events: a lazy page loads once per test file, so this is its first visit.
  it("moves focus to the h1 of a lazy page after a navigation", async () => {
    renderAt("/");
    await screen.findByRole("heading", { name: "Meine Arbeit" });
    await userEvent.click(
      screen.getAllByRole("button", { name: "Anlass wählen" })[0] as HTMLElement,
    );
    await userEvent.click(await screen.findByRole("menuitem", { name: "Alle Anlässe" }));
    const heading = await screen.findByRole("heading", { level: 1, name: "Alle Anlässe" });
    expect(heading).toHaveFocus();
  });

  it("opens the Arbeit page of an event at /events/:id/work with its own h1", async () => {
    renderAt("/events/0199b8e0-0000-7000-8000-000000000001/work");
    expect(await screen.findByRole("heading", { level: 1, name: "Arbeit" })).toBeInTheDocument();
    expect(await screen.findByText("Noch keine Arbeit in diesem Anlass")).toBeInTheDocument();
  });

  it.each([
    ["actions#ACT-001", "/work", "?type=action", "#ACT-001"],
    ["commitments", "/work", "?type=commitment", ""],
    ["milestones", "/work", "?type=milestone", ""],
    ["decisions", "/work", "?type=decision", ""],
    ["timeline", "/work", "?view=timeline", ""],
    ["workstreams", "/work", "?manage=streams", ""],
    ["members", "", "", "#mitglieder"],
  ])("sends the old address /events/:id/%s to its new place", async (old, path, search, hash) => {
    const id = "0199b8e0-0000-7000-8000-000000000001";
    renderAt(`/events/${id}/${old}`);
    await waitFor(() => expect(window.location.pathname).toBe(`/events/${id}${path}`));
    expect(window.location.search).toBe(search);
    expect(window.location.hash).toBe(hash);
  });

  it("keeps the events list at /events", async () => {
    renderAt("/events");
    expect(await screen.findByRole("heading", { name: "Alle Anlässe" })).toBeInTheDocument();
  });

  it("loads Einstellungen on demand with the h1 of the page and the panel of the tab", async () => {
    renderAt("/settings");
    expect(await screen.findByRole("heading", { level: 1, name: "Einstellungen" })).toBeVisible();
    expect(await screen.findByRole("heading", { level: 2, name: "Konto" })).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("redirects an old settings address to its tab and keeps the query and the hash", async () => {
    renderAt("/settings/telegram?x=1#schritte");
    expect(await screen.findByRole("tab", { name: "Telegram", selected: true })).toBeVisible();
    expect(window.location.pathname + window.location.search + window.location.hash).toBe(
      "/settings?x=1&tab=telegram#schritte",
    );
  });

  it("shows the not-found page for an unknown settings address", async () => {
    renderAt("/settings/nowhere");
    expect(await screen.findByText("Seite nicht gefunden")).toBeInTheDocument();
  });

  it("shows the not-found page for an unknown path", async () => {
    renderAt("/nowhere");
    expect(await screen.findByText("Seite nicht gefunden")).toBeInTheDocument();
  });

  it("shows a page without a session without the sidebar, even for a signed-in member", async () => {
    renderAt("/sign-in/link");
    expect(await screen.findByRole("heading", { name: "Anmelden" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Hauptnavigation" })).not.toBeInTheDocument();
  });

  it("has the skip link as its first focusable element, and it targets main", async () => {
    renderAt("/events");
    await screen.findByRole("heading", { name: "Alle Anlässe" });

    await userEvent.tab();
    const link = screen.getByRole("link", { name: "Zum Inhalt springen" });
    expect(link).toHaveFocus();
    expect(link).toHaveAttribute("href", "#main");
    expect(screen.getByRole("main")).toHaveAttribute("id", "main");
  });
});
