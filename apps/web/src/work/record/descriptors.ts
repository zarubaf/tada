// The kinds that the side panel knows. A kind joins by one row; set its row in `PANEL` of
// `workLinks.ts` as well, so that its cards open the panel.
import type { RecordKind } from "../../api/client";
import { actionDescriptor } from "./actionDescriptor";
import type { RecordDescriptor } from "./descriptor";
import { riskDescriptor } from "./riskDescriptor";

const DESCRIPTORS: { [K in RecordKind]?: RecordDescriptor } = {
  action: actionDescriptor,
  risk: riskDescriptor,
};

/** The descriptor of a kind, or nothing while the panel does not know the kind. */
export function descriptorOf(kind: RecordKind): RecordDescriptor | undefined {
  return DESCRIPTORS[kind];
}
