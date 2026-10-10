import { type FormEvent, useState } from "react";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { TaskDialog, TaskDialogActions } from "../ui/TaskDialog";
import { TextField } from "../ui/TextField";
import styles from "./PersonDataProtection.module.css";

const MAX_LENGTH = 1000;

/**
 * „Bestreiten“: a note of 1 to 1000 characters on the evidence (ADR 0073). The caller sends it and
 * returns a message if the server refuses; the dialog shows that message under the field.
 */
export function DisputeDialog({
  onSubmit,
  onCancel,
}: {
  onSubmit: (text: string) => Promise<string | undefined>;
  onCancel: () => void;
}) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    const note = text.trim();
    if (note === "" || [...note].length > MAX_LENGTH) {
      setError(t("person-data-dispute-error"));
      return;
    }
    setError(undefined);
    setBusy(true);
    setError(await onSubmit(note));
    setBusy(false);
  };

  return (
    <TaskDialog title={t("person-data-dispute-title")} onCancel={() => !busy && onCancel()}>
      <form className={styles.dialogForm} onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("person-data-dispute-label")}
          help={t("person-data-dispute-help")}
          value={text}
          onChange={setText}
          error={error}
          rows={4}
          isRequired
          autoFocus
        />
        <TaskDialogActions>
          <Button onPress={onCancel} isDisabled={busy}>
            {t("work-cancel")}
          </Button>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("person-data-dispute-submit")}
          </Button>
        </TaskDialogActions>
      </form>
    </TaskDialog>
  );
}
