import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { LegacyDocumentRedirect } from "./LegacyDocumentRedirect";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const DOCUMENT_ID = "0199b8e0-0000-7000-8000-0000000000d1";

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": status < 400 ? "application/json" : "application/problem+json" },
  });
}

function renderAt(path: string, status = 200) {
  const fetch = async (request: Request) => {
    expect(new URL(request.url).pathname).toBe(`/api/v1/documents/${DOCUMENT_ID}`);
    return status === 200
      ? json(200, { id: DOCUMENT_ID, event_id: EVENT_ID, name: "Konzept" })
      : json(status, { type: "", code: "internal", title: "", status, instance: "" });
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/start");
  window.history.pushState(null, "", path);
  render(
    <Router>
      <Routes>
        <Route path="/documents/:documentId">
          <LegacyDocumentRedirect api={api} />
        </Route>
        <Route path="/documents/:documentId/diff">
          <LegacyDocumentRedirect api={api} diff />
        </Route>
        <Route path="*">
          <p>Ziel</p>
        </Route>
      </Routes>
    </Router>,
  );
}

const at = () => window.location.pathname + window.location.search + window.location.hash;

afterEach(() => window.history.replaceState(null, "", "/"));

describe("LegacyDocumentRedirect", () => {
  it("sends a document to its place below the event", async () => {
    renderAt(`/documents/${DOCUMENT_ID}`);
    expect(await screen.findByText("Ziel")).toBeInTheDocument();
    expect(at()).toBe(`/events/${EVENT_ID}/documents/${DOCUMENT_ID}`);
  });

  it("keeps the versions of a comparison", async () => {
    renderAt(`/documents/${DOCUMENT_ID}/diff?from=v2&to=v3`);
    await waitFor(() =>
      expect(at()).toBe(`/events/${EVENT_ID}/documents/${DOCUMENT_ID}/diff?from=v2&to=v3`),
    );
  });

  it("offers a retry when the document does not load", async () => {
    renderAt(`/documents/${DOCUMENT_ID}`, 500);
    expect(await screen.findByRole("button", { name: "Erneut versuchen" })).toBeInTheDocument();
    expect(at()).toBe(`/documents/${DOCUMENT_ID}`);
  });
});
