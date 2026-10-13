import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { type Api, createApi } from "../api/client";
import { Router } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { json, ME } from "../test/fakeWorkServer";
import { useDirectory } from "./directory";

const own = { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" };
const session = {
  user_id: ME,
  display_name: "Anna Muster",
  organization: own,
  memberships: [own],
};

/** The workstreams of each event come from a promise that the test settles. */
function server() {
  const pending = new Map<string, (name: string) => void>();
  const fetch = async (request: Request) => {
    const path = new URL(request.url).pathname;
    if (path.endsWith("/session")) {
      return json(200, session);
    }
    if (path.endsWith("/api/v1/members") || path.endsWith("/memberships")) {
      return json(200, { items: [] });
    }
    const event = /events\/([^/]+)\/workstreams/.exec(path)?.[1] ?? "";
    const name = await new Promise<string>((resolve) => pending.set(event, resolve));
    return json(200, {
      items: [
        {
          id: event,
          event_id: event,
          name,
          redacted: false,
          lead_user_id: ME,
          status: "active",
          version: 1,
        },
      ],
    });
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  const settle = async (event: string, name: string) => {
    await waitFor(() => expect(pending.has(event)).toBe(true));
    pending.get(event)?.(name);
  };
  return { api, settle };
}

function Probe({ api, eventId }: { api: Api; eventId: string }) {
  const { state } = useDirectory(api, eventId);
  return (
    <p>
      {state.kind === "loaded"
        ? state.directory.workstreams.map((w) => w.name).join(",")
        : state.kind}
    </p>
  );
}

const page = (api: Api, eventId: string) => (
  <Router>
    <SessionProvider api={api}>
      <Probe api={api} eventId={eventId} />
    </SessionProvider>
  </Router>
);

describe("useDirectory", () => {
  it("drops the late answer of the event that the page left", async () => {
    const { api, settle } = server();
    const { rerender } = render(page(api, "ea"));
    await waitFor(() => expect(screen.getByText("loading")).toBeInTheDocument());
    rerender(page(api, "eb"));
    await settle("eb", "Stream B");
    await screen.findByText("Stream B");
    await settle("ea", "Stream A");
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.getByText("Stream B")).toBeInTheDocument();
  });

  it("does not show the directory of the previous event while the next one loads", async () => {
    const { api, settle } = server();
    const { rerender } = render(page(api, "ea"));
    await settle("ea", "Stream A");
    await screen.findByText("Stream A");
    rerender(page(api, "eb"));
    expect(screen.queryByText("Stream A")).not.toBeInTheDocument();
    expect(screen.getByText("loading")).toBeInTheDocument();
  });
});
