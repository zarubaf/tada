// What the bulk bar can do with a set of work records: which change each kind allows and how one
// change reaches the wrapped commands. The commands decide what a kind supports; a command that the
// API lacks is not offered, so this file holds no table of kinds.
import type { RecordKind } from "../api/client";
import type { CommandResult, Target, WorkCommands } from "./workCommands";
import type { WorkItem } from "./workItem";

/** One change for many records. */
export type BulkChange =
  | { field: "status"; status: string }
  | { field: "owner"; userId: string }
  /** `null` takes the records out of their workstream. */
  | { field: "area"; workstreamId: string | null };

type Field = BulkChange["field"];

const COMMAND: Record<Field, string> = {
  status: "changeStatus",
  owner: "changeOwner",
  area: "changeWorkstream",
};

type Command = (target: Target, value: unknown) => Promise<CommandResult<unknown>>;

function commandOf(commands: WorkCommands, kind: RecordKind, field: Field): Command | undefined {
  // The one place that looks a command up by name; the typed commands stay typed everywhere else.
  return (commands[kind] as unknown as Record<string, Command | undefined>)[COMMAND[field]];
}

/** True if every record has a command for the field. */
export function supports(commands: WorkCommands, items: readonly WorkItem[], field: Field) {
  return items.every((item) => commandOf(commands, item.kind, field) !== undefined);
}

export type StatusChoice =
  | { allowed: true; kind: RecordKind; statuses: string[] }
  | { allowed: false; reason: "mixed" | "no-command" | "none-common" };

/** The statuses that the server allows next, as the record says (`next_statuses`). */
function nextStatuses(item: WorkItem): readonly string[] {
  return "next_statuses" in item.record ? item.record.next_statuses : [];
}

/**
 * The status the bar may offer: only for records of one kind, and only those that every record may
 * take next (ADR 0082). The order is the order of the first record.
 */
export function statusChoice(commands: WorkCommands, items: readonly WorkItem[]): StatusChoice {
  const [first, ...rest] = items;
  if (!first) {
    return { allowed: false, reason: "none-common" };
  }
  if (rest.some((item) => item.kind !== first.kind)) {
    return { allowed: false, reason: "mixed" };
  }
  if (!supports(commands, items, "status")) {
    return { allowed: false, reason: "no-command" };
  }
  const statuses = nextStatuses(first).filter((status) =>
    rest.every((item) => nextStatuses(item).includes(status)),
  );
  return statuses.length === 0
    ? { allowed: false, reason: "none-common" }
    : { allowed: true, kind: first.kind, statuses };
}

/** True if the record has the value already, so that the change would be none. */
export function unchanged(item: WorkItem, change: BulkChange): boolean {
  switch (change.field) {
    case "owner":
      return item.owner.id === change.userId;
    case "area":
      return item.workstreamId === (change.workstreamId ?? undefined);
    case "status":
      return false;
  }
}

/** Sends the change for one record through its wrapped command. */
export function applyChange(
  commands: WorkCommands,
  item: WorkItem,
  change: BulkChange,
): Promise<CommandResult<unknown>> {
  const command = commandOf(commands, item.kind, change.field);
  const target = { id: item.id, version: item.version };
  if (!command) {
    throw new Error(`${item.kind} has no ${change.field} command`);
  }
  switch (change.field) {
    case "status":
      return command(target, change.status);
    case "owner":
      return command(target, change.userId);
    case "area":
      return command(target, change.workstreamId);
  }
}
