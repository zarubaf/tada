import type { Milestone } from "../api/client";

/** The readable ID and the title of a milestone; the ID alone after a legal redaction. */
export function milestoneName(milestone: Pick<Milestone, "local_id" | "title" | "redacted">) {
  return milestone.redacted ? milestone.local_id : `${milestone.local_id} ${milestone.title}`;
}
