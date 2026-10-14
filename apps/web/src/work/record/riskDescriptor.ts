import { IconBolt, IconCircleDashed, IconDice3, IconStack2, IconUser } from "@tabler/icons-react";
import type { CreateRiskRequest, Level, Risk, RiskStatus } from "../../api/client";
import type { WorkItem } from "../workItem";
import type { FieldSpec, FieldValue, RecordDescriptor } from "./descriptor";
import { LEVEL_ICONS } from "./LevelIcon";

type RiskItem = Extract<WorkItem, { kind: "risk" }>;

/** The item as a risk. A descriptor only meets items of its kind. */
function asRisk(item: WorkItem): RiskItem {
  if (item.kind !== "risk") {
    throw new Error(`the risk descriptor met a ${item.kind}`);
  }
  return item;
}

const target = (item: WorkItem) => ({ id: item.id, version: item.version });
const record = (item: WorkItem): Risk => asRisk(item).record;

/** The three levels of impact and likelihood, from low to high. Each has a word and a bars icon. */
const LEVELS: readonly Level[] = ["low", "medium", "high"];
const level = (value: string) => ({
  labelKey: `risk-level-${value}`,
  icon: LEVEL_ICONS[value as Level],
});

const title: FieldSpec = {
  id: "title",
  labelKey: "record-field-title",
  control: "text",
  pointer: "title",
  required: true,
  read: (item) => asRisk(item).title,
  save: (commands, item, value) => commands.risk.rename(target(item), value ?? ""),
};

const status: FieldSpec = {
  id: "status",
  labelKey: "record-field-status",
  icon: IconCircleDashed,
  control: "status",
  pointer: "status",
  required: true,
  read: (item) => asRisk(item).status,
  options: (item) => [asRisk(item).status, ...record(item).next_statuses],
  save: (commands, item, value) => commands.risk.changeStatus(target(item), value as RiskStatus),
};

const impact: FieldSpec = {
  id: "impact",
  labelKey: "record-field-impact",
  icon: IconBolt,
  control: "enum",
  pointer: "impact",
  promptKey: "record-choose-level",
  required: true,
  read: (item) => record(item).impact,
  values: LEVELS,
  present: level,
  save: (commands, item, value) => commands.risk.changeImpact(target(item), value as Level),
};

const likelihood: FieldSpec = {
  id: "likelihood",
  labelKey: "record-field-likelihood",
  icon: IconDice3,
  control: "enum",
  pointer: "likelihood",
  promptKey: "record-choose-level",
  required: true,
  read: (item) => record(item).likelihood,
  values: LEVELS,
  present: level,
  save: (commands, item, value) => commands.risk.changeLikelihood(target(item), value as Level),
};

const owner: FieldSpec = {
  id: "owner",
  labelKey: "record-field-owner",
  icon: IconUser,
  control: "person",
  pointer: "owner",
  promptKey: "record-choose-owner",
  required: true,
  read: (item) => asRisk(item).owner.id,
  save: (commands, item, value) => commands.risk.changeOwner(target(item), value ?? ""),
};

const area: FieldSpec = {
  id: "area",
  labelKey: "record-field-area",
  icon: IconStack2,
  control: "area",
  pointer: "workstream",
  emptyKey: "record-no-area",
  read: (item) => asRisk(item).workstreamId ?? null,
  save: (commands, item, value) => commands.risk.changeWorkstream(target(item), value),
};

const description: FieldSpec = {
  id: "description",
  labelKey: "record-field-description",
  control: "multiline",
  pointer: "description",
  emptyKey: "record-no-description",
  read: (item) => record(item).description ?? null,
  save: (commands, item, value) => commands.risk.changeDescription(target(item), value),
};

const mitigation: FieldSpec = {
  id: "mitigation",
  labelKey: "record-field-mitigation",
  control: "multiline",
  pointer: "mitigation",
  emptyKey: "record-no-mitigation",
  read: (item) => record(item).mitigation ?? null,
  save: (commands, item, value) => commands.risk.changeMitigation(target(item), value),
};

/** The text of a form field as the API takes it: no text is no value. */
const optional = (value: FieldValue): string | undefined =>
  value === null || value.trim() === "" ? undefined : value.trim();

function body(values: Record<string, FieldValue>, id: string): CreateRiskRequest {
  const description = optional(values.description ?? null);
  const mitigation = optional(values.mitigation ?? null);
  return {
    id,
    title: optional(values.title ?? null) ?? "",
    impact: (values.impact ?? "") as Level,
    likelihood: (values.likelihood ?? "") as Level,
    owner_user_id: values.owner ?? "",
    ...(description && { description }),
    ...(mitigation && { mitigation }),
    ...(values.area && { workstream_id: values.area }),
  };
}

export const riskDescriptor: RecordDescriptor = {
  kind: "risk",
  title,
  properties: [status, impact, likelihood, owner, area],
  description,
  texts: [mitigation],
  evidenceVersionKey: "risk-evidence-version",
  create: {
    titleKey: "record-create-risk",
    fields: ["title", "impact", "likelihood", "owner", "area", "description", "mitigation"],
    defaults: (owner) => ({ title: "", impact: null, likelihood: null, owner: owner ?? null }),
    submit: (commands, values, id) => commands.risk.create(body(values, id)),
  },
};
