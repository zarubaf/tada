import { readFileSync } from "node:fs";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ProfileState } from "../events/eventContext";
import { BERND, EVENT, ground, ME, renderWork } from "../test/fakeWorkServer";
import { TimelinePage } from "./TimelinePage";

const user = userEvent.setup({ delay: null });

function action(over: Record<string, unknown> = {}) {
  return {
    id: "a1",
    local_id: "ACT-001",
    event_id: EVENT.id,
    title: "Gesuch einreichen",
    redacted: false,
    owner_user_id: ME,
    workstream_id: "w1",
    start_date: "2030-05-06",
    due_date: "2030-05-10",
    status: "open",
    version: 1,
    evidence: [],
    can_change: true,
    next_statuses: [],
    ...over,
  };
}

const milestone = {
  id: "m1",
  local_id: "MIL-001",
  event_id: EVENT.id,
  title: "Bewilligung liegt vor",
  redacted: false,
  date: "2030-05-15",
  owner_user_id: ME,
  status: "open",
  warnings: [],
  version: 1,
  evidence: [],
  can_change: true,
  next_statuses: [],
};

const actions = [
  action(),
  action({
    id: "a2",
    local_id: "ACT-002",
    title: "Festzelt reservieren",
    owner_user_id: BERND,
    workstream_id: null,
    start_date: null,
    due_date: "2030-05-14",
    status: "blocked",
  }),
  action({
    id: "a3",
    local_id: "ACT-003",
    title: "Flyer drucken",
    start_date: null,
    due_date: null,
  }),
];

const profile: ProfileState = {
  kind: "loaded",
  fields: [],
  profile: {
    proposals: [],
    open_questions: [],
    facts: [
      {
        id: "f1",
        field_id: "fd1",
        field_key: "date_window",
        state: "assumption",
        value: { type: "date-window", start: "2030-05-13", end: "2030-05-17", granularity: "day" },
        redacted: false,
        version: 1,
        evidence: [],
        accepted_by: { kind: "member", id: ME, channel: "web" },
        accepted_at: "2030-03-01T13:12:00Z",
      },
    ],
  },
};

const render = (setup: Parameters<typeof renderWork>[1] = {}) =>
  renderWork((api) => <TimelinePage api={api} />, {
    profile,
    workstreams: [ground],
    ...setup,
    lists: { "/actions": actions, "/milestones": [milestone], ...setup.lists },
  });

const axis = () => screen.findByRole("region", { name: "Zeitachse" });

describe("TimelinePage", () => {
  it("each bar and point is a link to the action", async () => {
    render();
    const timeline = await axis();
    const bar = within(timeline).getByRole("link", {
      name: "ACT-001 Gesuch einreichen, 06.05.2030 bis 10.05.2030, offen",
    });
    expect(bar).toHaveAttribute("href", `/events/${EVENT.id}/actions`);
    const point = within(timeline).getByRole("link", {
      name: "ACT-002 Festzelt reservieren, fällig 14.05.2030, blockiert",
    });
    expect(point).toHaveAttribute("href", `/events/${EVENT.id}/actions`);
  });

  it("shows the lanes, the milestones, the event window and the undated count", async () => {
    render();
    const timeline = await axis();
    expect(within(timeline).getByRole("heading", { name: "Bodenbetrieb" })).toBeInTheDocument();
    expect(
      within(timeline).getByRole("heading", { name: "ohne Arbeitsbereich" }),
    ).toBeInTheDocument();
    expect(
      within(timeline).getByText("MIL-001 Bewilligung liegt vor, 15.05.2030, offen"),
    ).toBeInTheDocument();
    // The window is an assumption: the legend says so with its label.
    const legend = screen.getByRole("list", { name: "Legende" });
    expect(within(legend).getByText(/13\.05\.2030/)).toBeInTheDocument();
    expect(within(legend).getByText("Annahme")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "1 Aufgabe ohne Fälligkeit" })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/actions`,
    );
  });

  it("filters by owner and by status and resets the filters", async () => {
    render();
    const timeline = await axis();
    expect(within(timeline).getAllByRole("link")).toHaveLength(2);

    await user.click(screen.getByRole("button", { name: /Verantwortlich/ }));
    await user.click(screen.getByRole("option", { name: "Bernd Beispiel" }));
    expect(within(timeline).getAllByRole("link")).toHaveLength(1);
    expect(within(timeline).getByRole("link", { name: /ACT-002/ })).toBeInTheDocument();
    const filters = screen.getByRole("group", { name: "Filter" });
    expect(within(filters).getByText("2 Aufgaben ausgeblendet")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Status/ }));
    await user.click(screen.getByRole("option", { name: "offen" }));
    expect(screen.getByText("Keine Treffer für diese Filter.")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Filter zurücksetzen" })[0] as Element);
    const again = await axis();
    expect(within(again).getAllByRole("link")).toHaveLength(2);
  });

  it("the narrow list groups the items by week", async () => {
    render();
    const list = await screen.findByRole("region", { name: "Nach Wochen" });
    const weeks = within(list).getAllByRole("heading");
    expect(weeks.map((heading) => heading.textContent)).toEqual([
      "Woche vom 06.05.2030",
      "Woche vom 13.05.2030",
    ]);
    const second = within(list).getByRole("list", { name: "Woche vom 13.05.2030" });
    const rows = within(second).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("ACT-002"),
      expect.stringContaining("MIL-001"),
    ]);
    expect(within(rows[0] as HTMLElement).getByText("blockiert")).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).getByText("ohne Arbeitsbereich")).toBeInTheDocument();
  });

  it("long_span_scrolls_inside_the_timeline", async () => {
    render({
      lists: {
        "/actions": [action({ start_date: "2030-05-06", due_date: "2033-05-01" })],
        "/milestones": [],
      },
    });
    const timeline = await axis();
    // The scroll container is the timeline: a keyboard member can reach it and scroll it.
    expect(timeline).toHaveAttribute("tabindex", "0");
    const canvas = timeline.firstElementChild as HTMLElement;
    expect(canvas.style.getPropertyValue("--weeks")).toBe("156");
    // The timeline scrolls; the page does not, because the root may shrink below its content.
    const css = readFileSync("src/timeline/Timeline.module.css", "utf8");
    expect(css).toMatch(/\.scroller\s*{[^}]*overflow-x:\s*auto/);
    expect(css).toMatch(/\.timeline\s*{[^}]*min-width:\s*0/);
  });

  const noWindow: ProfileState = {
    kind: "loaded",
    fields: [],
    profile: { facts: [], proposals: [], open_questions: [] },
  };

  it("shows the empty state for an event without dates, milestones and window", async () => {
    render({
      profile: noWindow,
      lists: { "/actions": [action({ start_date: null, due_date: null })], "/milestones": [] },
    });
    expect(await screen.findByText("Noch nichts im Zeitplan")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Zeitachse" })).not.toBeInTheDocument();
  });

  it("shows the milestones of an event without dated actions", async () => {
    render({
      lists: {
        "/actions": [action({ start_date: null, due_date: null })],
        "/milestones": [milestone],
      },
    });
    const timeline = await axis();
    expect(screen.queryByText("Noch nichts im Zeitplan")).not.toBeInTheDocument();
    expect(
      within(timeline).getByText("MIL-001 Bewilligung liegt vor, 15.05.2030, offen"),
    ).toBeInTheDocument();
    expect(document.querySelector("[data-band]")).not.toBeNull();
  });

  it("shows no band and an unknown window when the profile has no window", async () => {
    render({
      profile: {
        kind: "loaded",
        fields: [],
        profile: { facts: [], proposals: [], open_questions: [] },
      },
    });
    await axis();
    const legend = screen.getByRole("list", { name: "Legende" });
    expect(within(legend).getByText("Unbekannt")).toBeInTheDocument();
    await waitFor(() => expect(document.querySelector("[data-band]")).toBeNull());
  });
});
