import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { SettingsPage } from "./SettingsPage";

// The panels have their own tests; here each stands in with its panel title.
vi.mock("./AccountPage", () => ({ AccountPage: () => <h2>Konto-Seite</h2> }));
vi.mock("../members/MembersPage", () => ({ MembersPage: () => <h2>Mitglieder-Seite</h2> }));
vi.mock("./OrganizationPage", () => ({ OrganizationPage: () => <h2>Organisation-Seite</h2> }));
vi.mock("./PmPage", () => ({ PmPage: () => <h2>AI-PM-Seite</h2> }));
vi.mock("../telegram/TelegramPage", () => ({ TelegramPage: () => <h2>Telegram-Seite</h2> }));
vi.mock("../tokens/TokensPage", () => ({ TokensPage: () => <h2>Token-Seite</h2> }));

const TABS = ["Konto", "Mitglieder", "Organisation", "AI-PM", "Telegram", "API-Token"];

function stubNarrow(narrow: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: narrow && query.includes("40rem"),
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

function renderAt(path: string, role: string) {
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async () =>
    new Response(JSON.stringify(session), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", path);
  return render(
    <Router>
      <SessionProvider api={api}>
        <SettingsPage api={api} />
      </SessionProvider>
    </Router>,
  );
}

const user = userEvent.setup({ delay: null });

afterEach(() => {
  window.history.replaceState(null, "", "/");
  vi.unstubAllGlobals();
});

describe("SettingsPage", () => {
  it.each([
    ["owner", TABS],
    ["admin", TABS],
    ["member", TABS.filter((label) => label !== "AI-PM")],
  ])("lists the tabs for the role %s", async (role, expected) => {
    renderAt("/settings", role);
    const list = await screen.findByRole("tablist", { name: "Bereich" });
    expect(
      within(list)
        .getAllByRole("tab")
        .map((tab) => tab.textContent),
    ).toEqual(expected);
  });

  it("has one main and one h1, and the panel titles are h2", async () => {
    renderAt("/settings", "owner");
    expect(await screen.findByRole("heading", { level: 2, name: "Konto-Seite" })).toBeVisible();
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getByRole("heading", { level: 1, name: "Einstellungen" })).toBeVisible();
    expect(screen.getAllByRole("main")).toHaveLength(1);
  });

  it("opens the first tab by default and keeps the tab out of the address", async () => {
    renderAt("/settings", "member");
    expect(await screen.findByRole("tab", { name: "Konto", selected: true })).toBeVisible();
    expect(window.location.search).toBe("");
  });

  it("round-trips the tab through the address", async () => {
    renderAt("/settings", "owner");
    await user.click(await screen.findByRole("tab", { name: "AI-PM" }));
    expect(await screen.findByRole("heading", { name: "AI-PM-Seite" })).toBeVisible();
    expect(window.location.search).toBe("?tab=ai-pm");

    window.history.back();
    expect(await screen.findByRole("heading", { name: "Konto-Seite" })).toBeVisible();
    expect(window.location.search).toBe("");
  });

  it("opens the tab that the address names", async () => {
    renderAt("/settings?tab=tokens", "member");
    expect(await screen.findByRole("heading", { name: "Token-Seite" })).toBeVisible();
    expect(screen.getByRole("tab", { name: "API-Token", selected: true })).toBeVisible();
  });

  it("falls back to the first tab for an unknown value", async () => {
    renderAt("/settings?tab=nonsense", "owner");
    expect(await screen.findByRole("heading", { name: "Konto-Seite" })).toBeVisible();
    expect(
      screen.queryByText("Sie haben keinen Zugriff auf diesen Bereich."),
    ).not.toBeInTheDocument();
  });

  it("falls back and says there is no access to a tab the member may not see", async () => {
    renderAt("/settings?tab=ai-pm", "member");
    expect(await screen.findByText("Sie haben keinen Zugriff auf diesen Bereich.")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Konto-Seite" })).toBeVisible();
    expect(screen.queryByRole("tab", { name: "AI-PM" })).not.toBeInTheDocument();
    expect(window.location.search).toBe("");

    await user.click(screen.getByRole("tab", { name: "Telegram" }));
    expect(await screen.findByRole("heading", { name: "Telegram-Seite" })).toBeVisible();
    expect(
      screen.queryByText("Sie haben keinen Zugriff auf diesen Bereich."),
    ).not.toBeInTheDocument();
  });

  it("shows a select in place of the tabs on a phone", async () => {
    stubNarrow(true);
    renderAt("/settings?tab=members", "member");
    expect(await screen.findByRole("heading", { name: "Mitglieder-Seite" })).toBeVisible();
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Mitglieder/ }));
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(
      TABS.filter((label) => label !== "AI-PM"),
    );
    await user.click(screen.getByRole("option", { name: "Telegram" }));
    expect(await screen.findByRole("heading", { name: "Telegram-Seite" })).toBeVisible();
    expect(window.location.search).toBe("?tab=telegram");
  });
});
