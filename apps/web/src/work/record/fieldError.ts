// The message of the server for one field of the panel. A field lists the other names that the
// server uses for it in `elsewhere`; this is the one place that reads them.
import type { Problem } from "../../api/client";
import { hasMessage, t } from "../../i18n";
import { fieldErrors } from "../fieldErrors";
import type { FieldSpec } from "./descriptor";

/** The message for the invalid field of a `validation-failed` problem that names `spec`, if any. */
export function fieldError(problem: Problem | undefined, spec: FieldSpec): string | undefined {
  if (problem?.code !== "validation-failed") {
    return undefined;
  }
  const own = fieldErrors(problem, [spec.pointer])[spec.pointer];
  if (own) {
    return own;
  }
  const other = (problem.errors ?? []).find(({ pointer }) =>
    spec.elsewhere?.includes(pointer.slice(1)),
  );
  if (!other) {
    return undefined;
  }
  const specific = `work-error-${spec.pointer}-${other.code}`;
  return t(hasMessage(specific) ? specific : `work-error-${spec.pointer}`);
}
