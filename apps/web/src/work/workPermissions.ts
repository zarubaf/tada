// Who may change a work record. The server decides per record and says so in `can_change`: the
// owner, the lead of the workstream or an event manager (ADR 0052, 0067, 0081), and for a decision
// the event manager. The client adds only what the server cannot know before it asks: a closed
// event is read-only for everybody (ADR 0071), and a redacted record has no text left to change.
import type { RecordKind } from "../api/client";
import type { Directory } from "./directory";
import { CREATABLE } from "./record/recordRoute";
import type { WorkItem } from "./workItem";

/**
 * True if the member may offer the change controls of the item. `eventWritable` is false for a
 * closed event (`useEventWritable`). The server checks the right again.
 */
export function canEdit(item: WorkItem, eventWritable: boolean): boolean {
  return eventWritable && item.record.can_change && !item.redacted;
}

/**
 * True if the member may create a record: an event manager or an owner or admin of the organization
 * (manager level, also without a membership), or a member who could own a record, which is a
 * contributor (ADR 0052). The same rule as `require_create` of the server, which checks it again.
 * A closed event is read-only.
 */
export function canCreate(
  directory: Pick<Directory, "assignees" | "isManager"> | undefined,
  me: string,
  eventWritable: boolean,
): boolean {
  return (
    eventWritable &&
    directory !== undefined &&
    (directory.isManager || directory.assignees.some((member) => member.id === me))
  );
}

/**
 * The kinds that only an event manager records: a decision (ADR 0081). It lives here, and not in
 * the descriptors, because the page needs it for the menu and the descriptors load with the panel.
 */
const MANAGERS_ONLY: ReadonlySet<RecordKind> = new Set(["decision"]);

type Who = Pick<Directory, "assignees" | "isManager">;

/** True if the member may create a record of the kind: `canCreate`, and a manager for a decision. */
export function mayCreateKind(
  kind: RecordKind,
  directory: Who | undefined,
  me: string,
  eventWritable: boolean,
): boolean {
  return (
    canCreate(directory, me, eventWritable) &&
    (!MANAGERS_ONLY.has(kind) || directory?.isManager === true)
  );
}

/** True if the kind is one that only managers create, for the words of the create state. */
export const isManagersOnly = (kind: RecordKind): boolean => MANAGERS_ONLY.has(kind);

/** The kinds that the member may create, in the order of the „Neu“ menu. */
export function creatableKinds(
  directory: Who | undefined,
  me: string,
  eventWritable: boolean,
): RecordKind[] {
  return CREATABLE.filter((kind) => mayCreateKind(kind, directory, me, eventWritable));
}
