// The one table of the statuses of the five record kinds (ADR 0082): each status has an icon, a
// tone, a German label and a board column. The record statuses come from the generated API types,
// so a status that the API adds fails the build here until it has a row.
import type {
  ActionStatus,
  CommitmentStatus,
  DecisionStatus,
  MilestoneStatus,
  RecordKind,
  RiskStatus,
} from "../api/client";
import type { StatusGlyph, StatusTone } from "../ui/StatusIcon";

export const RECORD_KINDS = [
  "action",
  "commitment",
  "milestone",
  "decision",
  "risk",
] as const satisfies readonly RecordKind[];

export interface StatusOf {
  action: ActionStatus;
  commitment: CommitmentStatus;
  milestone: MilestoneStatus;
  decision: DecisionStatus;
  risk: RiskStatus;
}

/** The columns of the board, in order. The ID is the status group; the board names it. */
export const BOARD_COLUMNS = ["open", "in-progress", "blocked", "done", "canceled"] as const;
export type BoardColumn = (typeof BOARD_COLUMNS)[number];

interface Row {
  glyph: StatusGlyph;
  tone: StatusTone;
  column: BoardColumn;
}

export const STATUSES: { [K in RecordKind]: Record<StatusOf[K], Row> } = {
  action: {
    open: { glyph: "open", tone: "neutral", column: "open" },
    "in-progress": { glyph: "progress", tone: "progress", column: "in-progress" },
    blocked: { glyph: "blocked", tone: "danger", column: "blocked" },
    done: { glyph: "done", tone: "success", column: "done" },
    canceled: { glyph: "canceled", tone: "muted", column: "canceled" },
  },
  // A conditional commitment is not yet reliable, so it has the warning tone.
  commitment: {
    conditional: { glyph: "conditional", tone: "warning", column: "open" },
    firm: { glyph: "firm", tone: "progress", column: "open" },
    fulfilled: { glyph: "done", tone: "success", column: "done" },
    broken: { glyph: "canceled", tone: "danger", column: "canceled" },
    withdrawn: { glyph: "canceled", tone: "muted", column: "canceled" },
  },
  milestone: {
    open: { glyph: "open", tone: "neutral", column: "open" },
    reached: { glyph: "done", tone: "success", column: "done" },
    canceled: { glyph: "canceled", tone: "muted", column: "canceled" },
  },
  // A decision in force binds; a superseded or revoked one is history, so it is muted.
  decision: {
    "in-force": { glyph: "done", tone: "success", column: "open" },
    superseded: { glyph: "superseded", tone: "muted", column: "canceled" },
    revoked: { glyph: "canceled", tone: "muted", column: "canceled" },
  },
  risk: {
    open: { glyph: "open", tone: "neutral", column: "open" },
    mitigated: { glyph: "mitigated", tone: "success", column: "done" },
    occurred: { glyph: "occurred", tone: "danger", column: "blocked" },
    closed: { glyph: "done", tone: "muted", column: "done" },
  },
};

export interface StatusPresentation {
  glyph: StatusGlyph;
  tone: StatusTone;
  /** The Fluent key of the German label, for example `risk-status-occurred`. */
  labelKey: string;
}

function row<K extends RecordKind>(kind: K, status: StatusOf[K]): Row {
  return (STATUSES[kind] as Record<StatusOf[K], Row>)[status];
}

/** The icon, the tone and the label of a status. */
export function statusPresentation<K extends RecordKind>(
  kind: K,
  status: StatusOf[K],
): StatusPresentation {
  const { glyph, tone } = row(kind, status);
  return { glyph, tone, labelKey: `${kind}-status-${status}` };
}

/**
 * The presentation of a status that is only known as text, for example one of `next_statuses` of a
 * record whose kind is a union. An unknown status is a bug in the caller and fails here.
 */
export function statusPresentationOfText(kind: RecordKind, status: string): StatusPresentation {
  const found = (STATUSES[kind] as Record<string, Row | undefined>)[status];
  if (!found) {
    throw new Error(`${kind} has no status ${status}`);
  }
  return { glyph: found.glyph, tone: found.tone, labelKey: `${kind}-status-${status}` };
}

/** The presentation of a record that holds its kind and its status together, for example an item. */
export function presentationOf<K extends RecordKind>(record: {
  kind: K;
  status: StatusOf[K];
}): StatusPresentation {
  return statusPresentation(record.kind, record.status);
}

/** The board column of a record: a card is in the column of its status group (ADR 0082). */
export function columnOf<K extends RecordKind>(kind: K, status: StatusOf[K]): BoardColumn {
  return row(kind, status).column;
}
