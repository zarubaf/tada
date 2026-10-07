import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi, type Event } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { EventPage } from "./EventPage";

const event: Event = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Musterhausen mit Veranstaltungsbewilligungsverfahren",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

function fakeApi(...responses: Response[]) {
  const urls: string[] = [];
  const fetch = vi.fn(async (request: Request) => {
    urls.push(new URL(request.url).pathname);
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), urls };
}

function renderAt(path: string, api: ReturnType<typeof createApi>) {
  window.history.replaceState(null, "", path);
  return render(
    <Router>
      <Routes>
        <Route path="/events/:eventId">
          <EventPage api={api}>
            <p>Platzhalter</p>
          </EventPage>
        </Route>
        <Route path="/events/:eventId/members">
          <EventPage api={api}>
            <p>Mitgliederliste</p>
          </EventPage>
        </Route>
      </Routes>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("EventPage", () => {
  it("shows the name, the sub-navigation and the content", async () => {
    const { api, urls } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}`, api);

    expect(await screen.findByRole("heading", { level: 1, name: event.name })).toBeInTheDocument();
    expect(urls[0]).toBe(`/api/v1/events/${event.id}`);
    expect(screen.getByText("FLY28")).toBeInTheDocument();
    expect(screen.getByText("Platzhalter")).toBeInTheDocument();

    const nav = screen.getByRole("navigation", { name: "Anlass" });
    expect(within(nav).getByRole("link", { name: "Übersicht" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(within(nav).getByRole("link", { name: "Mitglieder" })).not.toHaveAttribute(
      "aria-current",
    );
    expect(within(nav).queryByRole("link", { name: "Personen" })).not.toBeInTheDocument();
  });

  it("marks only Mitglieder as current on the members path", async () => {
    const { api } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}/members`, api);

    await screen.findByText("Mitgliederliste");
    const nav = screen.getByRole("navigation", { name: "Anlass" });
    expect(within(nav).getByRole("link", { name: "Mitglieder" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(within(nav).getByRole("link", { name: "Übersicht" })).not.toHaveAttribute(
      "aria-current",
    );
  });

  it("opens the members page from the sub-navigation", async () => {
    const { api, urls } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}`, api);

    await userEvent.click(await screen.findByRole("link", { name: "Mitglieder" }));
    expect(window.location.pathname).toBe(`/events/${event.id}/members`);
    expect(await screen.findByText("Mitgliederliste")).toBeInTheDocument();
    // The page stays mounted: no skeleton, and the event is not loaded a second time.
    expect(urls).toHaveLength(1);
  });

  it("shows the message of a not-found problem and retries", async () => {
    const problem = { type: "", code: "not-found", title: "", status: 404, instance: "" };
    const { api } = fakeApi(
      json(404, { ...problem, request_id: "r1" }, "application/problem+json"),
      json(200, event),
    );
    renderAt(`/events/${event.id}`, api);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Das gibt es nicht, oder Sie dürfen es nicht sehen.");

    await userEvent.click(within(alert).getByRole("button", { name: "Erneut versuchen" }));
    expect(await screen.findByRole("heading", { name: event.name })).toBeInTheDocument();
  });
});
