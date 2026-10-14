// A fake server and a render helper for the tests of the work pages (workstreams, actions,
// commitments and milestones). Only invented data.
import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { createApi, type Event } from "../api/client";
import { EventContext, type ProfileState } from "../events/eventContext";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";

export const EVENT: Event = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Testwil",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
  status: "active",
  redacted: false,
};

export const ME = "0199b8e0-0000-7000-8000-0000000000b1";
export const BERND = "0199b8e0-0000-7000-8000-0000000000b2";
export const CLARA = "0199b8e0-0000-7000-8000-0000000000b3";

const members = [
  { user_id: ME, display_name: "Anna Muster", role: "member", version: 1 },
  { user_id: BERND, display_name: "Bernd Beispiel", role: "member", version: 1 },
  { user_id: CLARA, display_name: "Cäcilia Probst", role: "member", version: 1 },
];

/** The event memberships; `manager` is the role of the signed-in member. */
function memberships(manager: boolean) {
  return [
    {
      user_id: ME,
      display_name: "Anna Muster",
      event_role: manager ? "event-manager" : "event-contributor",
      version: 1,
    },
    { user_id: BERND, display_name: "Bernd Beispiel", event_role: "event-contributor", version: 1 },
    { user_id: CLARA, display_name: "Cäcilia Probst", event_role: "event-viewer", version: 1 },
  ];
}

export const ground = {
  id: "w1",
  event_id: EVENT.id,
  name: "Bodenbetrieb",
  lead_user_id: CLARA,
  status: "active",
  version: 1,
};

export function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

export function problem(status: number, code: string, errors?: unknown) {
  const body = { type: "", code, title: "", status, instance: "", request_id: "r1", errors };
  return json(status, body, "application/problem+json");
}

/** An answer that serves `pages` one after the other, each with the cursor of the next. */
export function pagedList(pages: unknown[][]): () => Response {
  let served = 0;
  return () => {
    const index = Math.min(served++, pages.length - 1);
    return json(200, {
      items: pages[index],
      next_cursor: index < pages.length - 1 ? `page-${index + 1}` : null,
    });
  };
}

export interface Setup {
  /** The organization role of the signed-in member. */
  role?: string;
  /** Whether the signed-in member is an event manager. Otherwise the member is a contributor. */
  eventManager?: boolean;
  /** The event of the page. The default is an active event. */
  event?: Event;
  workstreams?: unknown[];
  /**
   * Answers by `METHOD /path-suffix`; the first match wins. A function gets the address and the
   * parsed body of the request, if it has one.
   */
  answers?: Record<string, (url: URL, body?: unknown) => Response | Promise<Response>>;
  /** The default answers of the lists, by path suffix. */
  lists?: Record<string, unknown[]>;
  /** The profile of the event that the event page has loaded. The default is still loading. */
  profile?: ProfileState;
  /** The fragment of the address, for a link to a row of a register. */
  hash?: string;
  /** The path and query of the address, if the page does not sit at the path of the event. */
  url?: string;
}

/** Renders `page` for the event with the fake server. It records `METHOD path` and the body. */
export function renderWork(
  page: (api: ReturnType<typeof createApi>) => ReactNode,
  {
    role = "member",
    eventManager = false,
    event = EVENT,
    workstreams = [ground],
    answers = {},
    lists = {},
    profile = { kind: "loading" },
    hash = "",
    url: address,
  }: Setup = {},
) {
  const calls: { call: string; query: string; body: unknown }[] = [];
  const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role };
  const session = {
    user_id: ME,
    display_name: "Anna Muster",
    organization: own,
    memberships: [own],
  };
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    const text = request.method === "GET" ? "" : await request.clone().text();
    const body = text === "" ? undefined : JSON.parse(text);
    calls.push({ call: `${request.method} ${url.pathname}`, query: url.search, body });
    const answer = Object.entries(answers).find(([key]) => {
      const [method, suffix = ""] = key.split(" ");
      return request.method === method && url.pathname.endsWith(suffix);
    });
    if (answer) {
      return answer[1](url, body);
    }
    const path = url.pathname;
    if (path.endsWith("/session")) {
      return json(200, session);
    }
    if (request.method === "GET" && path === `/api/v1/events/${event.id}`) {
      return json(200, event);
    }
    if (request.method === "GET" && path.endsWith("/api/v1/members")) {
      return json(200, { items: members });
    }
    if (request.method === "GET" && path.endsWith("/memberships")) {
      return json(200, { items: memberships(eventManager) });
    }
    if (request.method === "GET" && path.endsWith("/workstreams")) {
      return json(200, { items: workstreams });
    }
    const list = Object.entries(lists).find(([suffix]) => path.endsWith(suffix));
    if (request.method === "GET" && list) {
      return json(200, { items: list[1] });
    }
    // The milestones that the action form offers, and the kinds that a page did not ask for.
    if (request.method === "GET" && /\/(milestones|risks)$/.test(path)) {
      return json(200, { items: [] });
    }
    throw new Error(`unexpected ${request.method} ${path}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", address ?? `/events/${EVENT.id}${hash}`);
  render(
    <Router>
      <SessionProvider api={api}>
        <EventContext value={{ event, profile, reloadProfile: async () => true }}>
          {page(api)}
        </EventContext>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}
