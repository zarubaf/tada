// The words for the warnings that the server calculates for a milestone (ADR 0081). The register
// and the side panel say them the same way.
import type { MilestoneWarning } from "../api/client";
import { t } from "../i18n";

/**
 * The warning as a sentence. `readableId` gives the readable ID of an action by its UUID; an action
 * that the list lacks (it did not load) is named without its ID.
 */
export function milestoneWarningText(
  warning: MilestoneWarning,
  readableId: (actionId: string) => string | undefined,
): string {
  if (warning.kind === "overdue") {
    return t("milestone-warning-overdue");
  }
  const id = readableId(warning.action_id);
  return id ? t("milestone-warning-late-action", { id }) : t("milestone-warning-late-unknown");
}
