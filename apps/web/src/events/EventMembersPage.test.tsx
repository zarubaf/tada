import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi, type EventMembership } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { EventMembersPage } from "./EventMembersPage";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const ME = "0199b8e0-0000-7000-8000-0000000000b1";

function membership(
  userId: string,
  name: string,
  role: EventMembership["event_role"],
): EventMembership {
  return {
    user_id: userId,
    display_name: name,
    event_role: role,
    version: 1,
    created_at: "2030-05-18T08:00:00Z",
  };
}

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1" };
  return json(status, body, "application/problem+json");
}

interface Setup {
  organizationRole?: string;
  items?: EventMembership[];
  /** Answers by `METHOD /path-suffix`, for example `POST /remove`. */
  answers?: Record<string, () => Response>;
}

/** A fake server. It records `METHOD path` and the JSON body of each call. */
function setup({ organizationRole = "member", items = [], answers = {} }: Setup) {
  const calls: { call: string; body: unknown }[] = [];
  const organization = [
    { user_id: ME, display_name: "Anna Muster" },
    { user_id: "u2", display_name: "Bernd Beispiel" },
    { user_id: "u3", display_name: "Cäcilia Probst" },
  ];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: organizationRole };
  const session = {
    user_id: ME,
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    const body = request.method === "POST" ? await request.clone().json() : undefined;
    calls.push({ call: `${request.method} ${pathname}`, body });
    const answer = Object.entries(answers).find(([key]) => {
      const [method, suffix = ""] = key.split(" ");
      return request.method === method && pathname.endsWith(suffix);
    });
    if (answer) {
      return answer[1]();
    }
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (pathname.endsWith("/api/v1/members")) {
      return json(200, { items: organization });
    }
    if (pathname.endsWith("/memberships")) {
      return json(200, { items });
    }
    throw new Error(`unexpected ${request.method} ${pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${EVENT_ID}/members`);
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/events/:eventId/members">
            <EventMembersPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

const anna = membership(ME, "Anna Muster", "event-manager");
const bernd = membership("u2", "Bernd Beispiel", "event-contributor");

describe("EventMembersPage", () => {
  it("lists the members with their event role", async () => {
    setup({ items: [anna, bernd] });

    const table = await screen.findByRole("table", { name: "Mitglieder des Anlasses" });
    expect(within(table).getByText("Bernd Beispiel")).toBeInTheDocument();
    expect(
      within(table).getByRole("button", { name: /Rolle von Bernd Beispiel/ }),
    ).toHaveTextContent("Mitarbeit");
  });

  it("shows no actions to a member who cannot manage", async () => {
    setup({ items: [membership(ME, "Anna Muster", "event-contributor"), bernd] });

    const table = await screen.findByRole("table", { name: "Mitglieder des Anlasses" });
    expect(within(table).getAllByText("Mitarbeit")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: /Entfernen/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mitglied hinzufügen" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Rolle von/ })).not.toBeInTheDocument();
  });

  it("shows the actions to an organization admin who has no event role", async () => {
    setup({ organizationRole: "admin", items: [bernd] });

    expect(await screen.findByRole("button", { name: "Mitglied hinzufügen" })).toBeInTheDocument();
  });

  it("shows the message of a forbidden list", async () => {
    setup({ answers: { "GET /memberships": () => problem(403, "forbidden") } });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Sie haben keine Berechtigung für diese Aktion.",
    );
  });

  it("offers only the members without an event role, and adds one", async () => {
    const cäcilia = membership("u3", "Cäcilia Probst", "event-viewer");
    const { calls } = setup({
      items: [anna, bernd],
      answers: { "POST /memberships": () => json(201, cäcilia) },
    });

    await user.click(await screen.findByRole("button", { name: /Mitglied wählen/ }));
    expect(screen.queryByRole("option", { name: "Bernd Beispiel" })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Anna Muster" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "Cäcilia Probst" }));

    await user.click(screen.getByRole("button", { name: /Rolle$/ }));
    await user.click(screen.getByRole("option", { name: "Lesezugriff" }));
    await user.click(screen.getByRole("button", { name: "Mitglied hinzufügen" }));

    const table = await screen.findByRole("table", { name: "Mitglieder des Anlasses" });
    expect(await within(table).findByText("Cäcilia Probst")).toBeInTheDocument();
    expect(calls.find((c) => c.call.startsWith("POST"))?.body).toEqual({
      user_id: "u3",
      event_role: "event-viewer",
    });
  });

  it("changes a role with the record version", async () => {
    const { calls } = setup({
      items: [anna, { ...bernd, version: 4 }],
      answers: {
        "POST /change-role": () => json(200, { ...bernd, event_role: "event-viewer", version: 5 }),
      },
    });

    await user.click(await screen.findByRole("button", { name: /Rolle von Bernd Beispiel/ }));
    await user.click(screen.getByRole("option", { name: "Lesezugriff" }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Rolle von Bernd Beispiel/ })).toHaveTextContent(
        "Lesezugriff",
      ),
    );
    const call = calls.find((c) => c.call.endsWith("/change-role"));
    expect(call?.call).toBe(`POST /api/v1/events/${EVENT_ID}/memberships/u2/change-role`);
    expect(call?.body).toEqual({ event_role: "event-viewer", expected_version: 4 });
  });

  it("removes a member", async () => {
    const { calls } = setup({
      items: [anna, bernd],
      answers: { "POST /remove": () => new Response(null, { status: 204 }) },
    });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));

    const table = screen.getByRole("table", { name: "Mitglieder des Anlasses" });
    await waitFor(() =>
      expect(within(table).queryByText("Bernd Beispiel")).not.toBeInTheDocument(),
    );
    const call = calls.find((c) => c.call.endsWith("/remove"));
    expect(call?.body).toEqual({ expected_version: 1 });
  });

  it("shows the refusal to remove the last event manager", async () => {
    setup({
      items: [anna],
      answers: { "POST /remove": () => problem(409, "invalid-transition") },
    });

    await user.click(await screen.findByRole("button", { name: "Anna Muster entfernen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ein Anlass braucht mindestens eine Anlassleitung.",
    );
    expect(screen.getByText("Anna Muster")).toBeInTheDocument();
  });

  it("shows the refusal to demote the last event manager", async () => {
    setup({
      items: [anna],
      answers: { "POST /change-role": () => problem(409, "invalid-transition") },
    });

    await user.click(await screen.findByRole("button", { name: /Rolle von Anna Muster/ }));
    await user.click(screen.getByRole("option", { name: "Mitarbeit" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ein Anlass braucht mindestens eine Anlassleitung.",
    );
    expect(screen.getByRole("button", { name: /Rolle von Anna Muster/ })).toHaveTextContent(
      "Anlassleitung",
    );
  });
});
