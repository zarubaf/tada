// When a proposal cannot be accepted any more (ADR 0050). The server decides at apply time. The
// inbox shows the conflict before, so that „Annehmen“ is off and the reason is visible.

import type { Proposal } from "../api/client";

/** Why a proposal conflicts. `dependency` is the conflict of a proposal it depends on. */
export type Conflict = "fact-changed" | "target-changed" | "dependency";

function ownConflict(proposal: Proposal): Conflict | undefined {
  if (proposal.status === "conflict") {
    return proposal.conflict_reason ?? "target-changed";
  }
  if (proposal.status !== "open") {
    return undefined;
  }
  const { operation, current } = proposal;
  if (operation.kind === "set-fact") {
    const expected = operation.expected_version ?? null;
    if (expected !== (current?.version ?? null)) {
      return "fact-changed";
    }
  }
  return undefined;
}

/**
 * The conflict of an open proposal, or of a proposal with the status `conflict`. A proposal that
 * depends on a conflicting proposal conflicts too, because the apply is all or nothing.
 */
export function conflictOf(
  proposal: Proposal,
  proposals: Proposal[],
  seen: ReadonlySet<string> = new Set(),
): Conflict | undefined {
  const own = ownConflict(proposal);
  if (own || proposal.status !== "open") {
    return own;
  }
  const visited = new Set(seen).add(proposal.id);
  for (const id of proposal.depends_on) {
    const dependency = proposals.find((candidate) => candidate.id === id);
    if (dependency && !visited.has(id) && conflictOf(dependency, proposals, visited)) {
      return "dependency";
    }
  }
  return undefined;
}
