import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { createApi, type Event, type Problem } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { EventsPage } from "./EventsPage";

// Invented fixtures with long German names and umlauts (doc/design/principles.md).
function event(key: string, name: string): Event {
  return {
    id: `0199b8e0-0000-7000-8000-${key.padStart(12, "0")}`,
    key,
    name,
    time_zone: "Europe/Zurich",
    version: 1,
    created_at: "2030-05-18T08:00:00Z",
  };
}

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

/** A fake server: each call returns the next response and records the request URL. */
function fakeApi(...responses: Response[]) {
  const urls: string[] = [];
  const fetch = vi.fn(async (request: Request) => {
    urls.push(request.url);
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), urls };
}

describe("EventsPage", () => {
  it("lists the events in a table", async () => {
    const { api } = fakeApi(
      json(200, { items: [event("TEST30", "Tag der offenen Tür Testwil-Ämmerlibüel")] }),
    );
    render(<EventsPage api={api} />);

    const table = await screen.findByRole("table", { name: "Anlässe" });
    expect(within(table).getByText("TEST30")).toBeInTheDocument();
    expect(within(table).getByText("Tag der offenen Tür Testwil-Ämmerlibüel")).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Kürzel" })).toBeInTheDocument();
  });

  it("shows the empty state without events", async () => {
    const { api } = fakeApi(json(200, { items: [] }));
    render(<EventsPage api={api} />);

    expect(await screen.findByText("Noch keine Anlässe erfasst")).toBeInTheDocument();
  });

  it("shows the message of the problem code and the request ID, and retries", async () => {
    const problem: Problem = {
      type: "https://github.com/zarubaf/tada/blob/main/doc/problems.md#unavailable",
      code: "unavailable",
      title: "A dependency is unavailable. The client can retry.",
      status: 503,
      detail: "This English text never appears.",
      instance: "urn:uuid:01a11165-c361-77e9-a636-584f1ee6643c",
      request_id: "01a11165-c361-77e9-a636-584f1ee6643c",
    };
    const { api } = fakeApi(
      json(503, problem, "application/problem+json"),
      json(200, { items: [event("FLY28", "Fly-in 2028")] }),
    );
    render(<EventsPage api={api} />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Der Dienst ist im Moment nicht erreichbar.");
    expect(alert).toHaveTextContent("Fehler-ID: 01a11165-c361-77e9-a636-584f1ee6643c");
    expect(alert).not.toHaveTextContent("This English text never appears.");

    await userEvent.click(within(alert).getByRole("button", { name: "Erneut versuchen" }));
    expect(await screen.findByText("FLY28")).toBeInTheDocument();
  });

  it("shows the general message of the status class for an unknown code", async () => {
    const { api } = fakeApi(
      json(
        418,
        { code: "a-new-code", status: 418, request_id: "x", type: "", title: "", instance: "" },
        "application/problem+json",
      ),
    );
    render(<EventsPage api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("Die Anfrage ist ungültig.");
  });

  it("loads the next page with the cursor", async () => {
    const { api, urls } = fakeApi(
      json(200, { items: [event("AA", "Erster Anlass")], next_cursor: "QUEgMDE" }),
      json(200, { items: [event("BB", "Zweiter Anlass")] }),
    );
    render(<EventsPage api={api} />);

    await userEvent.click(await screen.findByRole("button", { name: "Weitere Anlässe laden" }));
    expect(await screen.findByText("BB")).toBeInTheDocument();
    expect(screen.getByText("AA")).toBeInTheDocument();
    expect(urls[1]).toContain("cursor=QUEgMDE");
    expect(screen.queryByRole("button", { name: "Weitere Anlässe laden" })).not.toBeInTheDocument();
  });

  it("keeps focus on the button while more pages remain", async () => {
    const { api } = fakeApi(
      json(200, { items: [event("AA", "Erster Anlass")], next_cursor: "c1" }),
      json(200, { items: [event("BB", "Zweiter Anlass")], next_cursor: "c2" }),
    );
    render(<EventsPage api={api} />);

    await userEvent.click(await screen.findByRole("button", { name: "Weitere Anlässe laden" }));

    expect(await screen.findByText("BB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Weitere Anlässe laden" })).toHaveFocus();
  });

  it("moves focus to the heading when the last page arrives", async () => {
    const { api } = fakeApi(
      json(200, { items: [event("AA", "Erster Anlass")], next_cursor: "c1" }),
      json(200, { items: [event("BB", "Zweiter Anlass")] }),
    );
    render(<EventsPage api={api} />);

    await userEvent.click(await screen.findByRole("button", { name: "Weitere Anlässe laden" }));

    expect(await screen.findByText("BB")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Anlässe" })).toHaveFocus();
  });
});

describe("EventsPage toolbar", () => {
  function signedIn(role: string) {
    const membership = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
    const session = {
      user_id: "u1",
      display_name: "Anna Muster",
      organization: membership,
      memberships: [membership],
    };
    const fetch = async (request: Request) =>
      json(200, new URL(request.url).pathname.endsWith("/session") ? session : { items: [] });
    const api = createApi(fetch as unknown as typeof globalThis.fetch);
    render(
      <Router>
        <SessionProvider api={api}>
          <EventsPage api={api} />
        </SessionProvider>
      </Router>,
    );
  }

  it.each(["owner", "admin"])("offers „Anlass erfassen“ to the role %s", async (role) => {
    signedIn(role);
    const link = await screen.findByRole("link", { name: "Anlass erfassen" });
    expect(link).toHaveAttribute("href", "/events/new");
  });

  it("does not offer „Anlass erfassen“ to a member", async () => {
    signedIn("member");
    await screen.findByText("Noch keine Anlässe erfasst");
    expect(screen.queryByRole("link", { name: "Anlass erfassen" })).not.toBeInTheDocument();
  });
});
