import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { createApi } from "../api/client";

const ORG = {
  organization_id: "0199b8e0-0000-7000-8000-0000000000a1",
  name: "Fliegergruppe Testwil",
  role: "admin",
};
const SESSION = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  organization: ORG,
  memberships: [ORG],
};
const EVENT = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Musterhausen",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
};
const VENUE = "0199b8e0-0000-7000-8000-0000000000f1";
const NEW_FIELD = "0199b8e0-0000-7000-8000-0000000000f2";
const AI = { kind: "ai", id: "0199b8e0-0000-7000-8000-0000000000c1", channel: "api-token" };

const CS_OLD = "0199b8e0-0000-7000-8000-000000000c01";
const CS_NEW = "0199b8e0-0000-7000-8000-000000000c02";
const CS_ORG = "0199b8e0-0000-7000-8000-000000000c03";

const venueField = {
  id: VENUE,
  key: "venue",
  label: { kind: "text", text: "Veranstaltungsort" },
  value_type: { type: "text" },
  value_schema: {},
  description: "",
  module: "core",
  status: "active",
};

const evidence = {
  source_version_id: "0199b8e0-0000-7000-8000-0000000000e1",
  passage: { start: 10, end: 40, quote: "Wir treffen uns auf dem Flugplatz Testwil." },
  excerpt: {
    before: "Hallo zusammen. ",
    quote: "Wir treffen uns auf dem Flugplatz Testwil.",
    after: " Bis bald.",
  },
};

function proposal(id: string, extra: object) {
  return {
    id,
    depends_on: [],
    reason: "Die Mail nennt den Ort.",
    evidence: [evidence],
    status: "open",
    stale: false,
    overdue: false,
    routed_to_me: true,
    ...extra,
  };
}

const P_VENUE = "0199b8e0-0000-7000-8000-0000000001a1";
const P_FIELD = "0199b8e0-0000-7000-8000-0000000001a2";
const P_FACT = "0199b8e0-0000-7000-8000-0000000001a3";
const P_QUESTION = "0199b8e0-0000-7000-8000-0000000001a4";

/** A changeset with a field, a fact that needs the field, and a question. */
const newChangeset = {
  id: CS_NEW,
  event_id: EVENT.id,
  author: AI,
  source_version_id: evidence.source_version_id,
  created_at: "2028-03-05T09:00:00Z",
  proposals: [
    proposal(P_FIELD, {
      operation: {
        kind: "add-field-definition",
        id: NEW_FIELD,
        event_id: EVENT.id,
        key: "hangar",
        label: { kind: "text", text: "Hangar" },
        value_type: { type: "text" },
        description: "Der Hangar des Anlasses.",
        module: "core",
      },
    }),
    proposal(P_FACT, {
      depends_on: [P_FIELD],
      operation: {
        kind: "set-fact",
        event_id: EVENT.id,
        field_id: NEW_FIELD,
        state: "accepted",
        value: { type: "text", text: "Hangar 3" },
        expected_version: null,
      },
    }),
    proposal(P_QUESTION, {
      operation: {
        kind: "create-open-question",
        id: "0199b8e0-0000-7000-8000-0000000004a9",
        event_id: EVENT.id,
        text: "Wer schliesst den Hangar ab?",
        owner: SESSION.user_id,
      },
    }),
  ],
};

/** A changeset whose fact changed after the proposal. */
const oldChangeset = {
  id: CS_OLD,
  event_id: EVENT.id,
  author: AI,
  source_version_id: evidence.source_version_id,
  created_at: "2028-02-01T09:00:00Z",
  proposals: [
    proposal(P_VENUE, {
      stale: true,
      operation: {
        kind: "set-fact",
        event_id: EVENT.id,
        field_id: VENUE,
        state: "accepted",
        value: { type: "text", text: "Flugplatz Testwil" },
        expected_version: 1,
      },
      current: {
        fact_id: "0199b8e0-0000-7000-8000-0000000002a1",
        version: 2,
        state: "accepted",
        value: { type: "text", text: "Flugplatz Musterhausen" },
      },
    }),
  ],
};

const openItems = [
  {
    id: CS_OLD,
    event_id: EVENT.id,
    author: AI,
    created_at: oldChangeset.created_at,
    open_proposals: 1,
    stale: true,
  },
  {
    id: CS_NEW,
    event_id: EVENT.id,
    author: AI,
    created_at: newChangeset.created_at,
    open_proposals: 3,
    stale: false,
  },
  {
    id: CS_ORG,
    event_id: null,
    author: AI,
    created_at: "2028-03-06T09:00:00Z",
    open_proposals: 1,
    stale: false,
  },
];

const orgChangeset = {
  id: CS_ORG,
  event_id: null,
  author: AI,
  source_version_id: evidence.source_version_id,
  created_at: "2028-03-06T09:00:00Z",
  proposals: [
    proposal("0199b8e0-0000-7000-8000-0000000001b1", {
      operation: {
        kind: "create-event",
        id: "0199b8e0-0000-7000-8000-000000000009",
        key: "TEST30",
        name: "Tag der offenen Tür Testwil",
        time_zone: "Europe/Zurich",
      },
    }),
  ],
};

const CS_WORK = "0199b8e0-0000-7000-8000-000000000c04";
const P_PERSON = "0199b8e0-0000-7000-8000-0000000001c1";
const P_COMMITMENT = "0199b8e0-0000-7000-8000-0000000001c2";
const P_OTHER = "0199b8e0-0000-7000-8000-0000000001c3";
const NEW_PERSON = "0199b8e0-0000-7000-8000-0000000005a1";
const OLD_PERSON = "0199b8e0-0000-7000-8000-0000000005a2";

/** A new person who may exist already, a commitment of that person, and a proposal for another reviewer. */
const workChangeset = {
  id: CS_WORK,
  event_id: EVENT.id,
  author: AI,
  source_version_id: evidence.source_version_id,
  created_at: "2028-03-07T09:00:00Z",
  proposals: [
    proposal(P_PERSON, {
      overdue: true,
      duplicates: [{ id: OLD_PERSON, local_id: "PER-004", name: "Hans Beispiel", kind: "person" }],
      operation: {
        kind: "create-person",
        id: NEW_PERSON,
        name: "Hans Beispiel",
        email: "hans@example.org",
        phone: null,
      },
    }),
    proposal(P_COMMITMENT, {
      depends_on: [P_PERSON],
      operation: {
        kind: "create-commitment",
        id: "0199b8e0-0000-7000-8000-0000000005b1",
        event_id: EVENT.id,
        text: "Hans stellt den Hangar bereit.",
        promisor: { person: NEW_PERSON },
        owner: SESSION.user_id,
        workstream: null,
        due_date: "2028-04-01",
        condition: "Wenn die Gemeinde zustimmt",
      },
    }),
    proposal(P_OTHER, {
      routed_to_me: false,
      operation: {
        kind: "change-action-status",
        event_id: EVENT.id,
        action_id: "0199b8e0-0000-7000-8000-0000000005c1",
        status: "blocked",
        expected_version: 1,
      },
    }),
  ],
};

const CHANGESETS = new Map<string, unknown>([
  [CS_WORK, workChangeset],
  [CS_OLD, oldChangeset],
  [CS_NEW, newChangeset],
  [CS_ORG, orgChangeset],
]);

type Handler = (request: Request) => Response | Promise<Response>;

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

const problem = (code: string, status: number) =>
  json(
    status,
    {
      type: `https://github.com/zarubaf/tada/blob/main/doc/problems.md#${code}`,
      code,
      title: "A problem.",
      status,
      instance: "urn:uuid:01a11165-c361-77e9-a636-584f1ee6643c",
      request_id: "01a11165-c361-77e9-a636-584f1ee6643c",
    },
    "application/problem+json",
  );

const problemWith = (code: string, status: number, errors: object[]) =>
  json(
    status,
    {
      type: `https://github.com/zarubaf/tada/blob/main/doc/problems.md#${code}`,
      code,
      title: "A problem.",
      status,
      instance: "urn:uuid:01a11165-c361-77e9-a636-584f1ee6643c",
      request_id: "01a11165-c361-77e9-a636-584f1ee6643c",
      errors,
    },
    "application/problem+json",
  );

/** A fake server. Each route is `METHOD /path`; `overrides` replace routes of the default set. */
function fakeServer(overrides: Record<string, Handler> = {}) {
  const calls: { method: string; path: string; body: unknown }[] = [];
  const routes: Record<string, Handler> = {
    "GET /api/v1/session": () => json(200, SESSION),
    "GET /api/v1/events": () => json(200, { items: [EVENT] }),
    "GET /api/v1/changesets": () => json(200, { items: openItems }),
    "GET /api/v1/events/:id/fields": () => json(200, { items: [venueField] }),
    "GET /api/v1/events/:id/profile": () =>
      json(200, { facts: [], proposals: [], open_questions: [] }),
    "POST /api/v1/changesets/:id/apply": () =>
      json(200, {
        proposals: [{ id: P_FIELD, status: "accepted" }],
        open_questions: [],
        documents: [],
      }),
    "POST /api/v1/changesets/:id/reject": () =>
      json(200, {
        proposals: [{ id: P_QUESTION, status: "rejected" }],
        open_questions: [],
        documents: [],
      }),
    ...overrides,
  };
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const body = request.method === "POST" ? await request.clone().json() : undefined;
    const detail = /^\/api\/v1\/changesets\/([^/]+)$/.exec(url.pathname);
    const generic = url.pathname
      .replace(/^\/api\/v1\/changesets\/[^/]+\//, "/api/v1/changesets/:id/")
      .replace(/^\/api\/v1\/events\/[^/]+\//, "/api/v1/events/:id/");
    calls.push({ method: request.method, path: url.pathname, body });
    if (detail?.[1] && request.method === "GET") {
      const changeset = CHANGESETS.get(detail[1]);
      return changeset ? json(200, changeset) : problem("not-found", 404);
    }
    const handler = routes[`${request.method} ${generic}`];
    return handler ? handler(request) : json(200, { items: [] });
  };
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), calls };
}

function renderAt(path: string, server = fakeServer()) {
  window.history.replaceState(null, "", path);
  render(<App api={server.api} />);
  return server;
}

afterEach(() => window.history.replaceState(null, "", "/"));

const applyCalls = (server: ReturnType<typeof fakeServer>) =>
  server.calls.filter((call) => call.method === "POST" && call.path.endsWith("/apply"));

describe("the list", () => {
  it("shows the open changesets, oldest first, with the stale mark and the event", async () => {
    renderAt("/inbox");

    const list = await screen.findByRole("region", { name: "Offene Änderungen" });
    const links = await within(list).findAllByRole("link");
    expect(links).toHaveLength(3);
    expect(links[0]).toHaveTextContent("FLY28 Fly-in Musterhausen");
    expect(links[0]).toHaveTextContent("Veraltet");
    expect(links[1]).not.toHaveTextContent("Veraltet");
    expect(links[2]).toHaveTextContent("Organisation");
  });

  it("shows the count in the navigation from the same list", async () => {
    renderAt("/inbox");

    const nav = await screen.findByRole("navigation", { name: "Hauptnavigation" });
    expect(await within(nav).findByRole("link", { name: /Eingang/ })).toHaveTextContent("3 offen");
  });

  it("shows the empty state when nothing waits", async () => {
    renderAt("/inbox", fakeServer({ "GET /api/v1/changesets": () => json(200, { items: [] }) }));

    expect(await screen.findByText("Nichts zu prüfen")).toBeInTheDocument();
  });

  it("shows the problem message with a retry when the list fails", async () => {
    renderAt("/inbox", fakeServer({ "GET /api/v1/changesets": () => problem("unavailable", 503) }));

    expect(
      await screen.findByText(/Der Dienst ist im Moment nicht erreichbar/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Erneut versuchen" })).toBeInTheDocument();
  });
});

describe("the detail", () => {
  it("opens at /inbox/:changesetId with the comparison, the source and the reason", async () => {
    renderAt(`/inbox/${CS_OLD}`);

    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    expect(within(card).getByText("Flugplatz Musterhausen")).toBeInTheDocument();
    expect(within(card).getByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(within(card).getByText("Aktuell")).toBeInTheDocument();
    expect(
      within(card).getByText("Wir treffen uns auf dem Flugplatz Testwil."),
    ).toBeInTheDocument();
    expect(within(card).getByText("Die Mail nennt den Ort.")).toBeInTheDocument();
    expect(within(card).getByText(/Quellversion 0199b8e0/)).toBeInTheDocument();
  });

  it("shows an organization changeset without an event", async () => {
    renderAt(`/inbox/${CS_ORG}`);

    const card = await screen.findByRole("article", { name: "Anlass anlegen" });
    expect(within(card).getByText("Tag der offenen Tür Testwil")).toBeInTheDocument();
  });

  it("disables „Annehmen“ for a conflict and names the reason", async () => {
    renderAt(`/inbox/${CS_OLD}`);

    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    expect(within(card).getByRole("button", { name: /^Annehmen/ })).toBeDisabled();
    expect(within(card).getByRole("button", { name: /^Bearbeiten und annehmen/ })).toBeDisabled();
    expect(
      within(card).getByText(/Jemand hat den Wert nach dem Vorschlag geändert/),
    ).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: /^Ablehnen/ })).toBeEnabled();
  });

  it("does not name an AI client in the conflict text, because a member can be the author", async () => {
    renderAt(`/inbox/${CS_OLD}`);

    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    expect(within(card).queryByText(/KI-Client/)).not.toBeInTheDocument();
  });

  it("disables „Bearbeiten und annehmen“ for a value type that this client does not know", async () => {
    const CS_UNKNOWN = "0199b8e0-0000-7000-8000-000000000c08";
    CHANGESETS.set(CS_UNKNOWN, {
      ...oldChangeset,
      id: CS_UNKNOWN,
      proposals: [
        proposal("0199b8e0-0000-7000-8000-0000000001c1", {
          operation: {
            kind: "set-fact",
            event_id: EVENT.id,
            field_id: VENUE,
            state: "accepted",
            value: { type: "text", text: "Flugplatz Testwil" },
            expected_version: null,
          },
        }),
      ],
    });
    renderAt(
      `/inbox/${CS_UNKNOWN}`,
      fakeServer({
        "GET /api/v1/events/:id/fields": () =>
          json(200, { items: [{ ...venueField, value_type: { type: "duration" } }] }),
      }),
    );

    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    expect(within(card).getByRole("button", { name: /^Bearbeiten und annehmen/ })).toBeDisabled();
    expect(within(card).getByText("Diesen Wert können Sie hier nicht bearbeiten.")).toBeVisible();
  });

  it("shows the failure of the detail with a retry", async () => {
    renderAt(
      `/inbox/${CS_NEW}`,
      fakeServer({ "GET /api/v1/events/:id/fields": () => problem("unavailable", 503) }),
    );

    expect(
      await screen.findByText(/Der Dienst ist im Moment nicht erreichbar/),
    ).toBeInTheDocument();
  });
});

describe("a draft proposal", () => {
  const DRAFT_CS = "0199b8e0-0000-7000-8000-000000000c09";
  const draftProposal = (id: string, document: object, draft: object | null) =>
    proposal(id, {
      operation: {
        kind: "create-document-draft",
        event_id: EVENT.id,
        markdown: "# Titel",
        document,
      },
      draft,
    });
  const FACT_LINK = "tada:fact/0199b8e0-0000-7000-8000-0000000002a1?v=2";
  const HIDDEN_LINK = "tada:fact/0199b8e0-0000-7000-8000-0000000002a2?v=1";
  const rendering = {
    markdown: `# Konzept\n\nDer Ort ist [](${FACT_LINK}).\n\nEs kommen 500 Gäste.\n\nDas Konto ist [](${HIDDEN_LINK}).`,
    lint_warnings: [
      { line: 3, kind: "number" },
      { line: 3, kind: "robot" },
    ],
    links: {
      [FACT_LINK]: {
        kind: "fact",
        fact_id: "0199b8e0-0000-7000-8000-0000000002a1",
        version: 2,
        state: "assumption",
        value: { type: "text", text: "Flugplatz Testwil" },
      },
      [HIDDEN_LINK]: { kind: "hidden" },
    },
  };

  function server() {
    CHANGESETS.set(DRAFT_CS, {
      ...newChangeset,
      id: DRAFT_CS,
      proposals: [
        draftProposal(
          "0199b8e0-0000-7000-8000-0000000001d1",
          { kind: "new", id: "0199b8e0-0000-7000-8000-0000000000d1", name: "Ablauf Samstag" },
          rendering,
        ),
        draftProposal(
          "0199b8e0-0000-7000-8000-0000000001d2",
          {
            kind: "existing",
            document_id: "0199b8e0-0000-7000-8000-0000000000d2",
            expected_version: 2,
          },
          null,
        ),
      ],
    });
    return renderAt(`/inbox/${DRAFT_CS}`);
  }

  afterEach(() => CHANGESETS.delete("0199b8e0-0000-7000-8000-000000000c09"));

  it("shows a new draft by its title, a note and the lint warnings", async () => {
    server();

    const card = await screen.findByRole("article", { name: "Dokumentenentwurf „Ablauf Samstag“" });
    expect(
      within(card).getByText("Das Dokument entsteht, wenn Sie den Entwurf annehmen."),
    ).toBeInTheDocument();
  });

  it("shows the lint warnings of the draft with their lines", async () => {
    server();

    const card = await screen.findByRole("article", { name: "Dokumentenentwurf „Ablauf Samstag“" });
    expect(within(card).getByText("2 Hinweise der Prüfung")).toBeInTheDocument();
    expect(within(card).getByText("Zeile 3: Zahl ausserhalb eines Fakt-Links")).toBeInTheDocument();
    // The list of kinds is open: a kind that this client lacks gets a general text.
    expect(within(card).getByText("Zeile 3: Anderer Hinweis")).toBeInTheDocument();
  });

  it("renders the draft with the cited values and „entfernt“ for a hidden target", async () => {
    server();

    const card = await screen.findByRole("article", { name: "Dokumentenentwurf „Ablauf Samstag“" });
    expect(await within(card).findByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(within(card).getByText("Annahme")).toBeInTheDocument();
    expect(within(card).getByText("entfernt")).toBeInTheDocument();
    // The card has h3 for its title and h4 for its sections: the draft sits below them.
    expect(within(card).getByRole("heading", { level: 5, name: "Konzept" })).toBeInTheDocument();
  });

  it("links an existing document and renders without a draft", async () => {
    server();

    const card = await screen.findByRole("article", {
      name: "Entwurf für ein bestehendes Dokument",
    });
    expect(within(card).getByRole("link", { name: "Dokument öffnen" })).toHaveAttribute(
      "href",
      "/documents/0199b8e0-0000-7000-8000-0000000000d2",
    );
  });
});

describe("the selection", () => {
  it("selects the dependency of a selected proposal and says so", async () => {
    renderAt(`/inbox/${CS_NEW}`);

    await userEvent.click(
      await screen.findByRole("checkbox", { name: "Wert für „Hangar“ auswählen" }),
    );

    expect(
      screen.getByRole("checkbox", { name: "Feld „Hangar“ hinzufügen auswählen" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "Offene Frage anlegen auswählen" }),
    ).not.toBeChecked();
    expect(screen.getByText("1 Abhängigkeit mitgewählt.")).toBeInTheDocument();
    const summary = screen.getByRole("region", { name: "Auswahl" });
    expect(within(summary).getByText(/2 Vorschläge ausgewählt/)).toBeInTheDocument();
    expect(within(summary).getByText(/Davon 1 als Abhängigkeit/)).toBeInTheDocument();
  });

  it("deselects the dependent when the member deselects the dependency", async () => {
    renderAt(`/inbox/${CS_NEW}`);

    await userEvent.click(
      await screen.findByRole("checkbox", { name: "Wert für „Hangar“ auswählen" }),
    );
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Feld „Hangar“ hinzufügen auswählen" }),
    );

    expect(screen.getByRole("checkbox", { name: "Wert für „Hangar“ auswählen" })).not.toBeChecked();
    expect(screen.queryByRole("region", { name: "Auswahl" })).not.toBeInTheDocument();
  });

  it("applies the selection with its dependencies after the summary", async () => {
    const server = renderAt(`/inbox/${CS_NEW}`);

    await userEvent.click(
      await screen.findByRole("checkbox", { name: "Wert für „Hangar“ auswählen" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Auswahl annehmen" }));

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    const body = applyCalls(server)[0]?.body as { selected: string[]; edits: unknown[] };
    expect([...body.selected].sort()).toEqual([P_FIELD, P_FACT].sort());
    expect(body.edits).toEqual([]);
  });
});

describe("the keyboard", () => {
  it("applies the active proposal on A, with its dependency", async () => {
    const server = renderAt(`/inbox/${CS_NEW}`);
    const card = await screen.findByRole("article", { name: "Wert für „Hangar“" });
    await userEvent.click(within(card).getByText("Hangar 3"));

    await userEvent.keyboard("a");

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    const body = applyCalls(server)[0]?.body as { selected: string[] };
    expect([...body.selected].sort()).toEqual([P_FIELD, P_FACT].sort());
    expect(await screen.findByText("1 Vorschlag angenommen.")).toBeInTheDocument();
  });

  it("does not apply a conflict on A", async () => {
    const server = renderAt(`/inbox/${CS_OLD}`);
    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    await userEvent.click(within(card).getByText("Flugplatz Testwil"));

    await userEvent.keyboard("a");

    expect(applyCalls(server)).toHaveLength(0);
    expect(
      await screen.findByText("Dieser Vorschlag hat einen Konflikt. Er lässt sich nicht annehmen."),
    ).toBeInTheDocument();
  });

  it("does not open the edit form on E for a conflicting proposal", async () => {
    renderAt(`/inbox/${CS_OLD}`);
    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    await userEvent.click(within(card).getByText("Flugplatz Testwil"));

    await userEvent.keyboard("e");

    expect(within(card).queryByRole("textbox", { name: "Text" })).not.toBeInTheDocument();
  });

  it("applies the selection on A while focus is on a selection checkbox", async () => {
    const server = renderAt(`/inbox/${CS_NEW}`);
    const box = await screen.findByRole("checkbox", { name: "Offene Frage anlegen auswählen" });
    box.focus();

    await userEvent.keyboard(" ");
    expect(box).toBeChecked();
    await userEvent.keyboard("a");

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    expect(applyCalls(server)[0]?.body).toEqual({ selected: [P_QUESTION], edits: [], links: [] });
  });

  it("names the proposal, not a selection, when a single conflict blocks A", async () => {
    renderAt(`/inbox/${CS_OLD}`);
    const card = await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    await userEvent.click(within(card).getByText("Flugplatz Testwil"));

    await userEvent.keyboard("a");

    expect(await screen.findByText(/Dieser Vorschlag hat einen Konflikt/)).toBeInTheDocument();
    expect(screen.queryByText(/Die Auswahl enthält einen Konflikt/)).not.toBeInTheDocument();
  });

  it("keeps the shortcut hint out of the accessible name of the buttons", async () => {
    renderAt(`/inbox/${CS_NEW}`);
    const card = await screen.findByRole("article", { name: "Feld „Hangar“ hinzufügen" });

    expect(within(card).getByRole("button", { name: "Annehmen" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Ablehnen" })).toBeInTheDocument();
  });

  it("moves through the list on J and K and keeps focus in the list", async () => {
    renderAt(`/inbox/${CS_OLD}`);
    await screen.findByRole("article", { name: "Wert für „Veranstaltungsort“" });
    const heading = screen.getByRole("heading", { level: 1, name: "Eingang" });
    heading.focus();

    await userEvent.keyboard("j");

    await waitFor(() => expect(window.location.pathname).toBe(`/inbox/${CS_NEW}`));
    const list = screen.getByRole("region", { name: "Offene Änderungen" });
    await waitFor(() => expect(within(list).getAllByRole("link")[1]).toHaveFocus());

    await userEvent.keyboard("k");

    await waitFor(() => expect(window.location.pathname).toBe(`/inbox/${CS_OLD}`));
    await waitFor(() => expect(within(list).getAllByRole("link")[0]).toHaveFocus());
  });

  it("shows a visible hint for each key on the active proposal", async () => {
    renderAt(`/inbox/${CS_NEW}`);

    const card = await screen.findByRole("article", { name: "Feld „Hangar“ hinzufügen" });
    expect(within(card).getByText("A")).toBeVisible();
    expect(within(card).getByText("R")).toBeVisible();
    expect(screen.getByText("nächster Eintrag")).toBeVisible();
  });
});

describe("edit and accept", () => {
  async function openEditForm() {
    const server = renderAt(`/inbox/${CS_NEW}`);
    const card = await screen.findByRole("article", { name: "Wert für „Hangar“" });
    await userEvent.click(within(card).getByRole("button", { name: /^Bearbeiten und annehmen/ }));
    const form = await within(card).findByRole("textbox", { name: "Text" });
    return { server, card, form };
  }

  it("opens the form with the proposed value and focus on the first field", async () => {
    const { form } = await openEditForm();

    expect(form).toHaveValue("Hangar 3");
    expect(form).toHaveFocus();
  });

  it("does not take the shortcut keys while the member types", async () => {
    const { server, form } = await openEditForm();

    await userEvent.clear(form);
    await userEvent.type(form, "area");

    expect(form).toHaveValue("area");
    expect(applyCalls(server)).toHaveLength(0);
  });

  it("keeps the form open, shows the error and focuses the first invalid field", async () => {
    const { server, card, form } = await openEditForm();
    await userEvent.clear(form);

    await userEvent.click(
      within(form.closest("form") as HTMLElement).getByRole("button", {
        name: "Bearbeiten und annehmen",
      }),
    );

    expect(await within(card).findByText("Geben Sie einen Wert ein.")).toBeInTheDocument();
    await waitFor(() => expect(form).toHaveFocus());
    expect(form).toHaveAttribute("aria-invalid", "true");
    expect(applyCalls(server)).toHaveLength(0);
  });

  it("sends the edited value with the dependency", async () => {
    const { server, form } = await openEditForm();
    await userEvent.clear(form);
    await userEvent.type(form, "Hangar 5");

    await userEvent.click(
      within(form.closest("form") as HTMLElement).getByRole("button", {
        name: "Bearbeiten und annehmen",
      }),
    );

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    const body = applyCalls(server)[0]?.body as { selected: string[]; edits: unknown[] };
    expect([...body.selected].sort()).toEqual([P_FACT, P_FIELD].sort());
    expect({ edits: body.edits }).toEqual({
      edits: [
        {
          proposal_id: P_FACT,
          state: {
            state: "accepted",
            value: { type: "text", text: "Hangar 5" },
            approximate: false,
          },
        },
      ],
    });
  });

  it("returns focus to the edit button when the member cancels", async () => {
    const { card } = await openEditForm();

    await userEvent.click(within(card).getByRole("button", { name: "Abbrechen" }));

    await waitFor(() =>
      expect(within(card).getByRole("button", { name: /^Bearbeiten und annehmen/ })).toHaveFocus(),
    );
  });
});

describe("reject", () => {
  it("asks for a confirmation and rejects the proposal", async () => {
    const server = renderAt(`/inbox/${CS_NEW}`);
    const card = await screen.findByRole("article", { name: "Offene Frage anlegen" });

    await userEvent.click(within(card).getByRole("button", { name: /^Ablehnen/ }));
    const dialog = await screen.findByRole("alertdialog", { name: "Vorschläge ablehnen?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Ablehnen" }));

    await waitFor(() =>
      expect(server.calls.find((call) => call.path.endsWith("/reject"))?.body).toEqual({
        proposal_ids: [P_QUESTION],
      }),
    );
    expect(await screen.findByText("1 Vorschlag abgelehnt.")).toBeInTheDocument();
  });
});

describe("a failed apply", () => {
  it("keeps focus on the button and announces the message", async () => {
    renderAt(
      `/inbox/${CS_NEW}`,
      fakeServer({ "POST /api/v1/changesets/:id/apply": () => problem("unavailable", 503) }),
    );
    const card = await screen.findByRole("article", { name: "Offene Frage anlegen" });
    const accept = within(card).getByRole("button", { name: /^Annehmen/ });
    accept.focus();

    await userEvent.keyboard("{Enter}");

    const alert = await screen.findByText(/Der Dienst ist im Moment nicht erreichbar/);
    expect(alert).toBeInTheDocument();
    expect(accept).toHaveFocus();
  });
});

describe("work records", () => {
  it("a create-commitment proposal shows its condition", async () => {
    renderAt(`/inbox/${CS_WORK}`);

    const card = await screen.findByRole("article", { name: "Zusage anlegen" });
    expect(within(card).getByText("Bedingung")).toBeInTheDocument();
    expect(within(card).getByText("Wenn die Gemeinde zustimmt")).toBeInTheDocument();
    expect(within(card).getByText("Hans stellt den Hangar bereit.")).toBeInTheDocument();
    expect(within(card).getByText("Hans Beispiel")).toBeInTheDocument();
  });

  it("shows the new status of an action with the status label", async () => {
    renderAt(`/inbox/${CS_WORK}`);

    const card = await screen.findByRole("article", { name: "Status einer Aufgabe ändern" });
    expect(within(card).getByText("blockiert")).toBeInTheDocument();
  });

  it("an overdue proposal shows the overdue badge", async () => {
    renderAt(`/inbox/${CS_WORK}`);

    const card = await screen.findByRole("article", { name: "Person „Hans Beispiel“ anlegen" });
    expect(within(card).getByText("Überfällig")).toBeInTheDocument();
    const commitment = screen.getByRole("article", { name: "Zusage anlegen" });
    expect(within(commitment).queryByText("Überfällig")).not.toBeInTheDocument();
  });

  it("shows a proposal for another reviewer without controls", async () => {
    renderAt(`/inbox/${CS_WORK}`);

    const card = await screen.findByRole("article", { name: "Status einer Aufgabe ändern" });
    expect(within(card).getByText("Andere Prüfung")).toBeInTheDocument();
    expect(within(card).queryByRole("checkbox")).not.toBeInTheDocument();
    expect(within(card).queryByRole("button")).not.toBeInTheDocument();
  });

  it("lists the duplicate candidates with their readable ID", async () => {
    renderAt(`/inbox/${CS_WORK}`);

    expect(
      await screen.findByRole("checkbox", {
        name: "Bestehenden Eintrag verwenden: Hans Beispiel (PER-004)",
      }),
    ).not.toBeChecked();
  });

  it("choosing a duplicate sends a link in the apply request", async () => {
    const server = renderAt(`/inbox/${CS_WORK}`);

    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Bestehenden Eintrag verwenden: Hans Beispiel (PER-004)",
      }),
    );

    // The server refuses a link without the dependents, so the inbox selects them and says so.
    expect(screen.getByRole("checkbox", { name: "Zusage anlegen auswählen" })).toBeChecked();
    expect(
      screen.getByText("1 Vorschlag, der den Eintrag braucht, mitgewählt."),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Auswahl annehmen" }));

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    const body = applyCalls(server)[0]?.body as { selected: string[]; links: unknown[] };
    expect([...body.selected].sort()).toEqual([P_PERSON, P_COMMITMENT].sort());
    expect(body.links).toEqual([{ proposal_id: P_PERSON, record_id: OLD_PERSON }]);
  });

  it("sends no link when the member did not choose a duplicate", async () => {
    const server = renderAt(`/inbox/${CS_WORK}`);

    await userEvent.click(
      await screen.findByRole("checkbox", { name: "Person „Hans Beispiel“ anlegen auswählen" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Auswahl annehmen" }));

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    expect(applyCalls(server)[0]?.body).toMatchObject({ links: [] });
  });

  it("names an invalid link", async () => {
    const server = renderAt(
      `/inbox/${CS_WORK}`,
      fakeServer({
        "POST /api/v1/changesets/:id/apply": () =>
          problemWith("validation-failed", 422, [{ pointer: "/links/0", code: "invalid-link" }]),
      }),
    );

    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Bestehenden Eintrag verwenden: Hans Beispiel (PER-004)",
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Auswahl annehmen" }));

    await waitFor(() => expect(applyCalls(server)).toHaveLength(1));
    expect(await screen.findByText(/Dieser Eintrag passt nicht/)).toBeInTheDocument();
  });
});
