import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi, type DocumentVersion } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { DocumentPage } from "./DocumentPage";

const DOCUMENT_ID = "d1";
const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

const sha = "0123456789abcdef".repeat(4);

function version(number: number, fileName: string, mediaType: string): DocumentVersion {
  return {
    id: `v${number}`,
    document_id: DOCUMENT_ID,
    number,
    kind: "upload",
    file_name: fileName,
    media_type: mediaType,
    size_bytes: 1_500,
    sha256: sha,
    uploaded_by: "u2",
    source_version_id: `v${number}`,
    created_at: "2030-05-18T08:00:00Z",
  };
}

const draft: DocumentVersion = {
  id: "v3",
  document_id: DOCUMENT_ID,
  number: 3,
  kind: "draft",
  status: "review",
  sha256: sha,
  uploaded_by: "u2",
  created_at: "2030-05-19T08:00:00Z",
};

/** The number of requests for the document that fail before one succeeds. */
const failures = { documents: 0 };

const DRAFT_LINK = "tada:fact/0199b8e0-0000-7000-8000-0000000000a1?v=1";

interface Options {
  /** True when the document reports that its facts changed; absent when the server does not know. */
  factsChanged?: boolean;
  /** The caller is an event manager: the list of the event memberships answers. */
  manager?: boolean;
  /** The answer of the approval. */
  approve?: () => Response;
  /** The rendering of the draft fails. */
  renderingFails?: boolean;
}

function setup(versions: DocumentVersion[], options: Options = {}) {
  const newest = versions[versions.length - 1] as DocumentVersion;
  const document = {
    id: DOCUMENT_ID,
    event_id: EVENT_ID,
    readable_id: "DOC-001",
    name: "Programm Flugtag.pdf",
    owner: "u1",
    created_at: "2030-05-18T08:00:00Z",
    version: versions.length,
    newest_version: newest,
    ...(options.factsChanged !== undefined && { facts_changed: options.factsChanged }),
  };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" },
    memberships: [{ organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" }],
  };
  const calls: { method: string; path: string; body: unknown }[] = [];
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    const body = request.method === "POST" ? await request.clone().json() : undefined;
    calls.push({ method: request.method, path: pathname, body });
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (pathname.endsWith("/approve")) {
      return (options.approve ?? (() => json(200, { ...newest, status: "approved" })))();
    }
    if (pathname.endsWith("/rendering")) {
      return options.renderingFails
        ? json(500, { type: "", code: "internal", title: "", status: 500, instance: "" })
        : json(200, {
            version: newest,
            draft: {
              markdown: `Der Ort ist [](${DRAFT_LINK}).\n\nEs gibt [Hinweise](https://example.org).`,
              lint_warnings: [],
              links: {
                [DRAFT_LINK]: {
                  kind: "fact",
                  fact_id: "0199b8e0-0000-7000-8000-0000000000a1",
                  version: 1,
                  state: "accepted",
                  value: { type: "text", text: "Flugplatz Testwil" },
                },
              },
            },
          });
    }
    if (pathname.endsWith(`/events/${EVENT_ID}/memberships`)) {
      return options.manager
        ? json(200, { items: [] })
        : json(403, { type: "", code: "forbidden", title: "", status: 403, instance: "" });
    }
    if (pathname.endsWith("/profile")) {
      return json(200, { facts: [], proposals: [], open_questions: [] });
    }
    if (pathname.endsWith("/fields")) {
      return json(200, { items: [] });
    }
    if (pathname.endsWith("/versions")) {
      return json(200, { items: versions });
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
      return json(200, {
        items: [{ user_id: "u2", display_name: "Bernd Beispiel", role: "member" }],
      });
    }
    if (pathname.endsWith(`/documents/${DOCUMENT_ID}`)) {
      if (failures.documents > 0) {
        failures.documents -= 1;
        return json(500, { type: "", code: "internal", title: "", status: 500, instance: "" });
      }
      return json(200, document);
    }
    throw new Error(`unexpected ${request.method} ${pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/documents/${DOCUMENT_ID}`);
  render(
    <Router>
      <Routes>
        <Route path="/documents/:documentId">
          <DocumentPage api={api} />
        </Route>
      </Routes>
    </Router>,
  );
  return { calls };
}

afterEach(() => {
  failures.documents = 0;
  window.history.replaceState(null, "", "/");
});

describe("DocumentPage", () => {
  it("lists the versions with hash prefix, uploader and a download link", async () => {
    setup([
      version(1, "Programm v1.pdf", "application/pdf"),
      version(2, "Programm v2.pdf", "application/pdf"),
    ]);

    const table = await screen.findByRole("table", { name: "Versionen" });
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(within(table).getAllByText("0123456789ab")).toHaveLength(2);
    expect(await within(table).findAllByText("Bernd Beispiel")).toHaveLength(2);
    expect(
      within(table).getByRole("link", { name: "Programm v2.pdf herunterladen, Version 2" }),
    ).toHaveAttribute("href", "/api/v1/document-versions/v2/content");
    const heading = screen.getByRole("heading", { level: 2, name: "Programm Flugtag.pdf" });
    await waitFor(() => expect(heading).toHaveFocus());
    // The event layout stays around the document.
    expect(screen.getByRole("link", { name: "Mitglieder" })).toBeInTheDocument();
  });

  it("moves focus to the heading when a retry succeeds", async () => {
    failures.documents = 1;
    setup([version(1, "Programm.pdf", "application/pdf")]);

    const retry = await screen.findByRole("button", { name: "Erneut versuchen" });
    expect(retry.closest("[role=alert]")).not.toHaveFocus();
    await userEvent.click(retry);

    const heading = await screen.findByRole("heading", { level: 2, name: "Programm Flugtag.pdf" });
    await waitFor(() => expect(heading).toHaveFocus());
  });

  it("moves focus to the message when a retry fails again", async () => {
    failures.documents = 2;
    setup([version(1, "Programm.pdf", "application/pdf")]);

    await userEvent.click(await screen.findByRole("button", { name: "Erneut versuchen" }));

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Erneut versuchen" }).closest("[role=alert]"),
      ).toHaveFocus(),
    );
  });

  it("states that files are not scanned for malware", async () => {
    setup([version(1, "Programm.pdf", "application/pdf")]);

    expect(
      await screen.findByText("Dateien werden nicht auf Schadsoftware geprüft."),
    ).toBeInTheDocument();
  });

  it("shows a draft as a row without a download link", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft]);

    const table = await screen.findByRole("table", { name: "Versionen" });
    expect(within(table).getByText("Entwurf (in Prüfung)")).toBeInTheDocument();
    expect(within(table).getAllByRole("link")).toHaveLength(1);
  });

  it("previews the newest version, also when it is a draft", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft]);

    const preview = await screen.findByRole("region", { name: "Vorschau der neuesten Version" });
    expect(await within(preview).findByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(
      within(preview).queryByRole("link", { name: "Vorschau in neuem Tab öffnen" }),
    ).toBeNull();
  });

  it("shows the failure of the draft with a retry", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft], { renderingFails: true });

    const preview = await screen.findByRole("region", { name: "Vorschau der neuesten Version" });
    expect(await within(preview).findByRole("button", { name: "Erneut versuchen" })).toBeVisible();
  });

  it("opens a PDF preview in a new tab and shows a text preview in a frame", async () => {
    setup([version(1, "Programm.pdf", "application/pdf")]);
    const link = await screen.findByRole("link", { name: "Vorschau in neuem Tab öffnen" });
    expect(link).toHaveAttribute("href", "/api/v1/document-versions/v1/content?disposition=inline");
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("previews a text file in a frame", async () => {
    setup([version(1, "Notizen.txt", "text/plain")]);

    const frame = await screen.findByTitle("Vorschau von Notizen.txt");
    expect(frame).toHaveAttribute("src", "/api/v1/document-versions/v1/content?disposition=inline");
  });

  it("previews a text type that carries a parameter", async () => {
    setup([version(1, "Notizen.txt", "text/plain; charset=utf-8")]);

    expect(await screen.findByTitle("Vorschau von Notizen.txt")).toBeInTheDocument();
  });

  it("offers no preview for another type", async () => {
    setup([
      version(
        1,
        "Budget.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
      ),
    ]);

    expect(await screen.findByText(/keine Vorschau/)).toBeInTheDocument();
  });

  it("shows „Fakten geändert“ when the document says so", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft], { factsChanged: true });

    expect(await screen.findByText("Fakten geändert")).toBeInTheDocument();
  });

  it.each([
    ["false", false],
    ["absent", undefined],
  ])("shows no „Fakten geändert“ when the value is %s", async (_name, factsChanged) => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft], {
      ...(factsChanged !== undefined && { factsChanged }),
    });

    await screen.findByRole("table", { name: "Versionen" });
    expect(screen.queryByText("Fakten geändert")).not.toBeInTheDocument();
  });

  it("offers „Version freigeben“ to an event manager for the newest draft", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft], { manager: true });

    expect(await screen.findByRole("button", { name: "Version freigeben" })).toBeInTheDocument();
  });

  it("offers no approval to a member who is no event manager", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft]);

    await screen.findByRole("table", { name: "Versionen" });
    await within(
      await screen.findByRole("region", { name: "Vorschau der neuesten Version" }),
    ).findByText("Flugplatz Testwil");
    expect(screen.queryByRole("button", { name: "Version freigeben" })).not.toBeInTheDocument();
  });

  it.each([
    ["an upload", [version(1, "Programm.pdf", "application/pdf")]],
    [
      "an approved draft",
      [version(1, "Programm.pdf", "application/pdf"), { ...draft, status: "approved" as const }],
    ],
  ])("offers no approval for the newest version when it is %s", async (_name, versions) => {
    setup(versions, { manager: true });

    await screen.findByRole("table", { name: "Versionen" });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Version freigeben" })).toBeNull(),
    );
  });

  it("approves with the record version of the document and announces the result", async () => {
    const { calls } = setup([version(1, "Programm.pdf", "application/pdf"), draft], {
      manager: true,
    });

    await userEvent.click(await screen.findByRole("button", { name: "Version freigeben" }));

    expect(await screen.findByText("Version 3 ist freigegeben.")).toBeInTheDocument();
    const approval = calls.find((call) => call.path.endsWith("/approve"));
    expect(approval).toMatchObject({
      method: "POST",
      path: "/api/v1/document-versions/v3/approve",
      body: { expected_version: 2 },
    });
    // The button left with the approval: focus goes to the heading of the document.
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 2, name: "Programm Flugtag.pdf" })).toHaveFocus(),
    );
  });

  it("keeps the button and its focus when the approval fails, and says why", async () => {
    setup([version(1, "Programm.pdf", "application/pdf"), draft], {
      manager: true,
      approve: () =>
        json(409, {
          type: "",
          code: "invalid-transition",
          title: "",
          status: 409,
          instance: "",
        }),
    });

    const button = await screen.findByRole("button", { name: "Version freigeben" });
    await userEvent.click(button);

    expect(await screen.findByRole("alert")).toHaveTextContent(/nicht mehr freigegeben werden/);
    expect(screen.getByRole("button", { name: "Version freigeben" })).toHaveFocus();
  });

  it("links the difference to the previous draft", async () => {
    const earlier = { ...draft, id: "v2", number: 2, status: "superseded" as const };
    setup([version(1, "Programm.pdf", "application/pdf"), earlier, { ...draft, number: 3 }]);

    const table = await screen.findByRole("table", { name: "Versionen" });
    expect(within(table).getByRole("link", { name: "Unterschiede zu Version 2" })).toHaveAttribute(
      "href",
      `/documents/${DOCUMENT_ID}/diff?from=v2&to=v3`,
    );
  });
});
