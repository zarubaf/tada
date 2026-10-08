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
  answers?: Record<string, () => Response | Promise<Response>>;
}

/** A fake server. It records `METHOD path` and the JSON body of each call. */
function setup({ organizationRole = "member", items = [], answers = {} }: Setup) {
  const calls: { call: string; body: unknown }[] = [];
  const organization = [
    { user_id: ME, display_name: "Anna Muster", role: "member", version: 1 },
    { user_id: "u2", display_name: "Bernd Beispiel", role: "member", version: 1 },
    { user_id: "u3", display_name: "Cäcilia Probst", role: "member", version: 1 },
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

  it("shows the message of a rate-limited action, not the general 4xx text", async () => {
    setup({
      items: [anna, bernd],
      answers: { "POST /change-role": () => problem(429, "rate-limited") },
    });

    await user.click(await screen.findByRole("button", { name: /Rolle von Bernd Beispiel/ }));
    await user.click(screen.getByRole("option", { name: "Lesezugriff" }));

    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Zu viele Anfragen."));
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
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Rolle vergeben" })).toHaveFocus(),
    );
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

  it("asks before it removes a member, and does nothing on cancel", async () => {
    const { calls } = setup({ items: [anna, bernd] });

    const button = await screen.findByRole("button", { name: "Bernd Beispiel entfernen" });
    await user.click(button);
    const dialog = await screen.findByRole("alertdialog", { name: "Bernd Beispiel entfernen?" });
    expect(dialog).not.toHaveTextContent("Sie entfernen Ihre eigene Rolle");
    await user.click(within(dialog).getByRole("button", { name: "Abbrechen" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(calls.some((c) => c.call.endsWith("/remove"))).toBe(false);
    await waitFor(() => expect(button).toHaveFocus());
  });

  it("removes a member after the confirmation and moves focus to the list heading", async () => {
    const { calls } = setup({
      items: [anna, bernd],
      answers: { "POST /remove": () => new Response(null, { status: 204 }) },
    });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Entfernen" }));

    const table = screen.getByRole("table", { name: "Mitglieder des Anlasses" });
    await waitFor(() =>
      expect(within(table).queryByText("Bernd Beispiel")).not.toBeInTheDocument(),
    );
    const call = calls.find((c) => c.call.endsWith("/remove"));
    expect(call?.body).toEqual({ expected_version: 1 });
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Mitglieder des Anlasses" })).toHaveFocus(),
    );
  });

  it("sends one request for a double press on the confirmation", async () => {
    let release: (response: Response) => void = () => {};
    const gate = new Promise<Response>((resolve) => {
      release = resolve;
    });
    const { calls } = setup({ items: [anna, bernd], answers: { "POST /remove": () => gate } });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    const confirm = within(await screen.findByRole("alertdialog")).getByRole("button", {
      name: "Entfernen",
    });
    await user.click(confirm);
    await user.click(confirm);
    release(new Response(null, { status: 204 }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(calls.filter((c) => c.call.endsWith("/remove"))).toHaveLength(1);
  });

  it("warns a manager who removes their own role", async () => {
    setup({ items: [anna, bernd] });

    await user.click(await screen.findByRole("button", { name: "Anna Muster entfernen" }));

    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      "Sie entfernen Ihre eigene Rolle als Anlassleitung.",
    );
  });

  it("announces the refusal to remove the last event manager in a live region that was there", async () => {
    setup({
      items: [anna],
      answers: { "POST /remove": () => problem(409, "invalid-transition") },
    });

    const region = await screen.findByRole("alert");
    expect(region).toBeEmpty();
    await user.click(await screen.findByRole("button", { name: "Anna Muster entfernen" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Entfernen" }),
    );

    await waitFor(() =>
      expect(region).toHaveTextContent("Ein Anlass braucht mindestens eine Anlassleitung."),
    );
    expect(screen.getByRole("alert")).toBe(region);
    expect(screen.getByText("Anna Muster")).toBeInTheDocument();
  });

  it("shows the refusal to demote the last event manager", async () => {
    setup({
      items: [anna],
      answers: { "POST /change-role": () => problem(409, "invalid-transition") },
    });

    const region = await screen.findByRole("alert");
    await user.click(await screen.findByRole("button", { name: /Rolle von Anna Muster/ }));
    await user.click(screen.getByRole("option", { name: "Mitarbeit" }));

    await waitFor(() =>
      expect(region).toHaveTextContent("Ein Anlass braucht mindestens eine Anlassleitung."),
    );
    expect(screen.getByRole("button", { name: /Rolle von Anna Muster/ })).toHaveTextContent(
      "Anlassleitung",
    );
  });

  it("loads the list again after a version conflict and says so", async () => {
    const { calls } = setup({
      items: [anna, bernd],
      answers: { "POST /change-role": () => problem(409, "record-version-conflict") },
    });

    await user.click(await screen.findByRole("button", { name: /Rolle von Bernd Beispiel/ }));
    await user.click(screen.getByRole("option", { name: "Lesezugriff" }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("Die Liste ist neu geladen."),
    );
    expect(
      calls.filter((c) => c.call === `GET /api/v1/events/${EVENT_ID}/memberships`),
    ).toHaveLength(2);
  });
});
