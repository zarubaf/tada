import { IconCalendar, IconCircleDashed } from "@tabler/icons-react";
import type { Milestone, MilestoneStatus } from "../../api/client";
import { milestoneWarningText } from "../milestoneWarnings";
import type { WorkItem } from "../workItem";
import { areaField, ownerField } from "./commonFields";
import type { FieldSpec, FieldValue, RecordDescriptor } from "./descriptor";

type MilestoneItem = Extract<WorkItem, { kind: "milestone" }>;

/** The item as a milestone. A descriptor only meets items of its kind. */
function asMilestone(item: WorkItem): MilestoneItem {
  if (item.kind !== "milestone") {
    throw new Error(`the milestone descriptor met a ${item.kind}`);
  }
  return item;
}

const target = (item: WorkItem) => ({ id: item.id, version: item.version });
const record = (item: WorkItem): Milestone => asMilestone(item).record;

const title: FieldSpec = {
  id: "title",
  labelKey: "record-field-title",
  control: "text",
  pointer: "title",
  required: true,
  read: (item) => asMilestone(item).title,
  save: (commands, item, value) => commands.milestone.rename(target(item), value ?? ""),
};

const status: FieldSpec = {
  id: "status",
  labelKey: "record-field-status",
  icon: IconCircleDashed,
  control: "status",
  pointer: "status",
  required: true,
  read: (item) => asMilestone(item).status,
  options: (item) => [asMilestone(item).status, ...record(item).next_statuses],
  save: (commands, item, value) =>
    commands.milestone.changeStatus(target(item), value as MilestoneStatus),
};

// The date of a milestone cannot be cleared: it has no words for no date and is required.
const date: FieldSpec = {
  id: "date",
  labelKey: "record-field-date",
  icon: IconCalendar,
  control: "date",
  pointer: "date",
  required: true,
  overdue: true,
  read: (item) => asMilestone(item).record.date,
  save: (commands, item, value) => commands.milestone.changeDue(target(item), value ?? ""),
};

const owner = ownerField(
  (item) => asMilestone(item).owner.id,
  (commands, item, value) => commands.milestone.changeOwner(target(item), value ?? ""),
);

const area = areaField(
  (item) => asMilestone(item).workstreamId ?? null,
  (commands, item, value) => commands.milestone.changeWorkstream(target(item), value),
);

const description: FieldSpec = {
  id: "description",
  labelKey: "record-field-description",
  control: "multiline",
  pointer: "description",
  emptyKey: "record-no-description",
  read: (item) => record(item).description ?? null,
  save: (commands, item, value) => commands.milestone.changeDescription(target(item), value),
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

export const milestoneDescriptor: RecordDescriptor = {
  kind: "milestone",
  title,
  properties: [status, date, owner, area],
  description,
  hints: (item, lookup) =>
    record(item).warnings.map((warning) =>
      milestoneWarningText(warning, (id) => lookup.readableId("action", id)),
    ),
  evidenceVersionKey: "milestone-evidence-version",
  create: {
    titleKey: "record-create-milestone",
    fields: ["title", "date", "owner", "area", "description"],
    defaults: (owner) => ({ title: "", date: null, owner: owner ?? null }),
    submit: (commands, values, id) => {
      const text = optional(values.description ?? null);
      return commands.milestone.create({
        id,
        title: optional(values.title ?? null) ?? "",
        date: values.date ?? "",
        owner_user_id: values.owner ?? "",
        ...(text && { description: text }),
        ...(values.area && { workstream_id: values.area }),
      });
    },
  },
};
