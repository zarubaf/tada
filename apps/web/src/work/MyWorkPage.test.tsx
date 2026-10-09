import { render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
import { isOverdue } from "./dueDate";
import { MyWorkPage } from "./MyWorkPage";

const EVENT_ID = "0199b8e0-0000-7000-8000-000000000001";
const ME = "0199b8e0-0000-7000-8000-0000000000b1";

function action(over: Record<string, unknown> = {}) {
  return {
    id: "a1",
    local_id: "ACT-042",
    event_id: EVENT_ID,
    event_key: "FLY28",
    title: "Generator bestellen",
    owner_user_id: ME,
    workstream_id: null,
    due_date: "2030-05-18",
    status: "open",
    version: 1,
    ...over,
  };
}

function commitment(over: Record<string, unknown> = {}) {
  return {
    id: "c1",
    local_id: "COM-003",
    event_id: EVENT_ID,
    event_key: "FLY28",
    text: "Lieferung am Freitag",
    condition: "nach unterschriebener Bestellung",
    promisor: { kind: "institution", id: "i1", name: "Testwil Generatoren AG" },
    owner_user_id: ME,
    workstream_id: null,
    due_date: "2030-05-17",
    status: "conditional",
    version: 1,
    evidence: [],
    ...over,
  };
}

function renderPage(answer: () => Response) {
  const fetch = async () => answer();
  const api = createApi(fetch as unknown as typeof globalThis.fetch);
  return render(
    <Router>
      <MyWorkPage api={api} />
    </Router>,
  );
}

const work = (body: Record<string, unknown>) => () =>
  new Response(JSON.stringify({ actions: [], commitments: [], review_count: 0, ...body }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });

beforeEach(() => vi.useFakeTimers({ toFake: ["Date"], now: new Date(2030, 4, 16, 12) }));
afterEach(() => vi.useRealTimers());

describe("My Work", () => {
  it("lists my actions by due date with the full reference", async () => {
    renderPage(
      work({
        actions: [
          action({ id: "a1", local_id: "ACT-042", due_date: "2030-05-18" }),
          action({ id: "a2", local_id: "ACT-007", title: "Zelt prüfen", due_date: null }),
        ],
      }),
    );

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    const rows = within(table).getAllByRole("row");
    const link = within(rows[1] as HTMLElement).getByRole("link", { name: "FLY28/ACT-042" });
    expect(link).toHaveAttribute("href", `/events/${EVENT_ID}/actions`);
    expect(within(rows[1] as HTMLElement).getByText("Generator bestellen")).toBeInTheDocument();
    expect(within(rows[1] as HTMLElement).getByText("18.05.2030")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("FLY28/ACT-007")).toBeInTheDocument();
  });

  it("lists my commitments with the full reference and the promisor", async () => {
    renderPage(work({ commitments: [commitment()] }));

    const table = await screen.findByRole("table", { name: "Zusagen" });
    expect(within(table).getByRole("link", { name: "FLY28/COM-003" })).toHaveAttribute(
      "href",
      `/events/${EVENT_ID}/commitments`,
    );
    expect(within(table).getByText("Testwil Generatoren AG")).toBeInTheDocument();
    expect(within(table).getByText("bedingt")).toBeInTheDocument();
  });

  it("marks a due date before today as overdue, but not today", async () => {
    renderPage(
      work({
        actions: [
          action({ id: "a1", local_id: "ACT-001", due_date: "2030-05-15" }),
          action({ id: "a2", local_id: "ACT-002", due_date: "2030-05-16" }),
        ],
      }),
    );

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    const rows = within(table).getAllByRole("row");
    expect(within(rows[1] as HTMLElement).getByText("überfällig")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).queryByText("überfällig")).not.toBeInTheDocument();
  });

  it("links the review count to the Inbox", async () => {
    renderPage(work({ review_count: 3, actions: [action()] }));

    const link = await screen.findByRole("link", { name: "3 Vorschläge warten auf Ihre Prüfung" });
    expect(link).toHaveAttribute("href", "/inbox");
  });

  it("shows the empty state", async () => {
    renderPage(work({}));

    expect(await screen.findByText("Nichts offen")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("shows a failure with a retry", async () => {
    renderPage(
      () =>
        new Response(JSON.stringify({ code: "internal", title: "", status: 500 }), {
          status: 500,
          headers: { "Content-Type": "application/problem+json" },
        }),
    );

    expect(await screen.findByRole("button", { name: "Erneut versuchen" })).toBeInTheDocument();
  });
});

describe("isOverdue", () => {
  it("is true only before today", () => {
    expect(isOverdue("2030-05-15", "2030-05-16")).toBe(true);
    expect(isOverdue("2030-05-16", "2030-05-16")).toBe(false);
    expect(isOverdue(null, "2030-05-16")).toBe(false);
  });
});
