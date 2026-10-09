// A fake server and a render helper for the tests of the work pages (workstreams, actions and
// commitments). Only invented data.
import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { createApi, type Event } from "../api/client";
import { EventContext } from "../events/eventContext";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";

export const EVENT: Event = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Testwil",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
};

export const ME = "0199b8e0-0000-7000-8000-0000000000b1";
export const BERND = "0199b8e0-0000-7000-8000-0000000000b2";
export const CLARA = "0199b8e0-0000-7000-8000-0000000000b3";

const members = [
  { user_id: ME, display_name: "Anna Muster", role: "member", version: 1 },
  { user_id: BERND, display_name: "Bernd Beispiel", role: "member", version: 1 },
  { user_id: CLARA, display_name: "Cäcilia Probst", role: "member", version: 1 },
];

export const memberships = [
  { user_id: ME, display_name: "Anna Muster", event_role: "event-contributor", version: 1 },
  { user_id: BERND, display_name: "Bernd Beispiel", event_role: "event-manager", version: 1 },
  { user_id: CLARA, display_name: "Cäcilia Probst", event_role: "event-viewer", version: 1 },
];

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

export interface Setup {
  /** The organization role of the signed-in member. */
  role?: string;
  /** Whether the member sees the event memberships, as an event manager does. */
  eventManager?: boolean;
  workstreams?: unknown[];
  /** Answers by `METHOD /path-suffix`; the first match wins. */
  answers?: Record<string, () => Response>;
  /** The default answers of the lists, by path suffix. */
  lists?: Record<string, unknown[]>;
}

/** Renders `page` for the event with the fake server. It records `METHOD path` and the body. */
export function renderWork(
  page: (api: ReturnType<typeof createApi>) => ReactNode,
  {
    role = "member",
    eventManager = false,
    workstreams = [ground],
    answers = {},
    lists = {},
  }: Setup = {},
) {
  const calls: { call: string; body: unknown }[] = [];
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
    calls.push({
      call: `${request.method} ${url.pathname}`,
      body: text === "" ? undefined : JSON.parse(text),
    });
    const answer = Object.entries(answers).find(([key]) => {
      const [method, suffix = ""] = key.split(" ");
      return request.method === method && url.pathname.endsWith(suffix);
    });
    if (answer) {
      return answer[1]();
    }
    const path = url.pathname;
    if (path.endsWith("/session")) {
      return json(200, session);
    }
    if (request.method === "GET" && path.endsWith("/api/v1/members")) {
      return json(200, { items: members });
    }
    if (request.method === "GET" && path.endsWith("/memberships")) {
      return eventManager ? json(200, { items: memberships }) : problem(403, "forbidden");
    }
    if (request.method === "GET" && path.endsWith("/workstreams")) {
      return json(200, { items: workstreams });
    }
    const list = Object.entries(lists).find(([suffix]) => path.endsWith(suffix));
    if (request.method === "GET" && list) {
      return json(200, { items: list[1] });
    }
    throw new Error(`unexpected ${request.method} ${path}`);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${EVENT.id}`);
  render(
    <Router>
      <SessionProvider api={api}>
        <EventContext
          value={{ event: EVENT, profile: { kind: "loading" }, reloadProfile: async () => true }}
        >
          {page(api)}
        </EventContext>
      </SessionProvider>
    </Router>,
  );
  return { calls };
}
