import { type FormEvent, useState } from "react";
import { type Api, type Commitment, problemMessage } from "../api/client";
import { t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { TaskDialog, TaskDialogActions } from "../ui/TaskDialog";
import { TextField } from "../ui/TextField";
import { fieldErrors } from "./fieldErrors";
import styles from "./Work.module.css";

/**
 * „Verbindlich machen“: a blocking task with a required reason. The commitment keeps the reason
 * (ADR 0068). The condition text stays as it is.
 */
export function MakeFirmDialog({
  api,
  eventId,
  commitment,
  onDone,
  onFailed,
  onCancel,
}: {
  api: Api;
  eventId: string;
  commitment: Commitment;
  onDone: (commitment: Commitment) => void;
  onFailed: (failure: SaveFailure) => void;
  onCancel: () => void;
}) {
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    if (reason.trim() === "") {
      setError(t("work-error-reason"));
      return;
    }
    setError(undefined);
    setBusy(true);
    try {
      const result = await api.POST("/api/v1/events/{event_id}/commitments/{id}/firm", {
        params: { path: { event_id: eventId, id: commitment.id } },
        body: { reason: reason.trim(), expected_version: commitment.version },
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
      title={t("make-firm-title", { id: commitment.local_id })}
      onCancel={() => !busy && onCancel()}
    >
      <p>{t("make-firm-text")}</p>
      {commitment.condition && (
        <p>
          <span className={styles.fixedLabel}>{t("commitment-field-condition-fixed")}: </span>
          {commitment.condition}
        </p>
      )}
      <form className={styles.dialogForm} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("make-firm-reason")}
          help={t("make-firm-reason-help")}
          value={reason}
          onChange={setReason}
          error={error}
          rows={3}
          isRequired
          autoFocus
        />
        <TaskDialogActions>
          <Button onPress={onCancel} isDisabled={busy}>
            {t("work-cancel")}
          </Button>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("make-firm-submit")}
          </Button>
        </TaskDialogActions>
      </form>
    </TaskDialog>
  );
}
