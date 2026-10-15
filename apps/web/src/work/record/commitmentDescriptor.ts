import {
  IconCalendarEvent,
  IconCircleCheck,
  IconCircleDashed,
  IconHourglass,
  IconLock,
  IconUserCheck,
} from "@tabler/icons-react";
import type { Commitment, CommitmentStatus, CreateCommitmentRequest } from "../../api/client";
import type { WorkItem } from "../workItem";
import { MakeFirmDialog } from "./commitmentDialogs";
import { areaField, ownerField } from "./commonFields";
import {
  type FieldSpec,
  type FieldValue,
  REDACTED,
  type RecordActionSpec,
  type RecordDescriptor,
} from "./descriptor";
import { partyOf } from "./parties";

type CommitmentItem = Extract<WorkItem, { kind: "commitment" }>;

/** The item as a commitment. A descriptor only meets items of its kind. */
function asCommitment(item: WorkItem): CommitmentItem {
  if (item.kind !== "commitment") {
    throw new Error(`the commitment descriptor met a ${item.kind}`);
  }
  return item;
}

const target = (item: WorkItem) => ({ id: item.id, version: item.version });
const record = (item: WorkItem): Commitment => asCommitment(item).record;

const text: FieldSpec = {
  id: "text",
  labelKey: "record-field-commitment-text",
  control: "text",
  pointer: "text",
  required: true,
  read: (item) => asCommitment(item).title,
  save: (commands, item, value) => commands.commitment.rename(target(item), value ?? ""),
};

/**
 * The status and the next ones. `firm` is never a choice of the member: only „Verbindlich machen“
 * reaches it, with a reason (ADR 0068). A firm commitment still shows it as its own status.
 */
const status: FieldSpec = {
  id: "status",
  labelKey: "record-field-status",
  icon: IconCircleDashed,
  control: "status",
  pointer: "status",
  required: true,
  read: (item) => asCommitment(item).status,
  options: (item) => [
    asCommitment(item).status,
    ...record(item).next_statuses.filter((next) => next !== "firm"),
  ],
  save: (commands, item, value) =>
    commands.commitment.changeStatus(target(item), value as Exclude<CommitmentStatus, "firm">),
};

/** The promisor never changes (ADR 0069), so the panel shows its name and the form chooses it. */
const promisor: FieldSpec = {
  id: "promisor",
  labelKey: "record-field-promisor",
  icon: IconUserCheck,
  control: "party",
  pointer: "promisor",
  promptKey: "commitment-field-promisor-placeholder",
  required: true,
  read: (item) => (record(item).promisor.redacted ? REDACTED : record(item).promisor.name),
};

const owner = ownerField(
  (item) => asCommitment(item).owner.id,
  (commands, item, value) => commands.commitment.changeOwner(target(item), value ?? ""),
);

const due: FieldSpec = {
  id: "due",
  labelKey: "record-field-due",
  icon: IconCalendarEvent,
  control: "date",
  pointer: "due_date",
  emptyKey: "record-no-date",
  overdue: true,
  read: (item) => record(item).due_date ?? null,
  save: (commands, item, value) => commands.commitment.changeDue(target(item), value),
};

/**
 * The condition never changes and stays after the commitment is firm (ADR 0068). Only the create
 * form takes it; a commitment with a condition starts conditional.
 */
const condition: FieldSpec = {
  id: "condition",
  labelKey: "record-field-condition",
  icon: IconHourglass,
  control: "text",
  pointer: "condition",
  helpKey: "commitment-field-condition-help",
  emptyKey: "record-no-condition",
  read: (item) => record(item).condition ?? null,
};

/** The reason of „Verbindlich machen“. A commitment that started firm has none, so the row is absent. */
const firmReason: FieldSpec = {
  id: "firmReason",
  labelKey: "record-field-firm-reason",
  icon: IconLock,
  control: "text",
  pointer: "reason",
  hideEmpty: true,
  read: (item) => record(item).firm_reason ?? null,
};

const area = areaField(
  (item) => asCommitment(item).workstreamId ?? null,
  (commands, item, value) => commands.commitment.changeWorkstream(target(item), value),
);

/** „Verbindlich machen“: the one way from conditional to firm, with the reason that the commitment keeps. */
const makeFirm: RecordActionSpec = {
  id: "make-firm",
  labelKey: "make-firm",
  icon: IconCircleCheck,
  available: (item, env) => env.editable && record(item).can_make_firm,
  dialog: MakeFirmDialog,
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

function body(values: Record<string, FieldValue>, id: string): CreateCommitmentRequest {
  const condition = optional(values.condition ?? null);
  const due = optional(values.due ?? null);
  return {
    id,
    text: optional(values.text ?? null) ?? "",
    promisor: partyOf(values.promisor ?? ""),
    owner_user_id: values.owner ?? "",
    ...(condition && { condition }),
    ...(values.area && { workstream_id: values.area }),
    ...(due && { due_date: due }),
  };
}

export const commitmentDescriptor: RecordDescriptor = {
  kind: "commitment",
  title: text,
  properties: [status, promisor, owner, due, condition, firmReason, area],
  evidenceVersionKey: "commitment-evidence-version",
  actions: [makeFirm],
  create: {
    titleKey: "record-create-commitment",
    fields: ["text", "promisor", "condition", "owner", "due", "area"],
    defaults: (owner) => ({ text: "", owner: owner ?? null }),
    submit: (commands, values, id) => commands.commitment.create(body(values, id)),
  },
};
