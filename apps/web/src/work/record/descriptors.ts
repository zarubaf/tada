// The side panel knows every kind of record: a new kind needs one row here.
import type { RecordKind } from "../../api/client";
import { actionDescriptor } from "./actionDescriptor";
import { commitmentDescriptor } from "./commitmentDescriptor";
import { decisionDescriptor } from "./decisionDescriptor";
import type { RecordDescriptor } from "./descriptor";
import { milestoneDescriptor } from "./milestoneDescriptor";
import { riskDescriptor } from "./riskDescriptor";

const DESCRIPTORS: { [K in RecordKind]: RecordDescriptor } = {
  action: actionDescriptor,
  risk: riskDescriptor,
  milestone: milestoneDescriptor,
  decision: decisionDescriptor,
  commitment: commitmentDescriptor,
};

/** The descriptor of a kind. */
export function descriptorOf(kind: RecordKind): RecordDescriptor {
  return DESCRIPTORS[kind];
}
