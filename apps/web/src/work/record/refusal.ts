// What a dialog of a record action shows for a command that did not work (ADR 0082). The dialogs
// of every kind map their results here, so a conflict reads the same for „Ersetzen“, „Widerrufen“
// and „Verbindlich machen“.
import type { Problem } from "../../api/client";
import { t } from "../../i18n";
import { fieldErrors } from "../fieldErrors";
import type { CommandResult } from "../workCommands";
import type { ActionDialogProps } from "./descriptor";

/**
 * Why the command did not run. `reason` is the message for the reason field, `message` any other
 * failure, which the dialog says as an alert. An empty refusal means that the extra fields show
 * their own errors; focus goes to the first of them.
 */
export interface Refusal {
  reason?: string;
  message?: string;
}

/** The refusal of the reason field, for the problem of a command that asks for a reason only. */
const reasonOf = (problem: Problem): Refusal | undefined => {
  const reason = fieldErrors(problem, ["reason"] as const).reason;
  return reason ? { reason } : undefined;
};

/**
 * The refusal for the result of a command, or nothing if it worked. After a conflict the record is
 * loaded again and the dialog says so, or says that the reload failed; the panel then closes the dialog if the action is gone
 * (`onConflict`). A `validation-failed` problem goes to `fields`, which maps it to the fields of
 * the dialog; by default it is the reason.
 */
export async function refusalOf(
  result: CommandResult<unknown>,
  { item, env, onConflict }: Pick<ActionDialogProps, "item" | "env" | "onConflict">,
  fields: (problem: Problem) => Refusal | undefined = reasonOf,
): Promise<Refusal | undefined> {
  if (result.ok) {
    return undefined;
  }
  const { failure } = result;
  if (failure.reason === "conflict") {
    onConflict();
    const loaded = await env.refresh(item.kind);
    return { message: t(loaded ? "record-action-conflict" : "record-action-conflict-stale") };
  }
  const invalid =
    failure.problem?.code === "validation-failed" ? fields(failure.problem) : undefined;
  return invalid ?? { message: failure.message };
}
