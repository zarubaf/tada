import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createApi, type Event } from "../api/client";
import { Route, Router, Routes } from "../router/Router";
import { EventOverview } from "./EventOverview";
import { EventPage } from "./EventPage";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const MEMBER_ID = "0199b8e0-0000-7000-8000-0000000000b1";

const event: Event = {
  id: EVENT_ID,
  key: "FLY28",
  name: "Fly-in Musterhausen",
  time_zone: "Europe/Zurich",
  version: 1,
  created_at: "2028-03-01T13:12:00Z",
};

function field(key: string, valueType: unknown) {
  return {
    id: `fd-${key}`,
    key,
    label: { kind: "message", id: `field-${key}` },
    value_type: valueType,
    value_schema: {},
    description: "",
    module: "core",
    status: "active",
  };
}

const fields = [
  field("date_window", { type: "date-window", granularity: null }),
  field("visitor_estimate", { type: "quantity", unit: "person_per_day" }),
  field("entry_fee_adult", { type: "money", currency: "CHF" }),
  field("venue", { type: "text" }),
];

function fact(key: string, state: string, value: unknown, extra: object = {}) {
  return {
    id: `f-${key}`,
    field_id: `fd-${key}`,
    field_key: key,
    state,
    value,
    version: 1,
    evidence: [],
    accepted_by: { kind: "member", id: MEMBER_ID, channel: "web" },
    accepted_at: "2026-10-04T08:30:00Z",
    ...extra,
  };
}

const profile = {
  facts: [
    fact(
      "venue",
      "accepted",
      { type: "text", text: "Flugplatz Testwil" },
      {
        evidence: [
          {
            source_version_id: "0199b8e0-0000-7000-8000-0000000000e2",
            passage: { start: 1, end: 18, quote: "Flugplatz Testwil" },
            captured_at: "2026-10-03T12:12:00Z",
          },
        ],
      },
    ),
    fact(
      "visitor_estimate",
      "assumption",
      { type: "quantity", min: "20000", max: "20000" },
      {
        approximate: true,
      },
    ),
    fact("entry_fee_adult", "unknown", undefined),
  ],
  proposals: [
    {
      id: "p1",
      changeset_id: "c1",
      field_id: "fd-entry_fee_adult",
      state: "accepted",
      value: { type: "money", min: 1500, max: 1500 },
      created_at: "2026-10-05T09:00:00Z",
    },
  ],
  open_questions: [
    {
      id: "q1",
      local_id: "QST-001",
      text: "Wer bestellt die Festwirtschaft?",
      owner_id: MEMBER_ID,
      version: 1,
    },
  ],
};

function json(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": status < 400 ? "application/json" : "application/problem+json" },
  });
}

function setup(options: { profile?: () => Response } = {}) {
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    if (pathname.endsWith("/profile")) {
      return options.profile?.() ?? json(200, profile);
    }
    if (pathname.endsWith("/fields")) {
      return json(200, { items: fields });
    }
    if (pathname.endsWith("/members")) {
      return json(200, {
        items: [{ user_id: MEMBER_ID, display_name: "Anna Muster", role: "member", version: 1 }],
      });
    }
    if (pathname.endsWith("/documents")) {
      return json(200, { items: [] });
    }
    return json(200, event);
  };
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  window.history.replaceState(null, "", `/events/${EVENT_ID}`);
  render(
    <Router>
      <Routes>
        <Route path="/events/:eventId">
          <EventPage api={api}>
            <EventOverview api={api} />
          </EventPage>
        </Route>
      </Routes>
    </Router>,
  );
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("EventOverview", () => {
  it("lists the open questions", async () => {
    setup();

    const section = (await screen.findByRole("heading", { name: "Offene Fragen" })).closest(
      "section",
    ) as HTMLElement;
    expect(within(section).getByText("QST-001")).toBeInTheDocument();
    expect(within(section).getByText("Wer bestellt die Festwirtschaft?")).toBeInTheDocument();
  });

  it("shows each field with the state of its value and Unbekannt for a missing one", async () => {
    setup();

    const facts = (await screen.findByRole("heading", { name: "Fakten" })).closest(
      "section",
    ) as HTMLElement;
    const row = (label: string) =>
      within(facts).getByText(label, { selector: "dt" }).closest("div") as HTMLElement;

    expect(within(row("Ort")).getByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(within(row("Ort")).getByText("Bestätigt")).toBeInTheDocument();

    expect(within(row("Erwartete Besucher pro Tag")).getByText("Annahme")).toBeVisible();
    expect(
      within(row("Erwartete Besucher pro Tag")).getByText(/^ca\. 20.000 Personen/),
    ).toBeVisible();

    // Unknown, with a fact: Unbekannt and no value.
    expect(within(row("Eintritt Erwachsene")).getByText("Unbekannt")).toBeVisible();
    // No fact at all: also Unbekannt, and no evidence button.
    expect(within(row("Zeitfenster")).getByText("Unbekannt")).toBeVisible();
    expect(within(row("Zeitfenster")).queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows a proposal in its own subsection, not as the value of the fact", async () => {
    setup();

    const facts = (await screen.findByRole("heading", { name: "Fakten" })).closest(
      "section",
    ) as HTMLElement;
    const proposals = within(facts)
      .getByRole("heading", { level: 3, name: "Vorschläge" })
      .closest("section") as HTMLElement;
    expect(within(proposals).getByText("Eintritt Erwachsene")).toBeInTheDocument();
    expect(within(proposals).getByText(/^CHF.15\.00$/)).toBeInTheDocument();
    expect(within(proposals).getByText("Vorschlag")).toBeVisible();
    // The accepted list does not show the amount.
    const row = within(facts).getByText("Eintritt Erwachsene", { selector: "dt" }).closest("div");
    expect(within(row as HTMLElement).queryByText(/^CHF.15\.00$/)).not.toBeInTheDocument();
  });

  it("opens the evidence of a value and returns focus to the button on Escape", async () => {
    setup();

    const button = await screen.findByRole("button", { name: "Beleg zu Ort" });
    await userEvent.click(button);
    const dialog = await screen.findByRole("dialog", { name: "Ort" });
    expect(within(dialog).getByText("erfasst am 03.10.2026, 14:12")).toBeVisible();

    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(button).toHaveFocus();
  });

  it("shows the message of a failed profile and retries with focus on the heading", async () => {
    let failures = 1;
    setup({
      profile: () => {
        if (failures > 0) {
          failures -= 1;
          return json(503, {
            type: "",
            code: "unavailable",
            title: "",
            status: 503,
            instance: "",
            request_id: "r1",
          });
        }
        return json(200, profile);
      },
    });

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("nicht erreichbar");
    await userEvent.click(within(alert).getByRole("button", { name: "Erneut versuchen" }));

    const heading = await screen.findByRole("heading", { name: "Offene Fragen" });
    expect(heading).toHaveFocus();
  });
});
