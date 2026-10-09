import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { PartiesPage } from "./PartiesPage";

const ME = "0199b8e0-0000-7000-8000-0000000000b1";

const beat = {
  id: "p1",
  local_id: "PER-001",
  name: "Beat Muster",
  email: "beat@example.org",
  phone: "+41 00 000 00 01",
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
  /** Answers by `METHOD /path-suffix`. */
  answers?: Record<string, () => Response>;
}

/** A fake server that filters the list by `q`. It records `METHOD path?query` and the body. */
function setup({ kind = "person", canChange = true, answers = {} }: Setup = {}) {
  const calls: { call: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" };
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
      return answer[1]();
    }
    if (url.pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (request.method === "GET" && url.pathname.endsWith("s")) {
      const q = url.searchParams.get("q")?.toLowerCase() ?? "";
      return json(200, { items: records.filter((r) => r.name.toLowerCase().includes(q)) });
    }
    throw new Error(`unexpected ${request.method} ${url.pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  const path = kind === "person" ? "/persons" : "/institutions";
  window.history.replaceState(null, "", path);
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path={path}>
            <PartiesPage api={api} kind={kind} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

describe("PartiesPage", () => {
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
});
