import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { createApi, type Fact } from "../api/client";
import { EvidencePanel } from "./EvidencePanel";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const SOURCE_VERSION = "0199b8e0-0000-7000-8000-0000000000e2";
const MEMBER_ID = "0199b8e0-0000-7000-8000-0000000000b1";

function json(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

const limits: (string | null)[] = [];

function fakeApi(options: { documentsFail?: boolean } = {}) {
  const fetch = async (request: Request) => {
    const { pathname } = new URL(request.url);
    if (pathname.endsWith("/members")) {
      return json({
        items: [{ user_id: MEMBER_ID, display_name: "Anna Muster", role: "member", version: 1 }],
      });
    }
    if (pathname.endsWith("/documents") && !options.documentsFail) {
      limits.push(new URL(request.url).searchParams.get("limit"));
      return json({
        items: [
          {
            id: "d1",
            event_id: EVENT_ID,
            readable_id: "DOC-001",
            name: "Programm Flugtag.pdf",
            owner: MEMBER_ID,
            created_at: "2030-05-18T08:00:00Z",
            version: 2,
            newest_version: {
              id: "v2",
              document_id: "d1",
              number: 2,
              kind: "upload",
              sha256: "0",
              uploaded_by: MEMBER_ID,
              source_version_id: SOURCE_VERSION,
              created_at: "2030-05-18T08:00:00Z",
            },
          },
        ],
      });
    }
    return new Response("{}", { status: 500, headers: { "Content-Type": "application/json" } });
  };
  return createApi(fetch as unknown as typeof globalThis.fetch);
}

const fact: Fact = {
  id: "f1",
  field_id: "fd1",
  field_key: "venue",
  state: "accepted",
  value: { type: "text", text: "Flugplatz Testwil" },
  version: 2,
  evidence: [
    {
      source_version_id: SOURCE_VERSION,
      passage: { start: 10, end: 27, quote: "Flugplatz Testwil", page: 3 },
      captured_at: "2026-10-03T12:12:00Z",
    },
  ],
  accepted_by: { kind: "member", id: MEMBER_ID, channel: "web" },
  accepted_at: "2026-10-04T08:30:00Z",
};

function renderPanel(overrides: Partial<Fact> = {}, onClose = () => {}, api = fakeApi()) {
  return render(
    <EvidencePanel
      api={api}
      eventId={EVENT_ID}
      timeZone="Europe/Zurich"
      title="Ort"
      valueText="Flugplatz Testwil"
      fact={{ ...fact, ...overrides }}
      onClose={onClose}
    />,
  );
}

describe("EvidencePanel", () => {
  it("shows the passage marked, the capture time and who accepted the value", async () => {
    renderPanel();

    const dialog = await screen.findByRole("dialog", { name: "Ort" });
    expect(within(dialog).getByRole("heading", { level: 2, name: "Ort" })).toBeVisible();
    expect(within(dialog).getByRole("heading", { level: 3, name: "Belege" })).toBeVisible();
    expect(within(dialog).getByText("Flugplatz Testwil", { selector: "mark" })).toBeVisible();
    expect(within(dialog).getByText("erfasst am 03.10.2026, 14:12")).toBeVisible();
    expect(within(dialog).getByText("Seite 3")).toBeVisible();
    expect(await within(dialog).findByText("Anna Muster")).toBeVisible();
    expect(within(dialog).getByText("04.10.2026, 10:30")).toBeVisible();
    expect(await within(dialog).findByText("Programm Flugtag.pdf, Version 2")).toBeVisible();
    expect(limits).toContain("200");
  });

  it("names the source version by its number when no document matches", async () => {
    renderPanel({}, () => {}, fakeApi({ documentsFail: true }));

    const dialog = await screen.findByRole("dialog", { name: "Ort" });
    expect(await within(dialog).findByText("Quellversion 0199b8e0")).toBeVisible();
    // The names of the members still resolve.
    expect(await within(dialog).findByText("Anna Muster")).toBeVisible();
  });

  it("says so when a value has no evidence", async () => {
    renderPanel({ evidence: [] });

    const dialog = await screen.findByRole("dialog", { name: "Ort" });
    expect(within(dialog).getByText("Für diesen Wert gibt es keinen Beleg.")).toBeVisible();
  });

  it("closes with the close button and with Escape", async () => {
    const onClose = vi.fn();
    renderPanel({}, onClose);

    const dialog = await screen.findByRole("dialog", { name: "Ort" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Schliessen" }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
