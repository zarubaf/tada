import { render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApi } from "../api/client";
import { Router } from "../router/Router";
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
  it("has one h1 in the toolbar row of the page", async () => {
    renderPage(work({ actions: [action()] }));

    const heading = await screen.findByRole("heading", { level: 1, name: "Meine Arbeit" });
    expect(heading.closest("[data-page-toolbar]")).not.toBeNull();
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getAllByRole("main")).toHaveLength(1);
  });

  it("lists my actions with the reference, the title as the link to the panel, status and date", async () => {
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
    const first = rows[1] as HTMLElement;
    expect(within(first).getByText("FLY28/ACT-042")).toBeInTheDocument();
    // The reference is text; the title is the link, and it leads to the panel of the record.
    expect(within(first).queryByRole("link", { name: /ACT-042/ })).not.toBeInTheDocument();
    expect(within(first).getByRole("link", { name: "Generator bestellen" })).toHaveAttribute(
      "href",
      `/events/${EVENT_ID}/work#ACT-042`,
    );
    expect(within(first).getByText("offen")).toBeInTheDocument();
    expect(within(first).getByText("18.05.2030")).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).getByText("FLY28/ACT-007")).toBeInTheDocument();
  });

  it("names each section with a heading and the number of its rows", async () => {
    renderPage(
      work({
        actions: [action({ id: "a1" }), action({ id: "a2", local_id: "ACT-007" })],
        commitments: [commitment()],
      }),
    );

    const actions = await screen.findByRole("heading", { level: 2, name: /Aufgaben/ });
    expect(actions).toHaveTextContent("2");
    expect(screen.getByRole("heading", { level: 2, name: /Zusagen/ })).toHaveTextContent("1");
  });

  it("lists my commitments with the reference, the promisor and the status", async () => {
    renderPage(work({ commitments: [commitment()] }));

    const table = await screen.findByRole("table", { name: "Zusagen" });
    expect(within(table).getByText("FLY28/COM-003")).toBeInTheDocument();
    expect(within(table).getByRole("link", { name: "Lieferung am Freitag" })).toHaveAttribute(
      "href",
      `/events/${EVENT_ID}/work#COM-003`,
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
    expect(within(rows[1] as HTMLElement).getByText(/überfällig/)).toBeInTheDocument();
    expect(within(rows[2] as HTMLElement).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("does not mark a finished action as overdue", async () => {
    renderPage(work({ actions: [action({ due_date: "2030-05-15", status: "done" })] }));

    const table = await screen.findByRole("table", { name: "Aufgaben" });
    expect(within(table).queryByText(/überfällig/)).not.toBeInTheDocument();
  });

  it("puts the review count in the toolbar row as a link to the Inbox", async () => {
    renderPage(work({ review_count: 3, actions: [action()] }));

    const link = await screen.findByRole("link", { name: "3 Vorschläge warten auf Ihre Prüfung" });
    expect(link).toHaveAttribute("href", "/inbox");
    expect(link.closest("[data-page-toolbar]")).not.toBeNull();
  });

  it("has no review link without proposals to review", async () => {
    renderPage(work({ actions: [action()] }));

    await screen.findByRole("table", { name: "Aufgaben" });
    expect(screen.queryByRole("link", { name: /Prüfung/ })).not.toBeInTheDocument();
  });

  it("shows the empty state under the toolbar row", async () => {
    renderPage(work({}));

    expect(await screen.findByText("Nichts offen")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Meine Arbeit" })).toBeInTheDocument();
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
