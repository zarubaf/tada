import {
  IconCalendarEvent,
  IconCircleCheck,
  IconCircleDashed,
  IconHourglass,
  IconStack2,
  IconUser,
  IconUserCheck,
} from "@tabler/icons-react";
import type { Commitment, CommitmentStatus, CreateCommitmentRequest } from "../../api/client";
import { t } from "../../i18n";
import type { WorkItem } from "../workItem";
import type { FieldSpec, FieldValue, MenuAction, RecordDescriptor } from "./descriptor";

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
  read: (item) => record(item).promisor.name,
};

const owner: FieldSpec = {
  id: "owner",
  labelKey: "record-field-owner",
  icon: IconUser,
  control: "person",
  pointer: "owner",
  promptKey: "record-choose-owner",
  required: true,
  read: (item) => asCommitment(item).owner.id,
  save: (commands, item, value) => commands.commitment.changeOwner(target(item), value ?? ""),
};

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

const area: FieldSpec = {
  id: "area",
  labelKey: "record-field-area",
  icon: IconStack2,
  control: "area",
  pointer: "workstream",
  emptyKey: "record-no-area",
  read: (item) => asCommitment(item).workstreamId ?? null,
  save: (commands, item, value) => commands.commitment.changeWorkstream(target(item), value),
};

/** „Verbindlich machen“: the one way from conditional to firm, with the reason that the commitment keeps. */
const makeFirm: MenuAction = {
  id: "make-firm",
  labelKey: "make-firm",
  icon: IconCircleCheck,
  available: (item) => record(item).can_make_firm,
  step: {
    title: (item) => t("make-firm-title", { id: item.readableId }),
    description: (item) => {
      const given = record(item).condition;
      return [
        t("make-firm-text"),
        ...(given ? [t("commitment-condition-line", { condition: given })] : []),
      ];
    },
    reasonKey: "make-firm-reason",
    reasonHelpKey: "make-firm-reason-help",
    confirmKey: "make-firm-submit",
  },
  run: (commands, item, reason) => commands.commitment.makeFirm(target(item), reason),
  doneKey: "make-firm-done",
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

function body(values: Record<string, FieldValue>, id: string): CreateCommitmentRequest {
  const condition = optional(values.condition ?? null);
  const due = optional(values.due ?? null);
  // The value of the party control is `person:{uuid}` or `institution:{uuid}`.
  const [kind, party = ""] = (values.promisor ?? "").split(":");
  return {
    id,
    text: optional(values.text ?? null) ?? "",
    promisor: { kind: kind === "person" ? "person" : "institution", id: party },
    owner_user_id: values.owner ?? "",
    ...(condition && { condition }),
    ...(values.area && { workstream_id: values.area }),
    ...(due && { due_date: due }),
  };
}

export const commitmentDescriptor: RecordDescriptor = {
  kind: "commitment",
  title: text,
  properties: [status, promisor, owner, due, condition, area],
  evidenceVersionKey: "commitment-evidence-version",
  menu: [makeFirm],
  create: {
    titleKey: "record-create-commitment",
    fields: ["text", "promisor", "condition", "owner", "due", "area"],
    defaults: (owner) => ({ text: "", owner: owner ?? null }),
    submit: (commands, values, id) => commands.commitment.create(body(values, id)),
  },
};
