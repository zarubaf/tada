// What the inbox needs to know about the edit of a record proposal without the form itself: the
// operations that can be edited and the messages for the fields that the server refused.

import type { ApplyEdit, Operation, Problem } from "../api/client";
import { hasMessage, t } from "../i18n";

export type Fields = NonNullable<ApplyEdit["fields"]>;
export type FieldName = keyof Fields;
export type FieldErrors = Partial<Record<FieldName, string>>;

export type RecordOperation = Extract<
  Operation,
  { kind: "create-action" | "create-commitment" | "create-person" | "create-institution" }
>;

/** The operations whose fields the reviewer can change. */
export function isRecordOperation(operation: Operation): operation is RecordOperation {
  return (
    operation.kind === "create-action" ||
    operation.kind === "create-commitment" ||
    operation.kind === "create-person" ||
    operation.kind === "create-institution"
  );
}

/** The message for a refused field: the code of the server first, then the field. */
export function message(field: FieldName, code: string): string {
  const key = field === "due_date" ? "due" : field;
  for (const candidate of [`inbox-edit-error-${field}-${code}`, `inbox-edit-error-${field}`]) {
    if (hasMessage(candidate)) {
      return t(candidate);
    }
  }
  return hasMessage(`work-error-${key}`) ? t(`work-error-${key}`) : t("problem-validation-failed");
}

/** The refused fields of an apply that failed with `validation-failed`. */
export function editFieldErrors(problem: Problem | undefined): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem?.errors ?? []) {
    const field = /\/fields\/([a-z_]+)$/.exec(pointer)?.[1] as FieldName | undefined;
    if (field) {
      result[field] = message(field, code);
    }
  }
  return result;
}
