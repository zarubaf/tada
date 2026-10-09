import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { DocumentDiffPage } from "./DocumentDiffPage";

const DOCUMENT_ID = "d1";
const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const FACT_VENUE = "0199b8e0-0000-7000-8000-0000000000a2";
const FACT_DATE = "0199b8e0-0000-7000-8000-0000000000a1";
const FACT_GONE = "0199b8e0-0000-7000-8000-0000000000a9";
const FIELD_VENUE = "0199b8e0-0000-7000-8000-0000000000f2";

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

const sha = "0123456789abcdef".repeat(4);
const draft = (number: number) => ({
  id: `v${number}`,
  document_id: DOCUMENT_ID,
  number,
  kind: "draft",
  status: "review",
  sha256: sha,
  uploaded_by: "u2",
  created_at: "2030-05-19T08:00:00Z",
});

const diff = {
  lines: [
    { kind: "unchanged", old_line: 1, new_line: 1, text: "Der Anlass findet in Testwil statt." },
    { kind: "removed", old_line: 2, text: "Er dauert zwei Tage." },
    { kind: "added", new_line: 2, text: "Er dauert drei Tage." },
  ],
  facts: {
    changed: [{ fact_id: FACT_VENUE, from: 1, to: 2 }],
    added: [{ fact_id: FACT_DATE, version: 1 }],
    removed: [{ fact_id: FACT_GONE, version: 3 }],
  },
};

const failures = { diff: 0 };

function setup(path: string, answer: unknown = diff) {
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    if (pathname.endsWith("/session")) {
      return json(200, {
        user_id: "u1",
        display_name: "Anna Muster",
        organization: { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" },
        memberships: [],
      });
    }
    if (pathname.endsWith("/diff")) {
      if (failures.diff > 0) {
        failures.diff -= 1;
        return json(500, { type: "", code: "internal", title: "", status: 500, instance: "" });
      }
      return json(200, answer);
    }
    if (pathname.endsWith("/versions")) {
      return json(200, { items: [draft(1), draft(2), draft(3)] });
    }
    if (pathname.endsWith("/profile")) {
      return json(200, {
        facts: [
          {
            id: FACT_VENUE,
            field_id: FIELD_VENUE,
            field_key: "venue",
            state: "accepted",
            value: { type: "text", text: "Flugplatz Testwil" },
            version: 2,
            evidence: [],
            accepted_by: { kind: "member", id: "u1", channel: "web" },
            accepted_at: "2030-05-02T08:00:00Z",
          },
        ],
        proposals: [],
        open_questions: [],
      });
    }
    if (pathname.endsWith("/fields")) {
      return json(200, {
        items: [
          {
            id: FIELD_VENUE,
            key: "venue",
            label: { kind: "text", text: "Veranstaltungsort" },
            value_type: { type: "text" },
            value_schema: {},
            description: "",
            module: "core",
            status: "active",
          },
        ],
      });
    }
    if (pathname.endsWith(`/events/${EVENT_ID}`)) {
      return json(200, {
        id: EVENT_ID,
        key: "FLY28",
        name: "Fly-in Musterhausen",
        time_zone: "Europe/Zurich",
        version: 1,
        created_at: "2030-05-18T08:00:00Z",
      });
    }
    if (pathname.endsWith("/members")) {
      return json(200, { items: [] });
    }
    if (pathname.endsWith(`/documents/${DOCUMENT_ID}`)) {
      return json(200, {
        id: DOCUMENT_ID,
        event_id: EVENT_ID,
        readable_id: "DOC-001",
        name: "Konzept Flugtag",
        owner: "u1",
        created_at: "2030-05-18T08:00:00Z",
        version: 3,
        newest_version: draft(3),
      });
    }
    throw new Error(`unexpected ${request.method} ${pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", path);
  render(
    <Router>
      <Routes>
        <Route path="/documents/:documentId/diff">
          <DocumentDiffPage api={api} />
        </Route>
      </Routes>
    </Router>,
  );
}

afterEach(() => {
  failures.diff = 0;
  window.history.replaceState(null, "", "/");
});

const PATH = `/documents/${DOCUMENT_ID}/diff?from=v2&to=v3`;

describe("DocumentDiffPage", () => {
  it("names the two versions and moves focus to the heading", async () => {
    setup(PATH);

    const heading = await screen.findByRole("heading", {
      level: 2,
      name: "Unterschiede: Version 2 zu Version 3",
    });
    await waitFor(() => expect(heading).toHaveFocus());
    expect(screen.getByRole("link", { name: "Zurück zum Dokument" })).toHaveAttribute(
      "href",
      `/documents/${DOCUMENT_ID}`,
    );
  });

  it("shows each line with its change in words, not in color alone", async () => {
    setup(PATH);

    const table = await screen.findByRole("table", { name: "Zeilen" });
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(4);
    expect(within(rows[1] as HTMLElement).getByText("unverändert")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("entfernt")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("Er dauert zwei Tage.")).toBeInTheDocument();
    expect(within(rows[3] as HTMLElement).getByText("hinzugefügt")).toBeInTheDocument();
    expect(within(rows[3] as HTMLElement).getByText("Er dauert drei Tage.")).toBeInTheDocument();
  });

  it("lists the facts that changed, appeared and disappeared", async () => {
    setup(PATH);

    const facts = await screen.findByRole("region", { name: "Fakten" });
    expect(
      await within(facts).findByText("Veranstaltungsort: Version 1 zu Version 2"),
    ).toBeInTheDocument();
    // A fact that the profile does not list has no field label.
    expect(within(facts).getByText("Neu: Unbekannter Fakt, Version 1")).toBeInTheDocument();
    expect(within(facts).getByText("Entfallen: Unbekannter Fakt, Version 3")).toBeInTheDocument();
  });

  it("says that no fact changed", async () => {
    setup(PATH, { lines: [], facts: { changed: [], added: [], removed: [] } });

    expect(await screen.findByText("Die Entwürfe nennen dieselben Fakten.")).toBeInTheDocument();
    expect(screen.getByText("Die Zeilen sind gleich.")).toBeInTheDocument();
  });

  it("asks for two versions when the address names none", async () => {
    setup(`/documents/${DOCUMENT_ID}/diff`);

    expect(await screen.findByText(/zwei Entwurfsversionen/)).toBeInTheDocument();
  });

  it("shows the failure with a retry that moves focus to the heading", async () => {
    failures.diff = 1;
    setup(PATH);

    await userEvent.click(await screen.findByRole("button", { name: "Erneut versuchen" }));

    const heading = await screen.findByRole("heading", { level: 2, name: /Unterschiede/ });
    await waitFor(() => expect(heading).toHaveFocus());
  });
});
