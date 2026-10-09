import { type FormEvent, useEffect, useRef, useState } from "react";
import type { ApplyEdit, Proposal, ValueType } from "../api/client";
import { ValueInput } from "../facts/ValueInput";
import { type DraftField, draftToValue, initialDraft } from "../facts/valueDraft";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { firstInvalidField } from "../ui/focus";
import styles from "./EditForm.module.css";

export interface EditFormProps {
  /** The proposal that sets a fact. The form starts with its value. */
  proposal: Proposal;
  valueType: ValueType;
  /** The apply runs: the submit button ignores a second press and keeps focus. */
  isPending: boolean;
  /** The reviewer accepts this state and value instead of the proposed ones. */
  onSubmit: (state: ApplyEdit["state"]) => void;
  onCancel: () => void;
}

/**
 * „Bearbeiten und annehmen“: the proposed value in the inputs of its value type. A value that
 * fails the check keeps the form open, and focus moves to the first field with an error.
 */
export function EditForm({ proposal, valueType, isPending, onSubmit, onCancel }: EditFormProps) {
  const { operation } = proposal;
  const proposed = operation.kind === "set-fact" ? operation : undefined;
  const [draft, setDraft] = useState(() =>
    initialDraft(valueType, proposed?.value, proposed?.approximate ?? false),
  );
  const [asAssumption, setAsAssumption] = useState(proposed?.state === "assumption");
  const [errors, setErrors] = useState<Partial<Record<DraftField, string>>>({});
  // Counts the failed submits, so that focus moves again for a second failure.
  const [failedSubmits, setFailedSubmits] = useState(0);
  const form = useRef<HTMLFormElement>(null);

  // When the form opens, focus moves to its first field.
  useEffect(() => {
    form.current?.querySelector<HTMLElement>("input, button")?.focus();
  }, []);

  useEffect(() => {
    if (failedSubmits > 0) {
      (
        firstInvalidField(form.current) ?? form.current?.querySelector<HTMLElement>("input")
      )?.focus();
    }
  }, [failedSubmits]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const result = draftToValue(valueType, draft);
    if (!result.ok) {
      setErrors(result.errors);
      setFailedSubmits((count) => count + 1);
      return;
    }
    setErrors({});
    const base = { value: result.value, approximate: draft.approximate };
    onSubmit(asAssumption ? { state: "assumption", ...base } : { state: "accepted", ...base });
  };

  return (
    <form ref={form} noValidate className={styles.form} onSubmit={submit}>
      <h4 className={styles.title}>{t("inbox-edit-title")}</h4>
      <ValueInput type={valueType} draft={draft} errors={errors} onChange={setDraft} />
      <Checkbox
        label={t("inbox-edit-assumption")}
        isSelected={asAssumption}
        onChange={setAsAssumption}
      />
      <div className={styles.actions}>
        <Button onPress={onCancel}>{t("inbox-edit-cancel")}</Button>
        <Button type="submit" variant="primary" isPending={isPending}>
          {t("inbox-edit-submit")}
        </Button>
      </div>
    </form>
  );
}
