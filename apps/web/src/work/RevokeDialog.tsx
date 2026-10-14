import { type FormEvent, useLayoutEffect, useRef, useState } from "react";
import { type Api, type Decision, problemMessage } from "../api/client";
import { t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { TaskDialog, TaskDialogActions } from "../ui/TaskDialog";
import { TextField } from "../ui/TextField";
import { fieldErrors } from "./fieldErrors";
import styles from "./Work.module.css";

/**
 * „Widerrufen“: a blocking task with a required reason. The decision keeps the reason and ends
 * without a successor (ADR 0081).
 */
export function RevokeDialog({
  api,
  eventId,
  decision,
  onDone,
  onFailed,
  onCancel,
}: {
  api: Api;
  eventId: string;
  decision: Decision;
  onDone: (decision: Decision) => void;
  onFailed: (failure: SaveFailure) => void;
  onCancel: () => void;
}) {
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const caret = useRef<{ field: HTMLInputElement; at: number }>(undefined);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs after each new value of the reason
  useLayoutEffect(() => {
    caret.current?.field.setSelectionRange(caret.current.at, caret.current.at);
    caret.current = undefined;
  }, [reason]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    if (reason.trim() === "") {
      setError(t("work-error-reason"));
      return;
    }
    if (/\p{Cc}/u.test(reason.trim())) {
      setError(t("work-error-reason-control-character"));
      return;
    }
    setError(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/events/{event_id}/decisions/{id}/revoke", {
        params: { path: { event_id: eventId, id: decision.id } },
        body: { reason: reason.trim(), expected_version: decision.version },
      });
      if (result.data) {
        onDone(result.data);
        return;
      }
      const invalid =
        result.error?.code === "validation-failed"
          ? fieldErrors(result.error, ["reason"] as const)
          : {};
      if (invalid.reason) {
        setError(invalid.reason);
      } else {
        onFailed(saveFailure(result));
      }
    } catch {
      onFailed({ message: problemMessage(undefined), conflict: false });
    }
    setBusy(false);
  };

  return (
    <TaskDialog
      title={t("decision-revoke-title", { id: decision.local_id })}
      onCancel={() => !busy && onCancel()}
    >
      <p>{t("decision-revoke-text")}</p>
      <form className={styles.dialogForm} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("decision-revoke-reason")}
          help={t("decision-revoke-reason-help")}
          value={reason}
          onChange={(value) => {
            // A keystroke after a paste that changed nothing: the caret is where the member put it.
            caret.current = undefined;
            setReason(value);
          }}
          onPaste={(event) => {
            // A reason is one line, and the browser would drop the line breaks of a paste
            // without a space: they become spaces.
            const pasted = event.clipboardData.getData("text");
            if (/[\r\n]/.test(pasted)) {
              event.preventDefault();
              const field = event.currentTarget as HTMLInputElement;
              const start = field.selectionStart ?? reason.length;
              const end = field.selectionEnd ?? start;
              const line = pasted.replace(/\s*[\r\n]+\s*/g, " ");
              setReason(reason.slice(0, start) + line + reason.slice(end));
              // The new value would put the caret at the end: it goes behind the pasted text.
              caret.current = { field, at: start + line.length };
            }
          }}
          error={error}
          isRequired
          autoFocus
        />
        <TaskDialogActions>
          <Button onPress={onCancel} isDisabled={busy}>
            {t("work-cancel")}
          </Button>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("decision-revoke")}
          </Button>
        </TaskDialogActions>
      </form>
    </TaskDialog>
  );
}
