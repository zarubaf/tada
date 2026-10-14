import type { Action, Milestone, RecordKind } from "../api/client";
import type { WorkItem } from "./workItem";

/** The kinds that have a place on the timeline. Commitments, decisions and risks do not. */
export const TIMELINE_KINDS: readonly RecordKind[] = ["action", "milestone"];

/** The actions and the milestones among the work items, as the records that the layout reads. */
export function timelineRecords(items: readonly WorkItem[]): {
  actions: Action[];
  milestones: Milestone[];
} {
  const actions: Action[] = [];
  const milestones: Milestone[] = [];
  for (const item of items) {
    if (item.kind === "action") {
      actions.push(item.record);
    } else if (item.kind === "milestone") {
      milestones.push(item.record);
    }
  }
  return { actions, milestones };
}

/** True when the type filter names kinds and none of them is on the timeline. */
export function onlyOffTimeline(kinds: readonly RecordKind[]): boolean {
  return kinds.length > 0 && !kinds.some((kind) => TIMELINE_KINDS.includes(kind));
}
