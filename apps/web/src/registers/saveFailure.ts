// The failure of a save that no field shows, one rule for the forms of all registers.
import type { Problem } from "../api/client";
import { failureOf } from "../api/failure";
import { t } from "../i18n";

/** What a form tells its page when the save failed for a reason that no field shows. */
export interface SaveFailure {
  message: string;
  /** Someone changed the record: the page loads the rows again and closes the form. */
  conflict: boolean;
}

export function saveFailure(result: {
  error?: Problem | undefined;
  response?: Response | undefined;
}): SaveFailure {
  if (result.error?.code === "record-version-conflict") {
    return { message: t("register-conflict"), conflict: true };
  }
  return { message: failureOf(result).message, conflict: false };
}
