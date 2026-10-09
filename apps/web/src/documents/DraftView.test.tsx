import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createApi, type DraftRendering, type Fact, type Field } from "../api/client";
import type { ProfileState } from "../events/eventContext";
import { DraftView } from "./DraftView";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const DATE_FIELD = "0199b8e0-0000-7000-8000-0000000000f1";
const VENUE_FIELD = "0199b8e0-0000-7000-8000-0000000000f2";
const FACT_DATE = "0199b8e0-0000-7000-8000-0000000000a1";
const FACT_VENUE = "0199b8e0-0000-7000-8000-0000000000a2";
const SOURCE = "0199b8e0-0000-7000-8000-0000000000e1";

const link = (kind: "fact" | "source", id: string, rest: string) => `tada:${kind}/${id}${rest}`;
const dateLink = link("fact", FACT_DATE, "?v=2");
const oldDateLink = link("fact", FACT_DATE, "?v=1");
const venueLink = link("fact", FACT_VENUE, "?v=1");
const hiddenLink = link("fact", "0199b8e0-0000-7000-8000-0000000000a3", "?v=1");
const sourceLink = link("source", SOURCE, "#0-5");

const fields: Field[] = [
  {
    id: DATE_FIELD,
    key: "event_date",
    label: { kind: "text", text: "Datum" },
    value_type: { type: "date" },
    value_schema: {},
    description: "",
    module: "core",
    status: "active",
  },
  {
    id: VENUE_FIELD,
    key: "venue",
    label: { kind: "text", text: "Veranstaltungsort" },
    value_type: { type: "text" },
    value_schema: {},
    description: "",
    module: "core",
    status: "active",
  },
];

const author = { kind: "member", id: "0199b8e0-0000-7000-8000-0000000000b1", channel: "web" };
const dateFact: Fact = {
  id: FACT_DATE,
  field_id: DATE_FIELD,
  field_key: "event_date",
  state: "accepted",
  value: { type: "date", date: "2030-05-18" },
  version: 2,
  evidence: [
    {
      source_version_id: SOURCE,
      passage: { start: 0, end: 5, quote: "18. Mai" },
      captured_at: "2030-05-01T08:00:00Z",
    },
  ],
  accepted_by: author as Fact["accepted_by"],
  accepted_at: "2030-05-02T08:00:00Z",
};

const loaded: ProfileState = {
  kind: "loaded",
  profile: { facts: [dateFact], proposals: [], open_questions: [] },
  fields,
};

function draft(markdown: string, links: DraftRendering["links"]): DraftRendering {
  return { markdown, lint_warnings: [], links };
}

function json(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

const api = createApi((async () => json({ items: [] })) as unknown as typeof globalThis.fetch);

function show(
  rendering: DraftRendering,
  profile: ProfileState = loaded,
  headingLevel: 2 | 3 | 4 | 5 | 6 = 2,
) {
  return render(
    <DraftView
      headingLevel={headingLevel}
      api={api}
      eventId={EVENT_ID}
      timeZone="Europe/Zurich"
      draft={rendering}
      profile={profile}
    />,
  );
}

describe("DraftView", () => {
  it("shows the value of a cited fact, formatted as everywhere", async () => {
    show(
      draft(`Der Anlass ist am [](${dateLink}).`, {
        [dateLink]: {
          kind: "fact",
          fact_id: FACT_DATE,
          version: 2,
          state: "accepted",
          value: dateFact.value,
        },
      }),
    );

    expect(await screen.findByText("18.05.2030")).toBeInTheDocument();
  });

  it("opens the evidence of a cited fact", async () => {
    show(
      draft(`Der Anlass ist am [](${dateLink}).`, {
        [dateLink]: {
          kind: "fact",
          fact_id: FACT_DATE,
          version: 2,
          state: "accepted",
          value: dateFact.value,
        },
      }),
    );

    await userEvent.click(
      await screen.findByRole("button", { name: /18\.05\.2030.*Beleg zu Datum/ }),
    );

    const sheet = await screen.findByRole("dialog", { name: "Datum" });
    expect(within(sheet).getByText("18. Mai")).toBeInTheDocument();
  });

  it("marks an assumption with „Annahme“", async () => {
    show(
      draft(`Der Ort ist [](${venueLink}).`, {
        [venueLink]: {
          kind: "fact",
          fact_id: FACT_VENUE,
          version: 1,
          state: "assumption",
          value: { type: "text", text: "Flugplatz Testwil" },
        },
      }),
    );

    expect(await screen.findByText("Flugplatz Testwil")).toBeInTheDocument();
    expect(screen.getByText("Annahme")).toBeInTheDocument();
  });

  it("shows an unknown fact as unknown, never as a value", async () => {
    show(
      draft(`Der Ort ist [](${venueLink}).`, {
        [venueLink]: { kind: "fact", fact_id: FACT_VENUE, version: 1, state: "unknown" },
      }),
    );

    expect(await screen.findByText("Unbekannt")).toBeInTheDocument();
  });

  it("shows „entfernt“ for a hidden target and for a link that the map lacks", async () => {
    show(
      draft(`A [](${hiddenLink}) und B [](${venueLink}).`, {
        [hiddenLink]: { kind: "hidden" },
      }),
    );

    expect(await screen.findAllByText("entfernt")).toHaveLength(2);
  });

  it("numbers a source and lists its passage", async () => {
    show(
      draft(`Die Zahl stammt aus [dem Protokoll](${sourceLink}).`, {
        [sourceLink]: {
          kind: "source",
          source_version_id: SOURCE,
          passage: { start: 0, end: 5, quote: "Hallo zusammen" },
        },
      }),
    );

    expect(await screen.findByText("dem Protokoll")).toBeInTheDocument();
    expect(screen.getByText("[1]")).toBeInTheDocument();
    expect(screen.getByText("Quelle 1")).toBeInTheDocument();
    const sources = screen.getByRole("region", { name: "Quellen" });
    expect(within(sources).getByText("Hallo zusammen")).toBeInTheDocument();
  });

  it("names the fact version when the draft cites an older one, and offers no evidence", async () => {
    show(
      draft(`Der Anlass war am [](${oldDateLink}).`, {
        [oldDateLink]: {
          kind: "fact",
          fact_id: FACT_DATE,
          version: 1,
          state: "accepted",
          value: { type: "date", date: "2030-05-17" },
        },
      }),
    );

    expect(await screen.findByText("17.05.2030")).toBeInTheDocument();
    expect(screen.getByText(/Version 1/)).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows a link with an unsafe destination as text", async () => {
    show(draft("Siehe [hier](javascript:alert(1)) und [dort](https://example.org).", {}));

    expect(await screen.findByText(/Siehe/)).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "hier" })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "dort" })).toHaveAttribute(
      "href",
      "https://example.org",
    );
  });

  it("drops raw HTML instead of making elements of it", async () => {
    const { container } = show(
      draft(
        'Text <script>window.hacked = 1</script><b onclick="x()">fett</b>\n\n<div>Block</div>',
        {},
      ),
    );

    expect(await screen.findByText(/Text/)).toBeInTheDocument();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
    expect(container.innerHTML).not.toContain("onclick");
    expect(container.innerHTML).not.toContain("<div>Block");
  });

  it("never loads an image", async () => {
    const { container } = show(draft("![Lageplan](https://example.org/plan.png)", {}));

    expect(await screen.findByText("Lageplan")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });

  it("waits for the profile, because the value types come with it", () => {
    show(draft("Text", {}), { kind: "loading" });

    expect(screen.queryByText("Text")).not.toBeInTheDocument();
  });

  it("puts its headings and „Quellen“ at the level that the page gives", async () => {
    show(
      draft(`# Konzept\n\nZahl aus [dem Protokoll](${sourceLink}).`, {
        [sourceLink]: {
          kind: "source",
          source_version_id: SOURCE,
          passage: { start: 0, end: 5, quote: "Hallo" },
        },
      }),
      loaded,
      4,
    );

    expect(await screen.findByRole("heading", { level: 4, name: "Konzept" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 4, name: "Quellen" })).toBeInTheDocument();
  });

  it("tells the page when the text is in it", async () => {
    let shown = 0;
    render(
      <DraftView
        api={api}
        eventId={EVENT_ID}
        timeZone="Europe/Zurich"
        draft={draft("Text", {})}
        profile={loaded}
        onShown={() => {
          shown += 1;
        }}
      />,
    );

    await screen.findByText("Text");
    expect(shown).toBe(1);
  });
});
