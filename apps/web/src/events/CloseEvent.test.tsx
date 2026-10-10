import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi, type Event } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { EventPage } from "./EventPage";

const ACTIVE: Event = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Testwil",
  time_zone: "Europe/Zurich",
  version: 4,
  created_at: "2028-03-01T13:12:00Z",
  status: "active",
  redacted: false,
};
const CLOSED: Event = {
  ...ACTIVE,
  status: "closed",
  closed_at: "2030-07-01T10:00:00Z",
  version: 5,
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function problem(status: number, code: string) {
  return json(
    status,
    { type: "", code, title: "", status, instance: "", request_id: "r1" },
    "application/problem+json",
  );
}

interface Setup {
  role?: string;
  /** Whether the member is an event manager. */
  eventManager?: boolean;
  event?: Event;
  answers?: Record<string, () => Response>;
}

function setup({ role = "member", eventManager = false, event = ACTIVE, answers = {} }: Setup) {
  const calls: { call: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
  const session = {
    user_id: "u1",
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    const text = request.method === "POST" ? await request.clone().text() : "";
    const key = `${request.method} ${pathname}`;
    calls.push({ call: key, body: text === "" ? undefined : JSON.parse(text) });
    const answer = answers[key];
    if (answer) {
      return answer();
    }
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (key === `GET /api/v1/events/${ACTIVE.id}`) {
      return json(200, event);
    }
    if (pathname.endsWith("/profile")) {
      return json(200, { facts: [], proposals: [], open_questions: [] });
    }
    if (pathname.endsWith("/fields")) {
      return json(200, { items: [] });
    }
    if (pathname.endsWith("/memberships")) {
      return eventManager || role !== "member"
        ? json(200, { items: [] })
        : problem(403, "forbidden");
    }
    throw new Error(`unexpected ${key}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${ACTIVE.id}`);
  render(
    <Router>
      <SessionProvider api={api}>
        <Routes>
          <Route path="/events/:eventId">
            <EventPage api={api}>
              <p>Inhalt</p>
            </EventPage>
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}

const user = userEvent.setup({ delay: null });

afterEach(() => window.history.replaceState(null, "", "/"));

const closeButton = () => screen.findByRole("button", { name: "Anlass abschliessen" });

describe("closing an event", () => {
  it("manager sees Anlass abschliessen and confirms", async () => {
    const { calls } = setup({
      eventManager: true,
      answers: { [`POST /api/v1/events/${ACTIVE.id}/close`]: () => json(200, CLOSED) },
    });

    await user.click(await closeButton());
    const dialog = await screen.findByRole("dialog", { name: "Anlass abschliessen?" });
    // Nothing is sent before the member confirms.
    expect(calls.some((c) => c.call.endsWith("/close"))).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Anlass abschliessen" }));

    const banner = (await screen.findByText("Abgeschlossen am 01.07.2030")).closest(
      "div[tabindex]",
    );
    // The close button left with the state: focus goes to the banner.
    await waitFor(() => expect(banner).toHaveFocus());
    expect(calls.find((c) => c.call.endsWith("/close"))?.body).toEqual({ expected_version: 4 });
    expect(screen.queryByRole("button", { name: "Anlass abschliessen" })).not.toBeInTheDocument();
  });

  it("does not offer closing to a plain member", async () => {
    setup({ role: "member", eventManager: false });

    await screen.findByText("Inhalt");
    await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toBeVisible());
    expect(screen.queryByRole("button", { name: "Anlass abschliessen" })).not.toBeInTheDocument();
  });

  it("keeps the event active when the member cancels", async () => {
    const { calls } = setup({ eventManager: true });

    await user.click(await closeButton());
    await user.click(await screen.findByRole("button", { name: "Abbrechen" }));

    expect(calls.some((c) => c.call.endsWith("/close"))).toBe(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    // Focus returns to the control that opened the dialog.
    expect(await closeButton()).toHaveFocus();
  });

  it("shows a stale version and keeps the dialog result readable", async () => {
    setup({
      eventManager: true,
      answers: {
        [`POST /api/v1/events/${ACTIVE.id}/close`]: () => problem(409, "record-version-conflict"),
      },
    });

    await user.click(await closeButton());
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Anlass abschliessen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Jemand hat diesen Eintrag inzwischen geändert.",
    );
  });
});

describe("a closed event", () => {
  it("owner sees Wieder öffnen and confirms", async () => {
    const { calls } = setup({
      role: "owner",
      event: CLOSED,
      answers: { [`POST /api/v1/events/${ACTIVE.id}/reopen`]: () => json(200, ACTIVE) },
    });

    expect(await screen.findByText("Abgeschlossen am 01.07.2030")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Wieder öffnen" }));
    const dialog = await screen.findByRole("dialog", { name: "Anlass wieder öffnen?" });
    await user.click(within(dialog).getByRole("button", { name: "Wieder öffnen" }));

    // The reopen button left with the state: focus goes to the close button.
    await waitFor(async () => expect(await closeButton()).toHaveFocus());
    expect(calls.find((c) => c.call.endsWith("/reopen"))?.body).toEqual({ expected_version: 5 });
    expect(screen.queryByText(/Abgeschlossen am/)).not.toBeInTheDocument();
  });

  it("owner does not see Wieder öffnen when the event is redacted", async () => {
    setup({ role: "owner", event: { ...CLOSED, redacted: true } });

    expect(await screen.findByText("Abgeschlossen am 01.07.2030")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Wieder öffnen" })).not.toBeInTheDocument();
  });

  it("a manager who is not owner or admin cannot reopen", async () => {
    setup({ role: "member", eventManager: true, event: CLOSED });

    expect(await screen.findByText("Abgeschlossen am 01.07.2030")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Wieder öffnen" })).not.toBeInTheDocument();
  });

  it("explains a reopen after a redaction that the server refuses", async () => {
    setup({
      role: "admin",
      event: CLOSED,
      answers: {
        [`POST /api/v1/events/${ACTIVE.id}/reopen`]: () => problem(409, "invalid-transition"),
      },
    });

    await user.click(await screen.findByRole("button", { name: "Wieder öffnen" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Wieder öffnen" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("nicht möglich");
  });
});
