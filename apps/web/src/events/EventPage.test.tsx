import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApi, type Event } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { SessionProvider } from "../session/SessionProvider";
import { EventPage } from "./EventPage";

const event: Event = {
  id: "0199b8e0-0000-7000-8000-000000000001",
  key: "FLY28",
  name: "Fly-in Musterhausen mit Veranstaltungsbewilligungsverfahren",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
  status: "active",
  redacted: false,
};

function json(status: number, body: unknown, contentType = "application/json") {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": contentType } });
}

const dateWindowFact = {
  id: "f1",
  field_id: "fd1",
  field_key: "date_window",
  state: "accepted",
  value: { type: "date-window", start: "2030-05-01", end: "2030-06-30", granularity: "month" },
  version: 1,
  evidence: [],
  accepted_by: { kind: "member", id: "u1", channel: "web" },
  accepted_at: "2030-03-01T13:12:00Z",
};

/** The catalog: the label of the field comes from here. */
const fields = [
  {
    id: "fd1",
    key: "date_window",
    label: { kind: "text", text: "Durchführung" },
    value_type: { type: "date-window", granularity: null },
    value_schema: {},
    description: "",
    module: "core",
    status: "active",
  },
];

const session = {
  user_id: "u1",
  display_name: "Anna Muster",
  organization: { organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" },
  memberships: [{ organization_id: "o1", name: "Fliegergruppe Testwil", role: "member" }],
};

/** The answer to the profile: the date window is accepted. */
const profile = { facts: [dateWindowFact], proposals: [], open_questions: [] };

/** `responses` answer the event; the profile and the fields have their own answers. */
function fakeApi(...responses: Response[]) {
  return fakeApiWithProfile(() => json(200, profile), ...responses);
}

function fakeApiWithProfile(answerProfile: () => Response, ...responses: Response[]) {
  const urls: string[] = [];
  const fetch = vi.fn(async (request: Request) => {
    const { pathname } = new URL(request.url);
    if (pathname.endsWith("/session")) {
      return json(200, session);
    }
    if (pathname.endsWith("/memberships")) {
      // The member is no event manager: no control to close the event shows.
      return json(403, { type: "", code: "forbidden", title: "", status: 403, instance: "" });
    }
    if (pathname.endsWith("/profile")) {
      return answerProfile();
    }
    if (pathname.endsWith("/fields")) {
      return json(200, { items: fields });
    }
    urls.push(pathname);
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
      <SessionProvider api={api}>
        <Routes>
          <Route path="/events/:eventId">
            <EventPage api={api}>
              <p>Platzhalter</p>
            </EventPage>
          </Route>
        </Routes>
      </SessionProvider>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("EventPage", () => {
  it("shows the name and the content, and has no navigation of its own", async () => {
    const { api, urls } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}`, api);

    expect(await screen.findByRole("heading", { level: 1, name: event.name })).toBeInTheDocument();
    expect(urls[0]).toBe(`/api/v1/events/${event.id}`);
    expect(screen.getByText("FLY28")).toBeInTheDocument();
    expect(screen.getByText("Platzhalter")).toBeInTheDocument();
    // The destinations of an event are the items of the sidebar (ADR 0082), not tabs.
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  });

  it("puts the key and the state of the event in the header, in words", async () => {
    const { api } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}`, api);

    const header = (await screen.findByRole("heading", { level: 1 })).closest(
      "header",
    ) as HTMLElement;
    expect(within(header).getByText("FLY28")).toBeInTheDocument();
    expect(within(header).getByText("Offen")).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("shows the date window of the profile in the header with its state", async () => {
    const { api } = fakeApi(json(200, event));
    renderAt(`/events/${event.id}`, api);

    const header = (await screen.findByRole("heading", { level: 1 })).closest("header");
    expect(
      await within(header as HTMLElement).findByText("Mai 2030 \u2013 Juni 2030"),
    ).toBeVisible();
    expect(within(header as HTMLElement).getByText("Bestätigt")).toBeInTheDocument();
    expect(within(header as HTMLElement).getByText(/Durchführung/)).toBeVisible();
  });

  it("shows Unbekannt in the header when the profile has no date window", async () => {
    const { api } = fakeApiWithProfile(
      () => json(200, { facts: [], proposals: [], open_questions: [] }),
      json(200, event),
    );
    renderAt(`/events/${event.id}`, api);

    const header = (await screen.findByRole("heading", { level: 1 })).closest("header");
    expect(await within(header as HTMLElement).findByText("Unbekannt")).toBeVisible();
  });

  it("shows the header without dates when the profile fails", async () => {
    const { api } = fakeApiWithProfile(() => json(500, {}), json(200, event));
    renderAt(`/events/${event.id}`, api);

    expect(await screen.findByRole("heading", { level: 1, name: event.name })).toBeInTheDocument();
    expect(screen.queryByText("Unbekannt")).not.toBeInTheDocument();
    expect(screen.getByText("Platzhalter")).toBeInTheDocument();
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

  it("keeps the name of a redacted event and shows entfernt for a redacted date window", async () => {
    const { api } = fakeApiWithProfile(
      () =>
        json(200, {
          facts: [{ ...dateWindowFact, value: undefined, redacted: true }],
          proposals: [],
          open_questions: [],
        }),
      json(200, { ...event, redacted: true }),
    );
    renderAt(`/events/${event.id}`, api);

    const heading = await screen.findByRole("heading", { level: 1 });
    expect(heading).toHaveTextContent(event.name);
    const header = heading.closest("header") as HTMLElement;
    await waitFor(() => expect(within(header).getAllByText("entfernt")).toHaveLength(1));
  });
});
