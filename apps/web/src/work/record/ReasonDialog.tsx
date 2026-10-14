import { type FormEvent, type ReactNode, useRef, useState } from "react";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { LiveRegion } from "../../ui/LiveRegion";
import { TaskDialog, TaskDialogActions } from "../../ui/TaskDialog";
import { TextField } from "../../ui/TextField";
import { fieldErrors } from "../fieldErrors";
import type { CommandResult } from "../workCommands";
import styles from "./RecordActionDialog.module.css";

export interface ReasonDialogProps {
  title: string;
  /** What the command does, one sentence for each line. */
  description?: readonly string[];
  /** A field of its own, between the description and the reason, for example a successor. */
  extra?: ReactNode;
  /** The label of the reason, with the mark of a required field. */
  reasonLabel: string;
  reasonHelp?: string;
  confirmLabel: string;
  /** The command ends something that does not come back: the confirm button is the danger button. */
  danger?: boolean;
  /** Runs the command with the reason, trimmed. */
  onConfirm: (reason: string) => Promise<CommandResult<unknown>>;
  /** The command worked. The caller closes the dialog. */
  onDone: () => void;
  onCancel: () => void;
}

/**
 * The step before a record action that needs a reason (ADR 0068): a blocking dialog with the
 * reason as its required field. It checks the reason, puts the refusal of the server at the field,
 * and keeps what the member typed through every failure, also a conflict. It closes only by the
 * caller, after `onDone`. A kind brings the texts and the command, so „Verbindlich machen“, and
 * later „Ersetzen“ and „Widerrufen“, share one dialog.
 */
export function ReasonDialog({
  title,
  description = [],
  extra,
  reasonLabel,
  reasonHelp,
  confirmLabel,
  danger = false,
  onConfirm,
  onDone,
  onCancel,
}: ReasonDialogProps) {
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string>();
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  // A second press in the same moment sees the flag before React has rendered `busy`.
  const sending = useRef(false);
  const field = useRef<HTMLDivElement>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (sending.current) {
      return;
    }
    setFailure(undefined);
    if (reason.trim() === "") {
      setError(t("work-error-reason"));
      field.current?.querySelector("textarea")?.focus();
      return;
    }
    setError(undefined);
    sending.current = true;
    setBusy(true);
    try {
      const result = await onConfirm(reason.trim());
      if (result.ok) {
        onDone();
        return;
      }
      const { reason: why, problem, message } = result.failure;
      const invalid = problem ? fieldErrors(problem, ["reason"] as const).reason : undefined;
      if (invalid) {
        setError(invalid);
      } else {
        setFailure(why === "conflict" ? t("record-action-conflict") : message);
      }
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  return (
    <TaskDialog title={title} onCancel={() => !busy && onCancel()}>
      {description.map((line) => (
        <p key={line}>{line}</p>
      ))}
      <form className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
        {extra}
        <div ref={field}>
          <TextField
            label={reasonLabel}
            {...(reasonHelp && { help: reasonHelp })}
            value={reason}
            onChange={setReason}
            error={error}
            rows={3}
            isRequired
            autoFocus
          />
        </div>
        <LiveRegion kind="alert">{failure}</LiveRegion>
        <TaskDialogActions>
          <Button onPress={onCancel} isDisabled={busy}>
            {t("record-cancel")}
          </Button>
          <Button type="submit" variant={danger ? "danger" : "primary"} isPending={busy}>
            {confirmLabel}
          </Button>
        </TaskDialogActions>
      </form>
    </TaskDialog>
  );
}
