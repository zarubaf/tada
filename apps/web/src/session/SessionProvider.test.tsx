import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router, usePathname } from "../router/Router";
import { SessionProvider, useOptionalSession } from "./SessionProvider";

const orgId = "0199b8e0-0000-7000-8000-0000000000a1";
const membership = { organization_id: orgId, name: "Fliegergruppe Testwil", role: "member" };
const info = {
  user_id: "0199b8e0-0000-7000-8000-0000000000b1",
  display_name: "Anna Muster",
  organization: membership,
  memberships: [membership],
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string) {
  return json(
    status,
    { type: "", title: "", code, status, instance: "", request_id: "x" },
    "application/problem+json",
  );
}

/** A fake server: each call returns the next response and records the method and path. */
function fakeApi(...responses: Response[]) {
  const calls: string[] = [];
  const fetch = vi.fn(async (request: Request) => {
    calls.push(`${request.method} ${new URL(request.url).pathname}`);
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), calls, fetch };
}

function Probe({ api }: { api: ReturnType<typeof createApi> }) {
  // After a redirect to the public sign-in page, nobody is signed in.
  const session = useOptionalSession();
  if (!session) {
    return null;
  }
  return (
    <>
      <p>
        {session.user.displayName} in {session.organization?.name}
      </p>
      <button
        type="button"
        onClick={() => void api.GET("/api/v1/events", { params: { query: {} } })}
      >
        events
      </button>
      <button type="button" onClick={() => void session.signOut()}>
        out
      </button>
    </>
  );
}

function Path() {
  return <p data-testid="path">{usePathname()}</p>;
}

function renderAt(path: string, api: ReturnType<typeof createApi>) {
  window.history.replaceState(null, "", path);
  return render(
    <Router>
      <Path />
      <SessionProvider api={api}>
        <Probe api={api} />
      </SessionProvider>
    </Router>,
  );
}

function renderPublic(path: string, api: ReturnType<typeof createApi>) {
  window.history.replaceState(null, "", path);
  return render(
    <Router>
      <Path />
      <SessionProvider api={api}>
        <p>Öffentliche Seite</p>
      </SessionProvider>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("SessionProvider", () => {
  it("exposes the user and the organization of the session", async () => {
    const { api } = fakeApi(json(200, info));
    renderAt("/events", api);

    expect(await screen.findByText("Anna Muster in Fliegergruppe Testwil")).toBeInTheDocument();
    expect(screen.getByTestId("path")).toHaveTextContent("/events");
  });

  it("redirects to the sign-in page on 401", async () => {
    const { api } = fakeApi(problem(401, "unauthenticated"));
    renderAt("/events", api);

    await vi.waitFor(() => expect(screen.getByTestId("path")).toHaveTextContent("/sign-in"));
    expect(screen.queryByText(/Anna Muster/)).not.toBeInTheDocument();
  });

  it("redirects to the organization choice when the session has no organization", async () => {
    const { api } = fakeApi(json(200, { ...info, organization: null }));
    renderAt("/events", api);

    await vi.waitFor(() =>
      expect(screen.getByTestId("path")).toHaveTextContent("/choose-organization"),
    );
  });

  it("redirects to the organization choice on an organization-required problem", async () => {
    const { api } = fakeApi(
      json(200, info),
      problem(403, "organization-required"),
      json(200, { ...info, organization: null }),
    );
    renderAt("/events", api);

    await userEvent.click(await screen.findByRole("button", { name: "events" }));
    await vi.waitFor(() =>
      expect(screen.getByTestId("path")).toHaveTextContent("/choose-organization"),
    );
  });

  it("loads the session again on an organization-required problem", async () => {
    const other = { ...membership, organization_id: "o2", name: "Segelflugclub Musterhausen" };
    const removed = { ...info, organization: null, memberships: [other] };
    const { api, calls } = fakeApi(
      json(200, info),
      problem(403, "organization-required"),
      json(200, removed),
    );
    renderAt("/events", api);

    await userEvent.click(await screen.findByRole("button", { name: "events" }));
    await vi.waitFor(() =>
      expect(screen.getByTestId("path")).toHaveTextContent("/choose-organization"),
    );
    expect(calls).toEqual(["GET /api/v1/session", "GET /api/v1/events", "GET /api/v1/session"]);
  });

  it("redirects to the sign-in page when a later call gets 401", async () => {
    const { api } = fakeApi(json(200, info), problem(401, "unauthenticated"));
    renderAt("/events", api);

    await userEvent.click(await screen.findByRole("button", { name: "events" }));
    await vi.waitFor(() => expect(screen.getByTestId("path")).toHaveTextContent("/sign-in"));
  });

  it("signs out and goes to the sign-in page", async () => {
    const { api, calls } = fakeApi(json(200, info), new Response(null, { status: 204 }));
    renderAt("/events", api);

    await userEvent.click(await screen.findByRole("button", { name: "out" }));
    await vi.waitFor(() => expect(screen.getByTestId("path")).toHaveTextContent("/sign-in"));
    expect(calls).toEqual(["GET /api/v1/session", "POST /api/v1/sign-out"]);
  });

  it("sends the cookie with each request", async () => {
    const { api, fetch } = fakeApi(json(200, info));
    renderAt("/events", api);

    await screen.findByText(/Anna Muster/);
    expect(fetch.mock.calls[0]?.[0].credentials).toBe("same-origin");
  });

  it.each(["/sign-in", "/sign-in/link", "/invitation"])(
    "shows the public page %s without a session",
    async (path) => {
      const { api, calls } = fakeApi(problem(401, "unauthenticated"));
      renderPublic(path, api);

      expect(screen.getByText("Öffentliche Seite")).toBeInTheDocument();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      await Promise.resolve();
      expect(screen.getByText("Öffentliche Seite")).toBeInTheDocument();
      expect(screen.getByTestId("path")).toHaveTextContent(path);
    },
  );

  it("sends a signed-in member from the sign-in page to the events", async () => {
    const { api } = fakeApi(json(200, info));
    renderPublic("/sign-in", api);

    await vi.waitFor(() => expect(screen.getByTestId("path")).toHaveTextContent("/events"));
  });
});
