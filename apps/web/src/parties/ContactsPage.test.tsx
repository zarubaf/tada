import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { ContactsPage } from "./ContactsPage";
import { ContactsRedirect } from "./ContactsRedirect";

const ME = "0199b8e0-0000-7000-8000-0000000000b1";

const beat = {
  id: "p1",
  local_id: "PER-001",
  name: "Beat Muster",
  email: "beat@example.org",
  phone: "+41 00 000 00 01",
  user_id: ME,
  version: 2,
  can_change: true,
};
const clara = {
  id: "p2",
  local_id: "PER-002",
  name: "Clara Probst",
  version: 1,
  can_change: true,
};
const generators = {
  id: "i1",
  local_id: "INS-001",
  name: "Testwil Generatoren AG",
  kind: "company",
  email: "info@example.org",
  version: 4,
  can_change: true,
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: unknown) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1", errors };
  return json(status, body, "application/problem+json");
}

interface Setup {
  kind?: "person" | "institution";
  /** The server says whether the caller can change the records. */
  canChange?: boolean;
  /** The organization role of the caller. */
  role?: "owner" | "admin" | "member";
  /** Answers by `METHOD /path-suffix`. */
  answers?: Record<string, (url: URL) => Response | Promise<Response>>;
  /** The fragment of the address, for a link to a row. */
  hash?: string;
}

/** A fake server that filters the list by `q`. It records `METHOD path?query` and the body. */
function setup({
  kind = "person",
  canChange = true,
  role = "member",
  answers = {},
  hash = "",
}: Setup = {}) {
  const calls: { call: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
  const session = {
    user_id: ME,
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const records = (kind === "person" ? [beat, clara] : [generators]).map((r) => ({
    ...r,
    can_change: canChange,
  }));
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const text = request.method === "GET" ? "" : await request.clone().text();
    calls.push({
      call: `${request.method} ${url.pathname}${url.search}`,
      body: text === "" ? undefined : JSON.parse(text),
    });
    const answer = Object.entries(answers).find(([key]) => {
      const [method, suffix = ""] = key.split(" ");
      return request.method === method && url.pathname.endsWith(suffix);
    });
    if (answer) {
      return answer[1](url);
    }
    if (url.pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (url.pathname.endsWith("/members")) {
      const q = url.searchParams.get("q")?.toLowerCase() ?? "";
      const members = [
        { user_id: ME, display_name: "Anna Muster", role: "owner", version: 1 },
        { user_id: "u2", display_name: "Bruno Beispiel", role: "member", version: 1 },
      ];
      return json(200, {
        items: members.filter((m) => m.display_name.toLowerCase().includes(q)),
      });
    }
    if (request.method === "GET" && url.pathname.endsWith("s")) {
      const q = url.searchParams.get("q")?.toLowerCase() ?? "";
      return json(200, { items: records.filter((r) => r.name.toLowerCase().includes(q)) });
    }
    throw new Error(`unexpected ${request.method} ${url.pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  const query = kind === "person" ? "" : "?tab=institutions";
  window.history.replaceState(null, "", `/contacts${query}${hash}`);
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/contacts">
            <ContactsPage api={api} />
          </Route>
          <Route path="/persons">
            <ContactsRedirect tab="persons" />
          </Route>
          <Route path="/institutions">
            <ContactsRedirect tab="institutions" />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

/** The „Neu“ button opens the form of a new record in a dialog. */
async function openCreate() {
  await user.click(screen.getByRole("button", { name: /^Neue? / }));
  return screen.findByRole("dialog");
}

const spaet = { ...clara, id: "p9", local_id: "PER-009", name: "Spät" };

/** An answer that the test sends when it calls `release`. */
function held() {
  let release: (response: Response) => void = () => {};
  const response = new Promise<Response>((resolve) => {
    release = resolve;
  });
  return { response, release };
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("ContactsPage", () => {
  it("lists persons and filters by name", async () => {
    const { calls } = setup();

    const table = await screen.findByRole("table", { name: "Personen" });
    expect(within(table).getByText("PER-001")).toBeInTheDocument();
    expect(within(table).getByText("Beat Muster")).toBeInTheDocument();
    expect(within(table).getByText("Clara Probst")).toBeInTheDocument();

    await user.type(screen.getByRole("searchbox", { name: "Nach Name suchen" }), "probst");

    await waitFor(() => expect(within(table).queryByText("Beat Muster")).not.toBeInTheDocument());
    expect(within(table).getByText("Clara Probst")).toBeInTheDocument();
    expect(calls.some((c) => c.call === "GET /api/v1/persons?q=probst")).toBe(true);
  });

  it("loads the next page, announces it and keeps focus when the last page is in", async () => {
    let requests = 0;
    const { calls } = setup({
      answers: {
        "GET /persons": () =>
          json(200, requests++ === 0 ? { items: [beat], next_cursor: "c2" } : { items: [clara] }),
      },
    });
    const table = await screen.findByRole("table", { name: "Personen" });

    await user.click(screen.getByRole("button", { name: "Weitere Einträge laden" }));

    expect(await within(table).findByText("Clara Probst")).toBeInTheDocument();
    expect(calls.some((c) => c.call === "GET /api/v1/persons?cursor=c2")).toBe(true);
    expect(screen.getByText("Weitere Einträge geladen.")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Weitere Einträge laden" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Kontakte" })).toHaveFocus();
  });

  it("ignores a next page that arrives after the search changed the list", async () => {
    const late = held();
    const { calls } = setup({
      answers: {
        "GET /persons": (url) => {
          const cursor = url.searchParams.get("cursor");
          if (!url.searchParams.has("q")) {
            return cursor ? late.response : json(200, { items: [beat], next_cursor: "old" });
          }
          return json(200, { items: [clara], next_cursor: "filtered" });
        },
      },
    });
    const table = await screen.findByRole("table", { name: "Personen" });
    const box = screen.getByRole("searchbox", { name: "Nach Name suchen" });

    await user.click(screen.getByRole("button", { name: "Weitere Einträge laden" }));
    await user.type(box, "probst");
    await within(table).findByText("Clara Probst");
    // The page on its way belongs to the old list: the button of the new list is not pending.
    expect(screen.getByRole("button", { name: "Weitere Einträge laden" })).not.toHaveAttribute(
      "aria-disabled",
    );
    late.release(json(200, { items: [spaet] }));
    await act(async () => {});

    expect(within(table).queryByText("Spät")).not.toBeInTheDocument();
    expect(within(table).queryByText("Beat Muster")).not.toBeInTheDocument();
    // The late page is the last of the old list, but it neither announces nor moves focus.
    expect(box).toHaveFocus();
    expect(screen.queryByText("Weitere Einträge geladen.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Weitere Einträge laden" }));
    expect(calls.at(-1)?.call).toBe("GET /api/v1/persons?q=probst&cursor=filtered");
  });

  it("loads the next page of the new list while a page of the old list is on its way", async () => {
    const late = held();
    const next = held();
    const { calls } = setup({
      answers: {
        "GET /persons": (url) => {
          const cursor = url.searchParams.get("cursor");
          if (!url.searchParams.has("q")) {
            return cursor ? late.response : json(200, { items: [beat], next_cursor: "old" });
          }
          return cursor ? next.response : json(200, { items: [clara], next_cursor: "filtered" });
        },
      },
    });
    const table = await screen.findByRole("table", { name: "Personen" });

    await user.click(screen.getByRole("button", { name: "Weitere Einträge laden" }));
    await user.type(screen.getByRole("searchbox", { name: "Nach Name suchen" }), "probst");
    await within(table).findByText("Clara Probst");
    const more = screen.getByRole("button", { name: "Weitere Einträge laden" });
    await user.click(more);
    expect(calls.at(-1)?.call).toBe("GET /api/v1/persons?q=probst&cursor=filtered");
    late.release(json(200, { items: [spaet] }));
    await act(async () => {});

    // The late page of the old list ends nothing: the page of the new list is still on its way.
    expect(more).toHaveAttribute("aria-disabled", "true");
    expect(within(table).queryByText("Spät")).not.toBeInTheDocument();
    next.release(
      json(200, { items: [{ ...clara, id: "p4", local_id: "PER-004", name: "Dora Probst" }] }),
    );
    expect(await within(table).findByText("Dora Probst")).toBeInTheDocument();
    expect(screen.getByText("Weitere Einträge geladen.")).toBeInTheDocument();
  });

  it("asks for no next page of the old list while the new list loads", async () => {
    const filtered = held();
    const { calls } = setup({
      answers: {
        "GET /persons": (url) => {
          if (!url.searchParams.has("q")) {
            return url.searchParams.has("cursor")
              ? json(200, { items: [spaet], next_cursor: "same" })
              : json(200, { items: [beat], next_cursor: "same" });
          }
          return url.searchParams.has("cursor")
            ? json(200, { items: [spaet], next_cursor: "same" })
            : filtered.response;
        },
      },
    });
    const table = await screen.findByRole("table", { name: "Personen" });
    await user.click(screen.getByRole("searchbox", { name: "Nach Name suchen" }));
    await user.paste("probst");
    await waitFor(() => expect(calls.at(-1)?.call).toBe("GET /api/v1/persons?q=probst"));

    // Both lists end on a cursor that reads the same.
    await user.click(screen.getByRole("button", { name: "Weitere Einträge laden" }));
    filtered.release(json(200, { items: [clara], next_cursor: "same" }));
    await within(table).findByText("Clara Probst");
    await act(async () => {});

    expect(calls.some((c) => c.call.includes("cursor="))).toBe(false);
    expect(within(table).queryByText("Spät")).not.toBeInTheDocument();
    expect(screen.queryByText("Weitere Einträge geladen.")).not.toBeInTheDocument();
  });

  it("searches the linked row in the new list when the search changes during the search", async () => {
    const late = held();
    const linked = { ...clara, id: "p3", local_id: "PER-003", name: "Carla Probst" };
    setup({
      hash: "#PER-003",
      answers: {
        "GET /persons": (url) => {
          const cursor = url.searchParams.get("cursor");
          if (!url.searchParams.has("q")) {
            return cursor ? late.response : json(200, { items: [beat], next_cursor: "c1" });
          }
          // The cursor of the new list reads the same as the one of the old list.
          return cursor
            ? json(200, { items: [linked] })
            : json(200, { items: [clara], next_cursor: "c1" });
        },
      },
    });
    const table = await screen.findByRole("table", { name: "Personen" });
    // One change, without the change event that user-event sends when the box loses focus to the
    // row: that event would arrive inside the commit that moves focus.
    fireEvent.change(screen.getByRole("searchbox", { name: "Nach Name suchen" }), {
      target: { value: "probst" },
    });

    const row = (await within(table).findByText("Carla Probst")).closest("tr");
    await waitFor(() => expect(row).toHaveFocus());
    late.release(json(200, { items: [spaet] }));
    await act(async () => {});

    expect(within(table).queryByText("Spät")).not.toBeInTheDocument();
    expect(screen.queryByText(/nicht gefunden/)).not.toBeInTheDocument();
  });

  it("lists institutions with their kind", async () => {
    setup({ kind: "institution" });

    const table = await screen.findByRole("table", { name: "Institutionen" });
    expect(within(table).getByText("INS-001")).toBeInTheDocument();
    expect(within(table).getByText("Firma")).toBeInTheDocument();
  });

  it("the create form shows field errors from validation-failed", async () => {
    const { calls } = setup({
      answers: {
        "POST /persons": () =>
          problem(422, "validation-failed", [
            { pointer: "/name", code: "empty" },
            { pointer: "/email", code: "shape" },
          ]),
      },
    });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Dora");
    await user.type(screen.getByRole("textbox", { name: "E-Mail-Adresse" }), "dora");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("Das ist keine gültige E-Mail-Adresse.")).toBeInTheDocument();
    expect(screen.getByText("Der Name hat 1 bis 200 Zeichen.")).toBeInTheDocument();
    expect(calls.find((c) => c.call === "POST /api/v1/persons")?.body).toEqual({
      name: "Dora",
      email: "dora",
    });
  });

  it("adds a created person to the list", async () => {
    setup({
      answers: {
        "POST /persons": () => json(201, { ...clara, id: "p3", local_id: "PER-003", name: "Dora" }),
      },
    });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Dora");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    expect(await screen.findByText("PER-003")).toBeInTheDocument();
  });

  it("offers editing only where the server says the caller can change the record", async () => {
    setup({ canChange: false });
    await screen.findByRole("table", { name: "Personen" });
    expect(screen.queryByRole("button", { name: /bearbeiten/i })).not.toBeInTheDocument();
  });

  it("changes a person with the version of the row", async () => {
    const { calls } = setup({
      answers: {
        "PATCH /persons/p1": () => json(200, { ...beat, name: "Beat Muster-Neu", version: 3 }),
      },
    });
    await screen.findByRole("table", { name: "Personen" });

    await user.click(screen.getByRole("button", { name: "Beat Muster bearbeiten" }));
    const name = screen.getByRole("textbox", { name: "Name (Pflichtfeld)" });
    expect(name).toHaveValue("Beat Muster");
    await user.clear(name);
    await user.type(name, "Beat Muster-Neu");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    expect(await screen.findByText("Beat Muster-Neu", { selector: "td" })).toBeInTheDocument();
    expect(calls.find((c) => c.call === "PATCH /api/v1/persons/p1")?.body).toEqual({
      name: "Beat Muster-Neu",
      email: "beat@example.org",
      phone: "+41 00 000 00 01",
      expected_version: 2,
    });
  });

  it("hides the account field from a member who is no owner or admin", async () => {
    const { calls } = setup({ role: "member" });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    expect(screen.queryByRole("combobox", { name: "Konto" })).not.toBeInTheDocument();
    expect(calls.some((c) => c.call.includes("/members"))).toBe(false);
  });

  it("lets an owner or admin link a new person to an account", async () => {
    const { calls } = setup({
      role: "admin",
      answers: { "POST /persons": () => json(201, { ...clara, id: "p3", name: "Dora" }) },
    });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Dora");
    await user.type(await screen.findByRole("combobox", { name: "Konto" }), "bru");
    await user.click(await screen.findByRole("option", { name: "Bruno Beispiel" }));
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    await screen.findByText("Dora", { selector: "td" });
    expect(calls.find((c) => c.call === "POST /api/v1/persons")?.body).toEqual({
      name: "Dora",
      user_id: "u2",
    });
  });

  it("does not link the member of an earlier choice after the text changed", async () => {
    const { calls } = setup({
      role: "admin",
      answers: { "POST /persons": () => json(201, { ...clara, id: "p3", name: "Dora" }) },
    });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Dora");
    const account = await screen.findByRole("combobox", { name: "Konto" });
    await user.type(account, "bru");
    await user.click(await screen.findByRole("option", { name: "Bruno Beispiel" }));
    await user.type(account, " und andere");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Erfassen", hidden: true }));

    await screen.findByText("Dora", { selector: "td" });
    // A new person without a link has no `user_id`.
    expect(calls.find((c) => c.call === "POST /api/v1/persons")?.body).toEqual({ name: "Dora" });
  });

  it("account picker searches members by q", async () => {
    const { calls } = setup({ role: "admin" });
    await screen.findByRole("table", { name: "Personen" });

    await openCreate();
    await user.type(await screen.findByRole("combobox", { name: "Konto" }), "bru");

    await screen.findByRole("option", { name: "Bruno Beispiel" });
    expect(screen.queryByRole("option", { name: "Anna Muster" })).not.toBeInTheDocument();
    const members = calls.filter((c) => c.call.includes("/members")).map((c) => c.call);
    expect(members).toContain("GET /api/v1/members?limit=6&q=bru");
    expect(members.some((call) => call.includes("limit=200"))).toBe(false);
  });

  it("lets an owner clear the account of a person in a change", async () => {
    const { calls } = setup({
      role: "owner",
      answers: { "PATCH /persons/p1": () => json(200, { ...beat, version: 3 }) },
    });
    await screen.findByRole("table", { name: "Personen" });
    await user.click(screen.getByRole("button", { name: "Beat Muster bearbeiten" }));

    const account = await screen.findByRole("combobox", { name: "Konto" });
    expect(account).toHaveValue("Verknüpft");
    await user.clear(account);
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call === "PATCH /api/v1/persons/p1")?.body).toMatchObject({
        user_id: null,
        expected_version: 2,
      }),
    );
  });

  it("keeps the account of a person in a change that leaves it alone", async () => {
    const { calls } = setup({
      role: "owner",
      answers: { "PATCH /persons/p1": () => json(200, { ...beat, version: 3 }) },
    });
    await screen.findByRole("table", { name: "Personen" });
    await user.click(screen.getByRole("button", { name: "Beat Muster bearbeiten" }));
    await screen.findByRole("combobox", { name: "Konto" });
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(calls.find((c) => c.call === "PATCH /api/v1/persons/p1")?.body).toMatchObject({
        user_id: ME,
      }),
    );
  });

  it("owner sees the Datenschutz section of a person, others do not", async () => {
    setup({
      role: "owner",
      answers: { "GET /persons/p1/mentions": () => json(200, { items: [] }) },
    });
    await user.click(await screen.findByRole("button", { name: "Beat Muster bearbeiten" }));
    expect(await screen.findByRole("heading", { name: "Datenschutz" })).toBeVisible();
    expect(await screen.findByRole("button", { name: "Export herunterladen" })).toBeVisible();
  });

  it("non-owner sees no Datenschutz section", async () => {
    setup({ role: "admin" });
    await user.click(await screen.findByRole("button", { name: "Beat Muster bearbeiten" }));
    await screen.findByRole("heading", { name: /bearbeiten/ });
    expect(screen.queryByRole("heading", { name: "Datenschutz" })).toBeNull();
  });

  it("shows entfernt for a redacted person and offers no edit", async () => {
    setup({
      role: "owner",
      answers: {
        "GET /persons": () =>
          json(200, { items: [{ ...beat, name: "", email: null, phone: null, redacted: true }] }),
      },
    });

    const table = await screen.findByRole("table", { name: "Personen" });
    expect(within(table).getByText("entfernt")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /bearbeiten/ })).not.toBeInTheDocument();
  });
});

describe("the tabs of Kontakte", () => {
  it("shows the persons by default and keeps no tab key in the address", async () => {
    setup();

    const tabs = await screen.findByRole("tablist", { name: "Art des Kontakts" });
    expect(within(tabs).getByRole("tab", { name: "Personen" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(await screen.findByRole("table", { name: "Personen" })).toBeInTheDocument();
    expect(window.location.search).toBe("");
  });

  it("opens the institutions from the query", async () => {
    setup({ kind: "institution" });

    expect(await screen.findByRole("table", { name: "Institutionen" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Institutionen" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("switches the tab, writes it to the address and removes it for the default", async () => {
    const { calls } = setup();
    await screen.findByRole("table", { name: "Personen" });

    await user.click(screen.getByRole("tab", { name: "Institutionen" }));
    expect(await screen.findByRole("table", { name: "Institutionen" })).toBeInTheDocument();
    expect(window.location.search).toBe("?tab=institutions");
    expect(calls.some((c) => c.call === "GET /api/v1/institutions")).toBe(true);

    await user.click(screen.getByRole("tab", { name: "Personen" }));
    expect(await screen.findByRole("table", { name: "Personen" })).toBeInTheDocument();
    expect(window.location.search).toBe("");
  });

  it("follows the back button between the tabs", async () => {
    setup();
    await screen.findByRole("table", { name: "Personen" });
    await user.click(screen.getByRole("tab", { name: "Institutionen" }));
    await screen.findByRole("table", { name: "Institutionen" });

    act(() => window.history.back());

    expect(await screen.findByRole("table", { name: "Personen" })).toBeInTheDocument();
  });

  it("drops the fragment of a row when the tab changes", async () => {
    setup({ kind: "institution", hash: "#INS-001" });
    await screen.findByRole("table", { name: "Institutionen" });

    await user.click(screen.getByRole("tab", { name: "Personen" }));

    await screen.findByRole("table", { name: "Personen" });
    expect(window.location.hash).toBe("");
  });

  it("clears the search when the tab changes", async () => {
    setup();
    await screen.findByRole("table", { name: "Personen" });
    await user.type(screen.getByRole("searchbox", { name: "Nach Name suchen" }), "probst");

    await user.click(screen.getByRole("tab", { name: "Institutionen" }));

    await screen.findByRole("table", { name: "Institutionen" });
    expect(screen.getByRole("searchbox", { name: "Nach Name suchen" })).toHaveValue("");
  });

  it("names the new button after the kind of the tab", async () => {
    setup();
    await screen.findByRole("table", { name: "Personen" });
    expect(screen.getByRole("button", { name: "Neue Person" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Institutionen" }));

    expect(await screen.findByRole("button", { name: "Neue Institution" })).toBeInTheDocument();
  });
});

describe("the form of a new record", () => {
  it("opens in a dialog and closes on cancel, with focus back on the button", async () => {
    setup();
    await screen.findByRole("table", { name: "Personen" });

    const dialog = await openCreate();
    expect(within(dialog).getByRole("heading", { name: "Person erfassen" })).toBeVisible();
    await user.click(within(dialog).getByRole("button", { name: "Abbrechen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(screen.getByRole("button", { name: "Neue Person" })).toHaveFocus());
  });

  it("closes after the save and says so", async () => {
    setup({
      answers: {
        "POST /persons": () => json(201, { ...clara, id: "p3", local_id: "PER-003", name: "Dora" }),
      },
    });
    await screen.findByRole("table", { name: "Personen" });
    await openCreate();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Dora");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("PER-003")).toBeInTheDocument();
    expect(screen.getByText(/Dora/, { selector: "[role=status]" })).toBeInTheDocument();
  });

  it("creates an institution on the institutions tab", async () => {
    const { calls } = setup({
      kind: "institution",
      answers: {
        "POST /institutions": () => json(201, { ...generators, id: "i2", name: "Neu AG" }),
      },
    });
    await screen.findByRole("table", { name: "Institutionen" });
    const dialog = await openCreate();
    expect(within(dialog).getByRole("heading", { name: "Institution erfassen" })).toBeVisible();
    await user.type(screen.getByRole("textbox", { name: "Name (Pflichtfeld)" }), "Neu AG");
    await user.click(screen.getByRole("button", { name: "Erfassen" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(calls.find((c) => c.call === "POST /api/v1/institutions")?.body).toMatchObject({
      name: "Neu AG",
      kind: "other",
    });
  });
});

describe("the old addresses", () => {
  const redirect = async (from: string) => {
    window.history.replaceState(null, "", from);
    render(
      <Router>
        <Routes>
          <Route path="/persons">
            <ContactsRedirect tab="persons" />
          </Route>
          <Route path="/institutions">
            <ContactsRedirect tab="institutions" />
          </Route>
        </Routes>
      </Router>,
    );
    await waitFor(() => expect(window.location.pathname).toBe("/contacts"));
  };

  it("sends /persons to the first tab", async () => {
    await redirect("/persons");
    expect(window.location.search).toBe("");
  });

  it("sends /institutions to its tab and keeps the fragment of a row", async () => {
    await redirect("/institutions#INS-001");
    expect(window.location.search).toBe("?tab=institutions");
    expect(window.location.hash).toBe("#INS-001");
  });

  it("keeps the other parts of the query", async () => {
    await redirect("/institutions?x=1");
    expect(new URLSearchParams(window.location.search).get("x")).toBe("1");
    expect(new URLSearchParams(window.location.search).get("tab")).toBe("institutions");
  });

  it("replaces the history entry, so that back does not return to the old address", async () => {
    window.history.replaceState(null, "", "/");
    const before = window.history.length;
    window.history.pushState(null, "", "/persons");
    await redirect("/persons");
    expect(window.history.length).toBe(before + 1);
  });
});
