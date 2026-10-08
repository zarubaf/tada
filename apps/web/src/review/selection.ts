// The selection of proposals for an apply (ADR 0050). Selecting a proposal also selects its
// dependencies, so the member sees what the server will accept. The server does the same check.

import type { Proposal } from "../api/client";

export interface Selection {
  selected: Set<string>;
  /** The dependencies that this selection added besides the proposal the member chose. */
  added: string[];
}

function openById(proposals: Proposal[]): Map<string, Proposal> {
  return new Map(proposals.filter((p) => p.status === "open").map((p) => [p.id, p]));
}

/** Selects `id` and, transitively, every open proposal it depends on. */
export function select(
  proposals: Proposal[],
  selected: ReadonlySet<string>,
  id: string,
): Selection {
  const open = openById(proposals);
  const result = new Set(selected);
  const added: string[] = [];
  const pending = open.has(id) ? [id] : [];
  while (pending.length > 0) {
    const current = pending.pop() as string;
    if (result.has(current) && current !== id) {
      continue;
    }
    result.add(current);
    for (const dependency of open.get(current)?.depends_on ?? []) {
      if (open.has(dependency) && !result.has(dependency)) {
        added.push(dependency);
        pending.push(dependency);
      }
    }
  }
  return { selected: result, added };
}

/** Removes `id` and the selected proposals that depend on it, because they could not apply. */
export function deselect(
  proposals: Proposal[],
  selected: ReadonlySet<string>,
  id: string,
): Set<string> {
  const open = openById(proposals);
  const result = new Set(selected);
  result.delete(id);
  // A proposal stays only while each open dependency of it is selected.
  let changed = true;
  while (changed) {
    changed = false;
    for (const proposalId of result) {
      const missing = (open.get(proposalId)?.depends_on ?? []).some(
        (dependency) => open.has(dependency) && !result.has(dependency),
      );
      if (missing) {
        result.delete(proposalId);
        changed = true;
      }
    }
  }
  return result;
}
