import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { AccountPage } from "./AccountPage";

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** A fake server; after the sign-out everywhere, the session is gone. */
function setup(answer: () => Response) {
  const calls: string[] = [];
  let signedOut = false;
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "owner" };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const key = `${request.method} ${new URL(request.url).pathname}`;
    calls.push(key);
    if (key === "GET /api/v1/session") {
      return signedOut
        ? new Response(JSON.stringify({ code: "unauthenticated", status: 401, request_id: "r1" }), {
            status: 401,
            headers: { "Content-Type": "application/problem+json" },
          })
        : json(200, session);
    }
    if (key === "POST /api/v1/session/sign-out-everywhere") {
      const response = answer();
      signedOut = response.ok;
      return response;
    }
    throw new Error(`unexpected ${key}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", "/settings/account");
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/settings/account">
            <AccountPage api={api} />
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return calls;
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

describe("AccountPage", () => {
  it("signs out everywhere after a confirmation and opens the sign-in page", async () => {
    const calls = setup(() => new Response(null, { status: 204 }));
    await user.click(await screen.findByRole("button", { name: "Überall abmelden" }));
    expect(calls).not.toContain("POST /api/v1/session/sign-out-everywhere");
    const dialog = await screen.findByRole("alertdialog", { name: "Überall abmelden?" });
    await user.click(within(dialog).getByRole("button", { name: "Überall abmelden" }));

    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(calls).toContain("POST /api/v1/session/sign-out-everywhere");
  });

  it("stays and says why when the request fails", async () => {
    setup(() => json(503, { code: "unavailable", status: 503, request_id: "r1" }));
    await user.click(await screen.findByRole("button", { name: "Überall abmelden" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Überall abmelden" }));

    expect(window.location.pathname).toBe("/settings/account");
  });
});
