import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi, type Document, type Event } from "../api/client";
import { EventContext } from "../events/eventContext";
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
    redacted: false,
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
      redacted: false,
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
const ACTIVE_EVENT: Event = {
  id: EVENT_ID,
  key: "FLY28",
  name: "Fly-in Testwil",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
  status: "active",
  redacted: false,
};

function setup(
  lists: Record<string, Document[]> = { "": [programm, budget] },
  event: Event = ACTIVE_EVENT,
) {
  const queries: (string | null)[] = [];
  const hooks: {
    upload?: () => Response;
    /** Delays the answer of a list request. */
    list?: (q: string | null) => Promise<void> | undefined;
    /** The number of list requests that fail before one succeeds. */
    failures?: number;
  } = {};
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    if (url.pathname.endsWith("/documents") && request.method === "POST") {
      return hooks.upload?.() ?? problem(500, "internal");
    }
    if (url.pathname.endsWith("/documents")) {
      const q = url.searchParams.get("q");
      queries.push(q);
      if (hooks.failures) {
        hooks.failures -= 1;
        return problem(500, "internal");
      }
      const items = lists[q ?? ""] ?? [];
      await hooks.list?.(q);
      return json(200, { items });
    }
    throw new Error(`unexpected ${request.method} ${url.pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${EVENT_ID}/documents`);
  render(
    <Router>
      <Routes>
        <Route path="/events/:eventId/documents">
          <EventContext
            value={{ event, profile: { kind: "loading" }, reloadProfile: async () => true }}
          >
            <DocumentsPage api={api} />
          </EventContext>
        </Route>
      </Routes>
    </Router>,
  );
  return { queries, hooks };
}

function fileInput(): HTMLInputElement {
  const input = document.querySelector<HTMLInputElement>("input[type=file]");
  if (!input) {
    throw new Error("no file input");
  }
  return input;
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

describe("DocumentsPage", () => {
  it("puts the heading, the search and the upload in the toolbar row", async () => {
    setup();

    const heading = await screen.findByRole("heading", { level: 1, name: "Dokumente" });
    const row = heading.closest("[data-page-toolbar]") as HTMLElement;
    expect(row).not.toBeNull();
    expect(within(row).getByText("FLY28")).toBeInTheDocument();
    expect(within(row).getByRole("searchbox", { name: "Dokumente suchen" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Datei hochladen" })).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveAttribute("data-width", "flush");
  });

  it("marks a closed event with a chip that has an icon and the word", async () => {
    setup(undefined, { ...ACTIVE_EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" });

    const heading = await screen.findByRole("heading", { level: 1, name: "Dokumente" });
    const row = heading.closest("[data-page-toolbar]") as HTMLElement;
    const chip = within(row).getByText("Abgeschlossen").closest("[data-variant]");
    expect(chip?.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("shows a draft as a chip with its status, as muted text for the version", async () => {
    const draft = makeDocument("d3", "DOC-003", "Ablauf Samstag");
    draft.newest_version = {
      ...draft.newest_version,
      kind: "draft",
      status: "review",
      media_type: null,
      size_bytes: null,
      file_name: null,
    };
    setup({ "": [draft] });

    const table = await screen.findByRole("table", { name: "Dokumente" });
    const chip = within(table).getByText("Entwurf").closest("[data-variant]");
    expect(chip?.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(within(table).getByText("in Prüfung").closest("[data-variant]")).not.toBeNull();
  });

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

  it("offers no upload for a closed event, and still lists the documents", async () => {
    setup(undefined, { ...ACTIVE_EVENT, status: "closed", closed_at: "2030-07-01T10:00:00Z" });

    const table = await screen.findByRole("table", { name: "Dokumente" });
    expect(within(table).getByText("DOC-001")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Datei hochladen" })).not.toBeInTheDocument();
    expect(document.querySelector("input[type=file]")).toBeNull();
  });

  it("shows a label for the file type, not the media type", async () => {
    setup();

    const table = await screen.findByRole("table", { name: "Dokumente" });
    expect(within(table).getAllByText("PDF").length).toBeGreaterThan(0);
    expect(within(table).queryByText("application/pdf")).not.toBeInTheDocument();
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

  it("ignores an older answer that arrives after a newer one", async () => {
    let releaseOld: () => void = () => {};
    const { hooks } = setup({ "": [programm], Alt: [programm], Neu: [budget] });
    await screen.findByRole("table");
    hooks.list = (q) =>
      q === "Alt" ? new Promise<void>((resolve) => (releaseOld = resolve)) : undefined;
    const field = screen.getByRole("searchbox", { name: "Dokumente suchen" });

    await user.type(field, "Alt");
    await user.click(screen.getByRole("button", { name: "Suchen" }));
    await user.clear(field);
    await user.type(field, "Neu");
    await user.click(screen.getByRole("button", { name: "Suchen" }));
    expect(await screen.findByText("DOC-002")).toBeInTheDocument();
    releaseOld();

    await waitFor(() => expect(screen.getByText("DOC-002")).toBeInTheDocument());
    expect(screen.queryByText("DOC-001")).not.toBeInTheDocument();
  });

  it("moves focus to the heading when a retry succeeds", async () => {
    const { hooks } = setup({ "": [programm, budget], Budget: [budget] });
    await screen.findByRole("table");
    hooks.failures = 1;
    await user.type(screen.getByRole("searchbox", { name: "Dokumente suchen" }), "Budget");
    await user.click(screen.getByRole("button", { name: "Suchen" }));
    const retry = await screen.findByRole("button", { name: "Erneut versuchen" });
    expect(retry.closest("[role=alert]")).not.toHaveFocus();

    await user.click(retry);

    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Dokumente" })).toHaveFocus(),
    );
  });

  it("moves focus to the message when a retry fails again", async () => {
    const { hooks } = setup();
    await screen.findByRole("table");
    hooks.failures = 2;
    await user.type(screen.getByRole("searchbox", { name: "Dokumente suchen" }), "x");
    await user.click(screen.getByRole("button", { name: "Suchen" }));
    await user.click(await screen.findByRole("button", { name: "Erneut versuchen" }));

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Erneut versuchen" }).closest("[role=alert]"),
      ).toHaveFocus(),
    );
  });

  it("shows the empty state when the event has no document", async () => {
    setup({ "": [] });

    expect(await screen.findByText("Noch keine Dokumente")).toBeInTheDocument();
  });

  it("announces an upload and lists the new document", async () => {
    const lists = { "": [programm] };
    const { hooks } = setup(lists);
    await screen.findByRole("table");
    lists[""] = [programm, budget];
    hooks.upload = () => json(201, budget);

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
    const { hooks } = setup();
    await screen.findByRole("table");
    hooks.upload = () =>
      problem(422, "validation-failed", [{ pointer: "/file", code: "quota-exceeded" }]);
    await user.upload(fileInput(), new File(["x"], "gross.pdf", { type: "application/pdf" }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Der Speicherplatz der Organisation reicht für diese Datei nicht aus.",
      ),
    );
    // The page shows the text once in the alert region and once in the message, which takes no
    // focus and no role: the button stays in the page, so it keeps focus (the browser check proves
    // it; jsdom focuses the hidden input when a test uploads).
    expect(screen.getAllByText(/Speicherplatz der Organisation/)).toHaveLength(2);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Datei hochladen" })).toBeInTheDocument();
  });
});
