// Who may change a work record. The server decides per record and says so in `can_change`: the
// owner, the lead of the workstream or an event manager (ADR 0052, 0067, 0081), and for a decision
// the event manager. The client adds only what the server cannot know before it asks: a closed
// event is read-only for everybody (ADR 0071), and a redacted record has no text left to change.
import type { Directory } from "./directory";
import type { WorkItem } from "./workItem";

/**
 * True if the member may offer the change controls of the item. `eventWritable` is false for a
 * closed event (`useEventWritable`). The server checks the right again.
 */
export function canEdit(item: WorkItem, eventWritable: boolean): boolean {
  return eventWritable && item.record.can_change && !item.redacted;
}

/**
 * True if the member may create a record. A contributor or manager of the event can (ADR 0052);
 * the directory lists exactly those as owners, so the member can create if they could own a record.
 * A closed event is read-only. The server checks the right again.
 */
export function canCreate(
  directory: Pick<Directory, "assignees"> | undefined,
  me: string,
  eventWritable: boolean,
): boolean {
  return eventWritable && directory?.assignees.some((member) => member.id === me) === true;
}
