// The dialog of an action on a record that needs more than a click: it asks for a reason, for
// more fields, or for both, and says what the action does before the member confirms it (ADR
// 0082). „Ersetzen“ and „Widerrufen“ of a decision use it. It knows no kind: the caller gives the
// words, the extra fields and the command, and decides when the dialog closes (it is open while it
// is mounted).
import { type FormEvent, type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../../ui/focus";
import { LiveRegion } from "../../ui/LiveRegion";
import { TaskDialog, TaskDialogActions } from "../../ui/TaskDialog";
import { TextField } from "../../ui/TextField";
import styles from "./RecordActionDialog.module.css";

/**
 * Why the command did not run. `reason` is the message for the reason field, `message` any other
 * failure, which the dialog says as an alert. An empty refusal means that the extra fields show
 * their own errors; focus goes to the first of them.
 */
export interface Refusal {
  reason?: string;
  message?: string;
}

export interface RecordActionDialogProps {
  /** The heading and the name of the dialog, for example „DEC-001 widerrufen“. */
  title: string;
  /** What the action does, in one or two sentences. */
  description?: string;
  /** Asks for a reason, a required line of text. Without it the dialog asks for none. */
  reason?: { label: string; help?: string };
  /** Extra fields before the reason. They keep their own values; the caller reads them in `onConfirm`. */
  children?: ReactNode;
  confirmLabel: string;
  /** The action ends something for good: the confirm button is the danger button. */
  danger?: boolean;
  /**
   * Runs the command with the trimmed reason (empty without a reason field). It resolves to
   * nothing on success, and the caller then unmounts the dialog; it resolves to a refusal, and the
   * dialog stays open and keeps what the member typed.
   */
  onConfirm: (reason: string) => Promise<Refusal | undefined>;
  /** Cancel and Escape. The caller returns focus to the control that opened the dialog. */
  onCancel: () => void;
}

export function RecordActionDialog({
  title,
  description,
  reason: reasonField,
  children,
  confirmLabel,
  danger = false,
  onConfirm,
  onCancel,
}: RecordActionDialogProps) {
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string>();
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  // A paste that changes the reason moves the caret to the end; it is set back behind the paste.
  const caret = useRef<{ field: HTMLInputElement; at: number }>(undefined);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs after each new value of the reason
  useLayoutEffect(() => {
    caret.current?.field.setSelectionRange(caret.current.at, caret.current.at);
    caret.current = undefined;
  }, [reason]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (sending.current) {
      return;
    }
    const text = reason.trim();
    if (reasonField && text === "") {
      setError(t("work-error-reason"));
      focusAfterCommit(() => firstInvalidField(form.current));
      return;
    }
    if (/\p{Cc}/u.test(text)) {
      setError(t("work-error-reason-control-character"));
      focusAfterCommit(() => firstInvalidField(form.current));
      return;
    }
    setError(undefined);
    setFailure(undefined);
    sending.current = true;
    setBusy(true);
    try {
      const refusal = await onConfirm(text);
      if (refusal) {
        setError(refusal.reason);
        setFailure(refusal.message);
        focusAfterCommit(() => firstInvalidField(form.current));
      }
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };

  return (
    <TaskDialog title={title} onCancel={() => !busy && onCancel()}>
      {description && <p>{description}</p>}
      <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
        {(children || reasonField) && (
          <div className={styles.fields}>
            {children}
            {reasonField && (
              <TextField
                label={reasonField.label}
                {...(reasonField.help && { help: reasonField.help })}
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
                    caret.current = { field, at: start + line.length };
                  }
                }}
                error={error}
                isRequired
                autoComplete="off"
                autoFocus={!children}
              />
            )}
          </div>
        )}
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
