// The geometry of the timeline of an event (doc/design/layout-and-responsiveness.md, „Timeline“).
// Pure: no React and no requests, so that the tests cover each position. A position counts the
// days from the Monday of the first week. Dates are civil dates (`2030-05-18`).
import type { Action, Fact, Milestone, Workstream } from "../api/client";

export type TimelineItem =
  /** An action with a start date and a due date. `to` is the day after the due date. */
  | { kind: "bar"; action: Action; from: number; to: number }
  /** An action with only a due date. */
  | { kind: "point"; action: Action; at: number };

export interface TimelineLane {
  /** The workstream of the lane; `null` is the lane „ohne Arbeitsbereich“. */
  workstream: Workstream | null;
  items: TimelineItem[];
}

export interface TimelineLayout {
  /** The Monday of each week on the axis. Empty when nothing has a date. */
  weeks: string[];
  lanes: TimelineLane[];
  milestones: { milestone: Milestone; at: number }[];
  /** The event window; `to` is the day after its end. */
  window: { from: number; to: number } | null;
  /** Today, if it is on the axis. */
  today: number | null;
  /** The actions without a due date. They have no place on the axis. */
  undated: number;
}

export interface LayoutInput {
  actions: Action[];
  milestones: Milestone[];
  workstreams: Workstream[];
  window: { start: string; end: string } | null;
  today: string;
}

const DAY = 86_400_000;

function epochDay(date: string): number {
  return Math.floor(Date.parse(`${date}T00:00:00Z`) / DAY);
}

function civil(day: number): string {
  return new Date(day * DAY).toISOString().slice(0, 10);
}

/** The Monday on or before `day`. Day 0 of the epoch (1970-01-01) is a Thursday. */
function monday(day: number): number {
  return day - ((day + 3) % 7);
}

/**
 * Lays out the actions, milestones and the event window on an axis in weeks. The axis runs from
 * the earliest to the latest date of all of them: the start and due dates of the actions, the
 * milestones and the event window. An event with no dated action, no milestone and no window gets
 * no axis.
 */
export function layout({
  actions,
  milestones,
  workstreams,
  window,
  today,
}: LayoutInput): TimelineLayout {
  const dated = actions.filter((action) => action.due_date);
  const undated = actions.length - dated.length;
  if (dated.length === 0 && milestones.length === 0 && !window) {
    return { weeks: [], lanes: [], milestones: [], window: null, today: null, undated };
  }

  const days = [
    ...dated.flatMap((action) => [action.start_date, action.due_date]),
    ...milestones.map((milestone) => milestone.date),
    ...(window ? [window.start, window.end] : []),
  ]
    .filter((date): date is string => Boolean(date))
    .map(epochDay);
  const first = monday(Math.min(...days));
  const count = Math.floor((Math.max(...days) - first) / 7) + 1;
  const at = (date: string) => epochDay(date) - first;

  const item = (action: Action): TimelineItem => {
    const due = at(action.due_date as string);
    return action.start_date
      ? { kind: "bar", action, from: at(action.start_date), to: due + 1 }
      : { kind: "point", action, at: due };
  };
  const start = (entry: TimelineItem) => (entry.kind === "bar" ? entry.from : entry.at);
  const itemsOf = (workstreamId: string | null) =>
    dated
      .filter((action) => (action.workstream_id ?? null) === workstreamId)
      .map(item)
      .sort((a, b) => start(a) - start(b) || a.action.local_id.localeCompare(b.action.local_id));

  // Each active workstream has a lane; a closed one only while it has items. The lane without a
  // workstream is last, and only when it has items.
  const known = new Set(workstreams.map((workstream) => workstream.id));
  const lanes: TimelineLane[] = workstreams
    .map((workstream) => ({ workstream, items: itemsOf(workstream.id) }))
    .filter((lane) => lane.workstream.status === "active" || lane.items.length > 0);
  // An action of a workstream that the list does not know joins the lane without a workstream.
  const rest = dated
    .filter((action) => !action.workstream_id || !known.has(action.workstream_id))
    .map(item)
    .sort((a, b) => start(a) - start(b) || a.action.local_id.localeCompare(b.action.local_id));
  if (rest.length > 0) {
    lanes.push({ workstream: null, items: rest });
  }

  const now = at(today);
  return {
    weeks: Array.from({ length: count }, (_, index) => civil(first + index * 7)),
    lanes,
    milestones: milestones
      .map((milestone) => ({ milestone, at: at(milestone.date) }))
      .sort((a, b) => a.at - b.at || a.milestone.local_id.localeCompare(b.milestone.local_id)),
    window: window ? { from: at(window.start), to: at(window.end) + 1 } : null,
    today: now >= 0 && now < count * 7 ? now : null,
    undated,
  };
}

export type WeekEntry =
  | { kind: "action"; item: TimelineItem; workstream: Workstream | null; day: number }
  | { kind: "milestone"; milestone: Milestone; day: number };

/**
 * The same items as a list grouped by week, for narrow layouts and screen readers: an action in
 * the week of its due date, a milestone in the week of its date. Weeks without items are left out.
 */
export function weekGroups(timeline: TimelineLayout): { week: string; entries: WeekEntry[] }[] {
  const entries: WeekEntry[] = [
    ...timeline.lanes.flatMap((lane) =>
      lane.items.map(
        (item): WeekEntry => ({
          kind: "action",
          item,
          workstream: lane.workstream,
          day: item.kind === "bar" ? item.to - 1 : item.at,
        }),
      ),
    ),
    ...timeline.milestones.map(
      ({ milestone, at }): WeekEntry => ({ kind: "milestone", milestone, day: at }),
    ),
  ].sort((a, b) => a.day - b.day);
  return timeline.weeks
    .map((week, index) => ({
      week,
      entries: entries.filter((entry) => Math.floor(entry.day / 7) === index),
    }))
    .filter((group) => group.entries.length > 0);
}

export interface EventWindow {
  start: string;
  end: string;
  field_key: "exact_dates" | "date_window";
  state: "accepted" | "assumption";
}

/**
 * The event window from the core facts (ADR 0080): the exact dates if they are accepted, else the
 * date window if it is accepted or an assumption. An unknown or a redacted fact gives no window.
 */
export function eventWindow(facts: readonly Fact[]): EventWindow | null {
  const windowOf = (key: EventWindow["field_key"], states: readonly Fact["state"][]) => {
    const fact = facts.find((candidate) => candidate.field_key === key);
    const value = fact?.value as { type?: string; start?: string; end?: string } | undefined;
    if (!fact || fact.redacted || !states.includes(fact.state) || value?.type !== "date-window") {
      return null;
    }
    if (!value.start || !value.end) {
      return null;
    }
    return {
      start: value.start,
      end: value.end,
      field_key: key,
      state: fact.state as EventWindow["state"],
    };
  };
  return (
    windowOf("exact_dates", ["accepted"]) ?? windowOf("date_window", ["accepted", "assumption"])
  );
}
