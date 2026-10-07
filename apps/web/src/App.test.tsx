import { render, screen } from "@testing-library/react";
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
    const body = new URL(request.url).pathname.endsWith("/session") ? session : { items: [] };
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
  it("redirects / to the events page", async () => {
    renderAt("/");
    expect(await screen.findByRole("heading", { name: "Anlässe" })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/events");
  });

  it("shows the not-found page for an unknown path", async () => {
    renderAt("/nowhere");
    expect(await screen.findByText("Seite nicht gefunden")).toBeInTheDocument();
  });

  it("has the skip link as its first focusable element, and it targets main", async () => {
    renderAt("/events");
    await screen.findByRole("heading", { name: "Anlässe" });

    await userEvent.tab();
    const link = screen.getByRole("link", { name: "Zum Inhalt springen" });
    expect(link).toHaveFocus();
    expect(link).toHaveAttribute("href", "#main");
    expect(screen.getByRole("main")).toHaveAttribute("id", "main");
  });
});
