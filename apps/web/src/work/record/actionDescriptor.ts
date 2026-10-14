import {
  IconCalendar,
  IconCalendarEvent,
  IconCircleDashed,
  IconDiamond,
  IconStack2,
  IconUser,
} from "@tabler/icons-react";
import type { Action, ActionStatus, CreateActionRequest } from "../../api/client";
import { t } from "../../i18n";
import type { WorkItem } from "../workItem";
import type { FieldSpec, FieldValue, RecordDescriptor } from "./descriptor";

type ActionItem = Extract<WorkItem, { kind: "action" }>;

/** The item as an action. A descriptor only meets items of its kind. */
function asAction(item: WorkItem): ActionItem {
  if (item.kind !== "action") {
    throw new Error(`the action descriptor met a ${item.kind}`);
  }
  return item;
}

/** The start of an action is not after its due date (the same rule as in the server). */
const startAfterDue = (start: FieldValue, due: FieldValue) =>
  start !== null && due !== null && start > due;

const target = (item: WorkItem) => ({ id: item.id, version: item.version });
const record = (item: WorkItem): Action => asAction(item).record;

const title: FieldSpec = {
  id: "title",
  labelKey: "record-field-title",
  control: "text",
  pointer: "title",
  required: true,
  read: (item) => asAction(item).title,
  save: (commands, item, value) => commands.action.rename(target(item), value ?? ""),
};

const status: FieldSpec = {
  id: "status",
  labelKey: "record-field-status",
  icon: IconCircleDashed,
  control: "status",
  pointer: "status",
  required: true,
  read: (item) => asAction(item).status,
  statuses: (item) => [asAction(item).status, ...record(item).next_statuses],
  save: (commands, item, value) =>
    commands.action.changeStatus(target(item), value as ActionStatus),
};

const owner: FieldSpec = {
  id: "owner",
  labelKey: "record-field-owner",
  icon: IconUser,
  control: "person",
  pointer: "owner",
  required: true,
  read: (item) => asAction(item).owner.id,
  save: (commands, item, value) => commands.action.changeOwner(target(item), value ?? ""),
};

const due: FieldSpec = {
  id: "due",
  labelKey: "record-field-due",
  icon: IconCalendarEvent,
  control: "date",
  pointer: "due_date",
  elsewhere: ["start_date"],
  validate: (item, value) =>
    startAfterDue(record(item).start_date ?? null, value)
      ? t("work-error-due_date-start-after-due")
      : undefined,
  emptyKey: "record-no-date",
  overdue: true,
  read: (item) => record(item).due_date ?? null,
  save: (commands, item, value) => commands.action.changeDue(target(item), value),
};

const start: FieldSpec = {
  id: "start",
  labelKey: "record-field-start",
  icon: IconCalendar,
  control: "date",
  pointer: "start_date",
  validate: (item, value) =>
    startAfterDue(value, record(item).due_date ?? null)
      ? t("work-error-start_date-start-after-due")
      : undefined,
  emptyKey: "record-no-date",
  read: (item) => record(item).start_date ?? null,
  save: (commands, item, value) => commands.action.changeStart(target(item), value),
};

const area: FieldSpec = {
  id: "area",
  labelKey: "record-field-area",
  icon: IconStack2,
  control: "area",
  pointer: "workstream",
  emptyKey: "record-no-area",
  read: (item) => asAction(item).workstreamId ?? null,
  save: (commands, item, value) => commands.action.changeWorkstream(target(item), value),
};

const milestone: FieldSpec = {
  id: "milestone",
  labelKey: "record-field-milestone",
  icon: IconDiamond,
  control: "milestone",
  pointer: "milestone",
  emptyKey: "record-no-milestone",
  read: (item) => record(item).milestone_id ?? null,
  save: (commands, item, value) => commands.action.changeMilestone(target(item), value),
};

const description: FieldSpec = {
  id: "description",
  labelKey: "record-field-description",
  control: "multiline",
  pointer: "description",
  emptyKey: "record-no-description",
  read: (item) => record(item).description ?? null,
  save: (commands, item, value) => commands.action.changeDescription(target(item), value),
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

function body(values: Record<string, FieldValue>): CreateActionRequest {
  const description = optional(values.description ?? null);
  const start = optional(values.start ?? null);
  const due = optional(values.due ?? null);
  return {
    title: optional(values.title ?? null) ?? "",
    owner_user_id: values.owner ?? "",
    ...(description && { description }),
    ...(values.area && { workstream_id: values.area }),
    ...(start && { start_date: start }),
    ...(due && { due_date: due }),
    ...(values.milestone && { milestone_id: values.milestone }),
  };
}

export const actionDescriptor: RecordDescriptor = {
  kind: "action",
  title,
  properties: [status, owner, due, start, area, milestone],
  description,
  evidenceVersionKey: "action-evidence-version",
  create: {
    titleKey: "record-create-action",
    fields: ["title", "owner", "due", "start", "area", "milestone", "description"],
    defaults: (me) => ({ title: "", owner: me }),
    validate: (values): Record<string, string> =>
      startAfterDue(optional(values.start ?? null) ?? null, optional(values.due ?? null) ?? null)
        ? {
            start: t("work-error-start_date-start-after-due"),
            due: t("work-error-due_date-start-after-due"),
          }
        : {},
    submit: (commands, values) => commands.action.create(body(values)),
  },
};
