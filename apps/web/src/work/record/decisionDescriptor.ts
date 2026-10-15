import {
  IconBan,
  IconCalendar,
  IconCircleDashed,
  IconLink,
  IconMessageOff,
  IconReplace,
  IconUsers,
} from "@tabler/icons-react";
import type { Decision } from "../../api/client";
import type { WorkItem } from "../workItem";
import { areaField, ownerField } from "./commonFields";
import { RevokeDecisionDialog, SupersedeDecisionDialog } from "./decisionDialogs";
import { partiesOf, partyId, wordingError } from "./decisionInput";
import { type FieldSpec, type FieldValue, REDACTED, type RecordDescriptor } from "./descriptor";

type DecisionItem = Extract<WorkItem, { kind: "decision" }>;

/** The item as a decision. A descriptor only meets items of its kind. */
function asDecision(item: WorkItem): DecisionItem {
  if (item.kind !== "decision") {
    throw new Error(`the decision descriptor met a ${item.kind}`);
  }
  return item;
}

const target = (item: WorkItem) => ({ id: item.id, version: item.version });
const record = (item: WorkItem): Decision => asDecision(item).record;

// The wording, the day and the parties never change after the creation (ADR 0081): a changed
// decision is a new one that supersedes the old. So these three fields have no `save`.
const wording: FieldSpec = {
  id: "title",
  labelKey: "record-field-wording",
  helpKey: "decision-field-wording-help",
  control: "multiline",
  pointer: "wording",
  required: true,
  read: (item) => asDecision(item).title,
};

const status: FieldSpec = {
  id: "status",
  labelKey: "record-field-status",
  icon: IconCircleDashed,
  control: "status",
  pointer: "status",
  read: (item) => asDecision(item).status,
  options: (item) => [asDecision(item).status],
};

const decidedOn: FieldSpec = {
  id: "decidedOn",
  labelKey: "record-field-decided-on",
  icon: IconCalendar,
  control: "date",
  pointer: "decided_on",
  required: true,
  read: (item) => record(item).decided_on,
};

const parties: FieldSpec = {
  id: "parties",
  labelKey: "record-field-parties",
  icon: IconUsers,
  control: "people",
  pointer: "decided_by",
  required: true,
  read: (item) => {
    const ids = record(item).decided_by.map(partyId);
    return ids.length === 0 ? null : ids.join(",");
  },
  people: (item) =>
    record(item).decided_by.map((party) => ({
      id: partyId(party),
      label: party.redacted ? REDACTED : party.name,
    })),
};

// The owner follows up on the decision. Only an event manager changes it (ADR 0081).
const owner = ownerField(
  (item) => asDecision(item).owner.id,
  (commands, item, value) => commands.decision.changeOwner(target(item), value ?? ""),
);

const area = areaField(
  (item) => asDecision(item).workstreamId ?? null,
  (commands, item, value) => commands.decision.changeWorkstream(target(item), value),
);

const successor: FieldSpec = {
  id: "successor",
  labelKey: "record-field-successor",
  icon: IconLink,
  control: "record",
  pointer: "superseded_by",
  refers: "decision",
  hideEmpty: true,
  read: (item) => record(item).superseded_by ?? null,
};

const revokedReason: FieldSpec = {
  id: "revokedReason",
  labelKey: "record-field-revoked-reason",
  icon: IconMessageOff,
  control: "text",
  pointer: "revoked_reason",
  hideEmpty: true,
  read: (item) => record(item).revoked_reason ?? null,
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

export const decisionDescriptor: RecordDescriptor = {
  kind: "decision",
  title: wording,
  properties: [status, decidedOn, parties, owner, area, successor, revokedReason],
  evidenceVersionKey: "decision-evidence-version",
  actions: [
    {
      id: "supersede",
      labelKey: "decision-replace",
      icon: IconReplace,
      available: (item, env) =>
        env.writable && record(item).can_end && record(item).status === "in-force",
      dialog: SupersedeDecisionDialog,
    },
    {
      id: "revoke",
      labelKey: "decision-revoke",
      icon: IconBan,
      available: (item, env) =>
        env.writable && record(item).can_end && record(item).status === "in-force",
      dialog: RevokeDecisionDialog,
    },
  ],
  create: {
    titleKey: "record-create-decision",
    // Only an event manager records a decision (ADR 0081).
    managersOnly: true,
    fields: ["title", "parties", "decidedOn", "owner", "area"],
    defaults: (owner) => ({ title: "", parties: null, decidedOn: null, owner: owner ?? null }),
    validate: (values): Record<string, string> => {
      const message = wordingError(values.title ?? "");
      return message ? { title: message } : {};
    },
    submit: (commands, values, id) =>
      commands.decision.create({
        id,
        wording: optional(values.title ?? null) ?? "",
        decided_on: values.decidedOn ?? "",
        decided_by: partiesOf(values.parties ?? null),
        owner_user_id: values.owner ?? "",
        ...(values.area && { workstream_id: values.area }),
      }),
  },
};
