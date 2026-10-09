// The status rules of actions and commitments, mirrored from the domain (ADR 0068). The server
// stays the authority; these rules only hide a change that it would refuse.
import type { ActionStatus, CommitmentStatus } from "../api/client";

const ACTION_NEXT: Record<ActionStatus, ActionStatus[]> = {
  open: ["in-progress", "blocked", "done", "canceled"],
  "in-progress": ["open", "blocked", "done", "canceled"],
  blocked: ["open", "in-progress", "done", "canceled"],
  done: ["open"],
  canceled: [],
};

// A change to `firm` is the command „Verbindlich machen“, never a status choice.
const COMMITMENT_NEXT: Record<CommitmentStatus, CommitmentStatus[]> = {
  conditional: ["fulfilled", "broken", "withdrawn"],
  firm: ["fulfilled", "broken", "withdrawn"],
  fulfilled: [],
  broken: [],
  withdrawn: [],
};

/** The statuses that a form offers for an action: the current one and each allowed next one. */
export function actionStatusChoices(current: ActionStatus): ActionStatus[] {
  return [current, ...ACTION_NEXT[current]];
}

/** The statuses that a form offers for a commitment: the current one and each allowed next one. */
export function commitmentStatusChoices(current: CommitmentStatus): CommitmentStatus[] {
  return [current, ...COMMITMENT_NEXT[current]];
}

/** Only a conditional commitment becomes firm. */
export function canMakeFirm(status: CommitmentStatus): boolean {
  return status === "conditional";
}
