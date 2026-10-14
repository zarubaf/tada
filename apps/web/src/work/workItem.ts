// One shape for the five kinds of work record, as the „Arbeit“ page shows them (ADR 0082): the
// fields that a card, a row and a filter need, and the raw API record for the side panel. A mapper
// per kind reads those fields from the record, so the page never asks which field holds the title.
// Requirements join when they ship (ADR 0081).
import type { Action, Commitment, Decision, Milestone, RecordKind, Risk } from "../api/client";
import { type BoardColumn, columnOf, type StatusOf } from "./recordStatus";

export interface RecordOf {
  action: Action;
  commitment: Commitment;
  milestone: Milestone;
  decision: Decision;
  risk: Risk;
}

export interface Owner {
  id: string;
  /** The display name, empty if the directory does not know the member. */
  name: string;
}

interface ItemOf<K extends RecordKind> {
  kind: K;
  id: string;
  /** The readable ID, for example `ACT-012`. */
  readableId: string;
  /** The text of the kind: title, wording or text. Empty after a legal redaction. */
  title: string;
  /** The real status of the kind; the column groups several statuses. */
  status: StatusOf[K];
  column: BoardColumn;
  owner: Owner;
  /**
   * The date of the kind: the due date of an action or a commitment, the date of a milestone and
   * the day of a decision. A risk has none.
   */
  date: string | undefined;
  workstreamId: string | undefined;
  version: number;
  /** True after a legal redaction (ADR 0072): the texts are empty. */
  redacted: boolean;
  /** The record as the API gave it, for the side panel. */
  record: RecordOf[K];
}

export type WorkItem = { [K in RecordKind]: ItemOf<K> }[RecordKind];
export type WorkItemOf<K extends RecordKind> = ItemOf<K>;

type NameOf = (userId: string) => string;

interface Fields<K extends RecordKind> {
  title: (record: RecordOf[K]) => string;
  date: (record: RecordOf[K]) => string | undefined;
}

const FIELDS: { [K in RecordKind]: Fields<K> } = {
  action: { title: (r) => r.title, date: (r) => r.due_date ?? undefined },
  commitment: { title: (r) => r.text, date: (r) => r.due_date ?? undefined },
  milestone: { title: (r) => r.title, date: (r) => r.date },
  decision: { title: (r) => r.wording, date: (r) => r.decided_on },
  risk: { title: (r) => r.title, date: () => undefined },
};

/** Maps the record of one kind. The names come from the directory of the event. */
export function toWorkItem<K extends RecordKind>(
  kind: K,
  record: RecordOf[K],
  nameOf: NameOf,
): ItemOf<K> {
  const fields = FIELDS[kind] as Fields<K>;
  const status = record.status as StatusOf[K];
  return {
    kind,
    id: record.id,
    readableId: record.local_id,
    title: fields.title(record),
    status,
    column: columnOf(kind, status),
    owner: { id: record.owner_user_id, name: nameOf(record.owner_user_id) },
    date: fields.date(record),
    workstreamId: record.workstream_id ?? undefined,
    version: record.version,
    redacted: record.redacted,
    record,
  };
}
