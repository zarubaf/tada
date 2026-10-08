import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { PrivacyPage } from "./PrivacyPage";

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

const unavailable = () =>
  json(
    503,
    { type: "", code: "unavailable", title: "", status: 503, instance: "", request_id: "r1" },
    "application/problem+json",
  );

function setup(...responses: (() => Response)[]) {
  const calls: string[] = [];
  const fetch = async (request: Request) => {
    calls.push(`${request.method} ${new URL(request.url).pathname}`);
    const answer = responses.shift();
    if (!answer) {
      throw new Error("no more responses");
    }
    return answer();
  };
  render(<PrivacyPage api={createApi(fetch as unknown as typeof globalThis.fetch)} />);
  return calls;
}

describe("PrivacyPage", () => {
  it("shows the template while the organization has no own text", async () => {
    const calls = setup(() => json(200, { markdown: null, version: 1 }));

    expect(await screen.findByRole("heading", { level: 1, name: "Datenschutz" })).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Verantwortlich" })).toBeVisible();
    expect(calls).toEqual(["GET /api/v1/organization/privacy-notice"]);
  });

  it("states what the AI clients of members read and what a quote can show", async () => {
    setup(() => json(200, { markdown: null, version: 1 }));

    expect(await screen.findByText(/Lesewerkzeuge \(MCP\)/)).toBeVisible();
    expect(screen.getByText(/Fakten, Quellen mit ihren Textstellen und Dokumente/)).toBeVisible();
    // The quote statement holds for the web client too, so it has its own section.
    expect(await screen.findByRole("heading", { name: "Zitate als Belege" })).toBeVisible();
    expect(
      screen.getByText(/ganzen Text einer Eingabe zeigen, die für die ganze Organisation gilt/),
    ).toBeVisible();
    const clients = screen.getByRole("heading", { name: "KI-Clients der Mitglieder" });
    expect(clients.nextElementSibling?.textContent).not.toMatch(/Zitat/);
    expect(screen.getByText(/Zähler für Anmeldeversuche/)).toBeVisible();
    expect(screen.getByText(/nicht auf Schadsoftware/)).toBeVisible();
  });

  it("shows the own text of the organization instead of the template", async () => {
    setup(() => json(200, { markdown: "## Unser Text\n\nAlles klar.", version: 3 }));

    expect(await screen.findByRole("heading", { name: "Unser Text" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Verantwortlich" })).not.toBeInTheDocument();
  });

  it("does not render raw HTML of the own text", async () => {
    setup(() => json(200, { markdown: "Text <script>boom()</script>", version: 2 }));

    expect(await screen.findByText(/Text/)).toBeVisible();
    expect(document.querySelector("script")).toBeNull();
  });

  it("offers a retry after a failure and moves focus to the heading", async () => {
    setup(unavailable, () => json(200, { markdown: "## Unser Text", version: 2 }));

    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("button", { name: "Erneut versuchen" }));

    expect(await screen.findByRole("heading", { name: "Unser Text" })).toBeVisible();
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Datenschutz" })).toHaveFocus(),
    );
  });
});
