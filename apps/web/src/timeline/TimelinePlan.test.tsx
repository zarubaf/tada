import { readFileSync } from "node:fs";
import { render as mount, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Action, Milestone, Workstream } from "../api/client";
import type { ProfileState } from "../events/eventContext";
import { Router } from "../router/Router";
import { BERND, EVENT, ground, ME } from "../test/fakeWorkServer";
import { TimelinePlan } from "./TimelinePlan";

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

interface Setup {
  profile?: ProfileState;
  actions?: unknown[];
  milestones?: unknown[];
}

const render = ({ profile: shown = profile, ...records }: Setup = {}) =>
  mount(
    <Router>
      <TimelinePlan
        actions={(records.actions ?? actions) as Action[]}
        milestones={(records.milestones ?? [milestone]) as Milestone[]}
        workstreams={[ground] as Workstream[]}
        profile={shown}
        today="2030-05-08"
        base={`/events/${EVENT.id}`}
        nameOf={(userId) => (userId === BERND ? "Bernd Beispiel" : "Anna Muster")}
        filtered={false}
      />
    </Router>,
  );

const axis = () => screen.findByRole("region", { name: "Zeitachse" });

describe("TimelinePlan", () => {
  it("each bar and point is a link to the action", async () => {
    render();
    const timeline = await axis();
    const bar = within(timeline).getByRole("link", {
      name: "ACT-001 Gesuch einreichen, 06.05.2030 bis 10.05.2030, offen",
    });
    expect(bar).toHaveAttribute("href", `/events/${EVENT.id}/work#ACT-001`);
    const point = within(timeline).getByRole("link", {
      name: "ACT-002 Festzelt reservieren, fällig 14.05.2030, blockiert",
    });
    expect(point).toHaveAttribute("href", `/events/${EVENT.id}/work#ACT-002`);
  });

  it("each milestone diamond is a link to the milestone", async () => {
    render();
    const timeline = await axis();
    const diamond = within(timeline).getByRole("link", { name: /MIL-001/ });
    expect(diamond).toHaveAttribute("href", `/events/${EVENT.id}/work#MIL-001`);
    expect(diamond).not.toHaveAttribute("tabindex", "-1");
  });

  it("shows the lanes, the milestones, the event window and the undated count", async () => {
    render();
    const timeline = await axis();
    expect(within(timeline).getByRole("heading", { name: "Bodenbetrieb" })).toBeInTheDocument();
    expect(within(timeline).getByRole("heading", { name: "ohne Bereich" })).toBeInTheDocument();
    expect(
      within(timeline).getByRole("link", {
        name: "MIL-001 Bewilligung liegt vor, 15.05.2030, offen",
      }),
    ).toBeInTheDocument();
    // The window is an assumption: the legend says so with its label.
    const legend = screen.getByRole("list", { name: "Legende" });
    expect(within(legend).getByText(/13\.05\.2030/)).toBeInTheDocument();
    expect(within(legend).getByText("Annahme")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "1 Aufgabe ohne Fälligkeit" })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work?type=action`,
    );
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
    expect(within(rows[0] as HTMLElement).getByText("ohne Bereich")).toBeInTheDocument();
  });

  it("long_span_scrolls_inside_the_timeline", async () => {
    render({
      actions: [action({ start_date: "2030-05-06", due_date: "2033-05-01" })],
      milestones: [],
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

  it("shows the milestones of an event without dated actions", async () => {
    render({ actions: [action({ start_date: null, due_date: null })] });
    const timeline = await axis();
    expect(
      within(timeline).getByRole("link", {
        name: "MIL-001 Bewilligung liegt vor, 15.05.2030, offen",
      }),
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

  it("shows the status of each row as text, not by color alone", async () => {
    render();
    const timeline = await axis();
    expect(within(timeline).getByText("blockiert")).toBeInTheDocument();
    expect(within(timeline).getAllByText("offen")).toHaveLength(2);
  });

  it("links the rows of the list to the panel of the record", async () => {
    render();
    const list = await screen.findByRole("region", { name: "Nach Wochen" });
    expect(within(list).getByRole("link", { name: /ACT-002/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work#ACT-002`,
    );
    expect(within(list).getByRole("link", { name: /MIL-001/ })).toHaveAttribute(
      "href",
      `/events/${EVENT.id}/work#MIL-001`,
    );
  });

  it("keeps the list for assistive technology at a wide width and out of the tab order", async () => {
    widen(1000);
    render();
    const timeline = await axis();
    const list = await screen.findByRole("region", { name: "Nach Wochen" });
    // The list stays in the accessibility tree; its links leave the tab order of the chart's links.
    const listLinks = within(list).getAllByRole("link");
    expect(listLinks.length).toBeGreaterThan(0);
    for (const link of listLinks) {
      expect(link).toHaveAttribute("tabindex", "-1");
    }
    for (const link of within(timeline).getAllByRole("link")) {
      expect(link).not.toHaveAttribute("tabindex", "-1");
    }
  });

  it("lets the list take the keyboard at a narrow width", async () => {
    widen(500);
    render();
    const list = await screen.findByRole("region", { name: "Nach Wochen" });
    for (const link of within(list).getAllByRole("link")) {
      expect(link).not.toHaveAttribute("tabindex");
    }
  });

  it("shows a line in the list when nothing is on the axis but the window", async () => {
    render({ actions: [], milestones: [] });
    const list = await screen.findByRole("region", { name: "Nach Wochen" });
    expect(within(list).getByText("Keine Einträge in dieser Ansicht.")).toBeInTheDocument();
  });

  it("names a month window by its month", async () => {
    const month = structuredClone(profile);
    if (month.kind === "loaded" && month.profile.facts[0]) {
      month.profile.facts[0].value = {
        type: "date-window",
        start: "2030-05-01",
        end: "2030-05-31",
        granularity: "month",
      };
    }
    render({ profile: month });
    await axis();
    const legend = screen.getByRole("list", { name: "Legende" });
    expect(within(legend).getByText("Mai 2030")).toBeInTheDocument();
  });

  it("draws no node for each week and few labels for a span of years", async () => {
    render({
      actions: [action({ start_date: "2030-05-06", due_date: "2033-05-01" })],
      milestones: [],
    });
    const timeline = await axis();
    expect(timeline.querySelectorAll("[class*='weekLine']")).toHaveLength(0);
    const labels = timeline.querySelectorAll("[class*='axisLabel']").length;
    expect(labels).toBeGreaterThan(10);
    expect(labels).toBeLessThanOrEqual(45);
  });
});

/** jsdom has no layout: the observer reports the width of the container. */
function widen(width: number) {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        this.callback([{ target, contentRect: { width } } as ResizeObserverEntry], this);
      }
      unobserve() {}
      disconnect() {}
    },
  );
}

afterEach(() => vi.unstubAllGlobals());
