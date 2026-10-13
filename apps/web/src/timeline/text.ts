// The words of the timeline items, shared by the time axis and the list grouped by week.
import type { Action, Milestone, Workstream } from "../api/client";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";

/** The title of a record, or „entfernt“ if it is redacted (ADR 0072). */
export function titleOf(record: { redacted: boolean; title: string }): string {
  return record.redacted ? t("redacted") : record.title;
}

/** The dates of a dated action: „06.05.2030 bis 10.05.2030“ or „fällig 10.05.2030“. */
export function actionWhen(action: Action): string {
  const due = formatDate(action.due_date ?? "");
  return action.start_date
    ? t("timeline-span", { from: formatDate(action.start_date), to: due })
    : t("timeline-due", { date: due });
}

/** The full name of an action on the timeline: ID, title, dates and status. */
export function actionLabel(action: Action): string {
  return t("timeline-action", {
    id: action.local_id,
    title: titleOf(action),
    when: actionWhen(action),
    status: t(`action-status-${action.status}`),
  });
}

/** The full name of a milestone on the timeline: ID, title, date and status. */
export function milestoneLabel(milestone: Milestone): string {
  return t("timeline-milestone", {
    id: milestone.local_id,
    title: titleOf(milestone),
    date: formatDate(milestone.date),
    status: t(`milestone-status-${milestone.status}`),
  });
}

/** The name of a lane: the workstream, or „ohne Arbeitsbereich“. */
export function laneName(workstream: Workstream | null): string {
  if (!workstream) {
    return t("timeline-lane-none");
  }
  return workstream.redacted ? t("redacted") : workstream.name;
}
