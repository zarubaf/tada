import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi, type Document } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { DocumentsPage } from "./DocumentsPage";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";

function makeDocument(id: string, readableId: string, name: string): Document {
  return {
    id,
    event_id: EVENT_ID,
    readable_id: readableId,
    name,
    owner: "u1",
    created_at: "2030-05-18T08:00:00Z",
    version: 1,
    newest_version: {
      id: `${id}-v1`,
      document_id: id,
      number: 1,
      kind: "upload",
      file_name: name,
      media_type: "application/pdf",
      size_bytes: 2_500_000,
      sha256: "0123456789abcdef".repeat(4),
      uploaded_by: "u1",
      source_version_id: `${id}-v1`,
      created_at: "2030-05-18T08:00:00Z",
    },
  };
}

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors: unknown[] = []) {
  return json(
    status,
    { type: "", code, title: "", status, instance: "", request_id: "r1", errors },
    "application/problem+json",
  );
}

const programm = makeDocument("d1", "DOC-001", "Programm Flugtag.pdf");
const budget = makeDocument("d2", "DOC-002", "Budget Übersicht.pdf");

/** A fake server for the list. It records the `q` of each call. */
function setup(lists: Record<string, Document[]> = { "": [programm, budget] }) {
  const queries: (string | null)[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    if (url.pathname.endsWith("/documents")) {
      const q = url.searchParams.get("q");
      queries.push(q);
      return json(200, { items: lists[q ?? ""] ?? [] });
    }
    throw new Error(`unexpected ${request.method} ${url.pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${EVENT_ID}/documents`);
  render(
    <Router>
      <Routes>
        <Route path="/events/:eventId/documents">
          <DocumentsPage api={api} />
        </Route>
      </Routes>
    </Router>,
  );
  return { queries };
}

function fileInput(): HTMLInputElement {
  const input = document.querySelector<HTMLInputElement>("input[type=file]");
  if (!input) {
    throw new Error("no file input");
  }
  return input;
}

const user = userEvent.setup({ delay: null });

afterEach(() => {
  window.history.replaceState(null, "", "/");
  vi.unstubAllGlobals();
});

describe("DocumentsPage", () => {
  it("lists the documents with a link to each", async () => {
    setup();

    const table = await screen.findByRole("table", { name: "Dokumente" });
    expect(within(table).getByText("DOC-001")).toBeInTheDocument();
    expect(within(table).getByRole("link", { name: "Budget Übersicht.pdf" })).toHaveAttribute(
      "href",
      "/documents/d2",
    );
    expect(within(table).getAllByText("2.5 MB")).toHaveLength(2);
  });

  it("searches by name and sends the trimmed text as q", async () => {
    const { queries } = setup({ "": [programm, budget], Budget: [budget] });

    await screen.findByRole("table");
    await user.type(screen.getByRole("searchbox", { name: "Dokumente suchen" }), "  Budget ");
    await user.click(screen.getByRole("button", { name: "Suchen" }));

    await waitFor(() => expect(screen.queryByText("DOC-001")).not.toBeInTheDocument());
    expect(queries).toEqual([null, "Budget"]);
  });

  it("resets a search without a match and moves focus to the search field", async () => {
    setup();

    await screen.findByRole("table");
    const field = screen.getByRole("searchbox", { name: "Dokumente suchen" });
    await user.type(field, "Nichts");
    await user.click(screen.getByRole("button", { name: "Suchen" }));
    expect(await screen.findByText("Keine Treffer für diese Filter.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Filter zurücksetzen" }));

    expect(await screen.findByRole("table")).toBeInTheDocument();
    expect(field).toHaveValue("");
    expect(field).toHaveFocus();
  });

  it("shows the empty state when the event has no document", async () => {
    setup({ "": [] });

    expect(await screen.findByText("Noch keine Dokumente")).toBeInTheDocument();
  });

  it("announces an upload and lists the new document", async () => {
    const lists = { "": [programm] };
    setup(lists);
    await screen.findByRole("table");
    lists[""] = [programm, budget];
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => json(201, budget)),
    );

    await user.upload(
      fileInput(),
      new File(["x"], "Budget Übersicht.pdf", { type: "application/pdf" }),
    );

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Hochgeladen: Budget Übersicht.pdf"),
    );
    expect(await screen.findByText("DOC-002")).toBeInTheDocument();
  });

  it("shows the message of a failed upload in the alert", async () => {
    setup();
    await screen.findByRole("table");
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        problem(422, "validation-failed", [{ pointer: "/file", code: "quota-exceeded" }]),
      ),
    );
    await user.upload(fileInput(), new File(["x"], "gross.pdf", { type: "application/pdf" }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Der Speicherplatz der Organisation reicht für diese Datei nicht aus.",
      ),
    );
    // The button stays in the page, so it keeps focus; the browser check proves it (jsdom focuses
    // the hidden input when a test uploads).
    expect(screen.getByRole("button", { name: "Datei hochladen" })).toBeInTheDocument();
  });
});
