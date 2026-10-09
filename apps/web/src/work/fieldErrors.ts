// The messages of a failed save of a work record: one rule for the forms of workstreams, actions
// and commitments.
import type { Problem } from "../api/client";
import { failureOf } from "../api/failure";
import { hasMessage, t } from "../i18n";

/** The message for each invalid field of a `validation-failed` problem that the form knows. */
export function fieldErrors<F extends string>(
  problem: Problem,
  fields: readonly F[],
): Partial<Record<F, string>> {
  const result: Partial<Record<F, string>> = {};
  for (const { pointer, code } of problem.errors ?? []) {
    const field = fields.find((candidate) => candidate === pointer.slice(1));
    if (field) {
      const specific = `work-error-${field}-${code}`;
      result[field] = t(hasMessage(specific) ? specific : `work-error-${field}`);
    }
  }
  return result;
}

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
    return { message: t("work-conflict"), conflict: true };
  }
  return { message: failureOf(result).message, conflict: false };
}

/** The text of an optional field: empty is no value. */
export function blankToNull(text: string): string | null {
  return text.trim() === "" ? null : text.trim();
}
