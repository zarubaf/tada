import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { MembersPage } from "./MembersPage";

const ME = "0199b8e0-0000-7000-8000-0000000000b1";

const anna = {
  user_id: ME,
  display_name: "Anna Muster",
  email: "anna@example.org",
  role: "owner",
  version: 1,
};
const bernd = {
  user_id: "u2",
  display_name: "Bernd Beispiel",
  email: "bernd@example.org",
  role: "member",
  version: 3,
};
const invitation = {
  id: "i1",
  email: "clara@example.org",
  display_name: "Clara Probst",
  role: "member",
  created_at: "2030-05-18T08:00:00Z",
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string, errors?: unknown) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1", errors };
  return json(status, body, "application/problem+json");
}

interface Setup {
  role?: string;
  members?: unknown[];
  invitations?: unknown[];
  /** Members pages by the `cursor` query ("" is the first page). */
  pages?: Record<string, () => Response>;
  /** The organization of the session from the second `GET /session` on. */
  organizationAfter?: unknown;
  /** Answers by `METHOD /path-suffix`. */
  answers?: Record<string, () => Response | Promise<Response>>;
}

/** A fake server. It records `METHOD path` and the JSON body of each call. */
function setup({
  role = "owner",
  members = [anna, bernd],
  invitations = [],
  pages,
  organizationAfter,
  answers = {},
}: Setup) {
  const calls: { call: string; body: unknown }[] = [];
  let sessions = 0;
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
  const session = {
    user_id: ME,
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    const text = request.method === "POST" ? await request.clone().text() : "";
    const body = text === "" ? undefined : JSON.parse(text);
    calls.push({ call: `${request.method} ${pathname}`, body });
    const answer = Object.entries(answers).find(([key]) => {
      const [method, suffix = ""] = key.split(" ");
      return request.method === method && pathname.endsWith(suffix);
    });
    if (answer) {
      return answer[1]();
    }
    if (pathname.endsWith("/session")) {
      sessions += 1;
      const later = sessions > 1 && organizationAfter !== undefined;
      return json(200, later ? { ...session, organization: organizationAfter } : session);
    }
    if (pathname.endsWith("/api/v1/members")) {
      const page = pages?.[new URL(request.url).searchParams.get("cursor") ?? ""];
      return page ? page() : json(200, { items: members });
    }
    if (pathname.endsWith("/api/v1/invitations")) {
      return json(200, { items: invitations });
    }
    throw new Error(`unexpected ${request.method} ${pathname}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/settings/members");
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/settings/members">
            <MembersPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls, count: (call: string) => calls.filter((c) => c.call === call).length };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

async function roleOptions(): Promise<string[]> {
  await user.click(await screen.findByRole("button", { name: /Rolle$/ }));
  return screen.getAllByRole("option").map((option) => option.textContent ?? "");
}

describe("MembersPage", () => {
  it("offers an owner all three roles", async () => {
    setup({ role: "owner" });

    expect(await roleOptions()).toEqual(["Organisationsleitung", "Administration", "Mitglied"]);
  });

  it("offers an admin the roles admin and member only", async () => {
    setup({ role: "admin" });

    expect(await roleOptions()).toEqual(["Administration", "Mitglied"]);
  });

  it("shows a member the list without emails, invitations and invite form", async () => {
    const { calls } = setup({
      role: "member",
      members: [
        { ...anna, email: null },
        { ...bernd, email: null },
      ],
    });

    const table = await screen.findByRole("table", { name: "Mitglieder" });
    expect(within(table).getByText("Bernd Beispiel")).toBeInTheDocument();
    expect(within(table).queryByRole("columnheader", { name: "E-Mail" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Einladen" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /entfernen/ })).not.toBeInTheDocument();
    expect(calls.some((c) => c.call.endsWith("/invitations"))).toBe(false);
  });

  it("shows an admin the emails and the pending invitations", async () => {
    setup({ role: "admin", invitations: [invitation] });

    const members = await screen.findByRole("table", { name: "Mitglieder" });
    expect(within(members).getByText("bernd@example.org")).toBeInTheDocument();
    const invitations = await screen.findByRole("table", { name: "Offene Einladungen" });
    expect(within(invitations).getByText("clara@example.org")).toBeInTheDocument();
  });

  it("keeps the live regions in the page from the start", async () => {
    setup({});

    await screen.findByRole("table", { name: "Mitglieder" });
    await screen.findByText("Keine offenen Einladungen");
    expect(screen.getByRole("alert")).toBeEmpty();
    expect(screen.getByRole("status")).toBeEmpty();
  });

  it("invites a member, lists the invitation, announces it and moves focus to the name", async () => {
    const { calls } = setup({
      answers: { "POST /invitations": () => json(201, invitation) },
    });

    await user.type(await screen.findByRole("textbox", { name: /Name/ }), "Clara Probst");
    await user.type(screen.getByRole("textbox", { name: /E-Mail/ }), "clara@example.org");
    await user.click(screen.getByRole("button", { name: "Einladen" }));

    const table = await screen.findByRole("table", { name: "Offene Einladungen" });
    expect(await within(table).findByText("clara@example.org")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Einladung an Clara Probst gesendet.");
    await waitFor(() => expect(screen.getByRole("textbox", { name: /Name/ })).toHaveFocus());
    const post = calls.find((c) => c.call === "POST /api/v1/invitations");
    expect(post?.body).toMatchObject({
      email: "clara@example.org",
      display_name: "Clara Probst",
      role: "member",
    });
  });

  it("shows the field errors and moves focus to the first invalid field", async () => {
    setup({
      answers: {
        "POST /invitations": () =>
          problem(422, "validation-failed", [{ pointer: "/email", code: "invalid" }]),
      },
    });

    await user.type(await screen.findByRole("textbox", { name: /Name/ }), "Clara Probst");
    await user.type(screen.getByRole("textbox", { name: /E-Mail/ }), "kaputt");
    await user.click(screen.getByRole("button", { name: "Einladen" }));

    await waitFor(() => expect(screen.getByRole("textbox", { name: /E-Mail/ })).toHaveFocus());
    expect(screen.getByRole("textbox", { name: /E-Mail/ })).toBeInvalid();
  });

  it("asks before it removes a member, sends the version and moves focus to the heading", async () => {
    const { calls } = setup({
      answers: { "POST /remove": () => new Response(null, { status: 204 }) },
    });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Mitglied entfernen?" });
    await user.click(within(dialog).getByRole("button", { name: "Entfernen" }));

    const table = screen.getByRole("table", { name: "Mitglieder" });
    await waitFor(() =>
      expect(within(table).queryByText("Bernd Beispiel")).not.toBeInTheDocument(),
    );
    const call = calls.find((c) => c.call.endsWith("/remove"));
    expect(call?.call).toBe("POST /api/v1/members/u2/remove");
    expect(call?.body).toEqual({ expected_version: 3 });
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Mitglieder" })).toHaveFocus(),
    );
  });

  it("does nothing when the confirmation is cancelled", async () => {
    const { calls } = setup({});

    const button = await screen.findByRole("button", { name: "Bernd Beispiel entfernen" });
    await user.click(button);
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Abbrechen" }),
    );

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(calls.some((c) => c.call.endsWith("/remove"))).toBe(false);
    await waitFor(() => expect(button).toHaveFocus());
  });

  it("sends one request for a double press on the confirmation", async () => {
    let release: (response: Response) => void = () => {};
    const gate = new Promise<Response>((resolve) => {
      release = resolve;
    });
    const { calls } = setup({ answers: { "POST /remove": () => gate } });

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

  it("announces a refused removal in the live region that was there", async () => {
    setup({ answers: { "POST /remove": () => problem(403, "forbidden") } });

    const region = await screen.findByRole("alert");
    expect(region).toBeEmpty();
    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Entfernen" }),
    );

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Sie haben keine Berechtigung für diese Aktion.",
      ),
    );
    expect(screen.getByRole("alert")).toBe(region);
  });

  it("revokes an invitation after the confirmation", async () => {
    const { calls } = setup({
      invitations: [invitation],
      answers: { "POST /revoke": () => new Response(null, { status: 204 }) },
    });

    await user.click(
      await screen.findByRole("button", { name: "Einladung an Clara Probst widerrufen" }),
    );
    const dialog = await screen.findByRole("alertdialog", { name: "Einladung widerrufen?" });
    await user.click(within(dialog).getByRole("button", { name: "Widerrufen" }));

    await waitFor(() => expect(screen.queryByText("clara@example.org")).not.toBeInTheDocument());
    expect(calls.some((c) => c.call === "POST /api/v1/invitations/i1/revoke")).toBe(true);
  });

  it("shows the message of a failed list with a retry", async () => {
    setup({ answers: { "GET /members": () => problem(503, "unavailable") } });

    expect(await screen.findByRole("button", { name: "Erneut versuchen" })).toBeInTheDocument();
  });

  it("warns the member who removes themselves and leaves the organization afterwards", async () => {
    const { calls } = setup({
      organizationAfter: null,
      answers: { "POST /remove": () => new Response(null, { status: 204 }) },
    });

    await user.click(await screen.findByRole("button", { name: "Organisation verlassen" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Organisation verlassen?" });
    expect(dialog).toHaveTextContent("Sie entfernen sich selbst.");
    await user.click(within(dialog).getByRole("button", { name: "Organisation verlassen" }));

    await waitFor(() => expect(window.location.pathname).toBe("/choose-organization"));
    expect(calls.filter((c) => c.call.endsWith("/session")).length).toBeGreaterThan(1);
    expect(calls.some((c) => c.call === `POST /api/v1/members/${ME}/remove`)).toBe(true);
  });

  it("offers a plain member to leave, with the same warning, and no removal of others", async () => {
    setup({ role: "member", members: [{ ...anna, role: "member" }, bernd] });

    const leave = await screen.findByRole("button", { name: "Organisation verlassen" });
    expect(
      screen.queryByRole("button", { name: "Bernd Beispiel entfernen" }),
    ).not.toBeInTheDocument();
    await user.click(leave);
    expect(await screen.findByRole("alertdialog")).toHaveTextContent("Sie entfernen sich selbst.");
  });

  it("does not warn about leaving when an owner removes someone else", async () => {
    setup({});

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    expect(await screen.findByRole("alertdialog")).not.toHaveTextContent("sich selbst");
  });

  it("says why the last owner cannot be removed", async () => {
    setup({ answers: { "POST /remove": () => problem(409, "invalid-transition") } });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Entfernen" }),
    );

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("mindestens eine Organisationsleitung"),
    );
  });

  it("loads the list again after a version conflict", async () => {
    const { count } = setup({
      answers: { "POST /remove": () => problem(409, "record-version-conflict") },
    });

    await user.click(await screen.findByRole("button", { name: "Bernd Beispiel entfernen" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Entfernen" }),
    );

    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("neu geladen"));
    await waitFor(() => expect(count("GET /api/v1/members")).toBe(2));
  });

  it("loads the next page and moves focus to the heading when the button leaves", async () => {
    setup({
      pages: {
        "": () => json(200, { items: [anna], next_cursor: "c1" }),
        c1: () => json(200, { items: [bernd] }),
      },
    });

    await user.click(await screen.findByRole("button", { name: "Weitere Mitglieder laden" }));

    expect(await screen.findByText("Bernd Beispiel")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Weitere Mitglieder geladen.");
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Mitglieder" })).toHaveFocus(),
    );
    expect(
      screen.queryByRole("button", { name: "Weitere Mitglieder laden" }),
    ).not.toBeInTheDocument();
  });

  it("keeps the button mounted while the next page loads", async () => {
    let release: (response: Response) => void = () => {};
    const gate = new Promise<Response>((resolve) => {
      release = resolve;
    });
    setup({
      pages: {
        "": () => json(200, { items: [anna], next_cursor: "c1" }),
        c1: () => gate as unknown as Response,
      },
    });

    const button = await screen.findByRole("button", { name: "Weitere Mitglieder laden" });
    await user.click(button);

    expect(button).toBeInTheDocument();
    expect(button).toHaveAttribute("aria-disabled", "true");
    release(json(200, { items: [bernd] }));
    expect(await screen.findByText("Bernd Beispiel")).toBeInTheDocument();
  });

  it("keeps the loaded rows when a later page fails and retries from the same cursor", async () => {
    let attempts = 0;
    const { count } = setup({
      pages: {
        "": () => json(200, { items: [anna], next_cursor: "c1" }),
        c1: () => (++attempts === 1 ? problem(503, "unavailable") : json(200, { items: [bernd] })),
      },
    });

    const more = await screen.findByRole("button", { name: "Weitere Mitglieder laden" });
    await user.click(more);

    // The button stays: it keeps focus, and the alert region announces the failure.
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("nicht erreichbar"));
    expect(more).toHaveFocus();
    expect(screen.getByText("Anna Muster")).toBeInTheDocument();
    await user.click(more);

    expect(await screen.findByText("Bernd Beispiel")).toBeInTheDocument();
    expect(count("GET /api/v1/members")).toBe(3);
  });

  it("makes the invite button wait while the server asks to wait", async () => {
    setup({
      answers: {
        "POST /invitations": () =>
          new Response(
            JSON.stringify({
              type: "",
              code: "rate-limited",
              title: "",
              status: 429,
              instance: "",
              request_id: "r",
            }),
            {
              status: 429,
              headers: { "Content-Type": "application/problem+json", "Retry-After": "60" },
            },
          ),
      },
    });

    await user.type(await screen.findByRole("textbox", { name: /Name/ }), "Clara Probst");
    await user.type(screen.getByRole("textbox", { name: /E-Mail/ }), "clara@example.org");
    await user.click(screen.getByRole("button", { name: "Einladen" }));

    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("60 Sekunden"));
    const button = screen.getByRole("button", { name: "Einladen" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).toHaveFocus();
  });

  it("loads the invitations again when a revoked invitation is gone", async () => {
    const { count } = setup({
      invitations: [invitation],
      answers: { "POST /revoke": () => problem(404, "not-found") },
    });

    await user.click(
      await screen.findByRole("button", { name: "Einladung an Clara Probst widerrufen" }),
    );
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Widerrufen" }),
    );

    await waitFor(() => expect(count("GET /api/v1/invitations")).toBe(2));
  });
});
