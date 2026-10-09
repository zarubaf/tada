// The messages for the invalid fields of a work record: one rule for the forms of workstreams,
// actions and commitments.
import type { Problem } from "../api/client";
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

/** The text of an optional field: empty is no value. */
export function blankToNull(text: string): string | null {
  return text.trim() === "" ? null : text.trim();
}
