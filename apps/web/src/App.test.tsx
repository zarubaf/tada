import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { createApi } from "./api/client";

function emptyEvents() {
  const fetch = async () =>
    new Response(JSON.stringify({ items: [] }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  return createApi(fetch as unknown as typeof globalThis.fetch);
}

function renderAt(path: string) {
  window.history.replaceState(null, "", path);
  return render(<App api={emptyEvents()} />);
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("App", () => {
  it("redirects / to the events page", async () => {
    renderAt("/");
    expect(await screen.findByRole("heading", { name: "Anlässe" })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/events");
  });

  it("shows the not-found page for an unknown path", () => {
    renderAt("/nowhere");
    expect(screen.getByText("Seite nicht gefunden")).toBeInTheDocument();
  });
});
