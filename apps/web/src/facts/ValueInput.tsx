import { useId } from "react";
import type { ValueType } from "../api/client";
import { t } from "../i18n";
import { Checkbox } from "../ui/Checkbox";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import { formatLabel } from "./formatValue";
import styles from "./ValueInput.module.css";
import type { Draft, DraftField, Granularity } from "./valueDraft";

export interface ValueInputProps {
  /** The value type of the field decides which inputs show. */
  type: ValueType;
  draft: Draft;
  /** The message of each field that failed the check. The field shows it and is invalid. */
  errors: Partial<Record<DraftField, string>>;
  onChange: (draft: Draft) => void;
}

const GRANULARITIES: Granularity[] = ["day", "week", "month"];

/**
 * The inputs for a value of one value type (ADR 0049). It is a controlled form part: the draft
 * and its errors live in the parent, which checks the draft with `draftToValue`.
 */
export function ValueInput({ type, draft, errors, onChange }: ValueInputProps) {
  const set = (change: Partial<Draft>) => onChange({ ...draft, ...change });
  return (
    <div className={styles.input}>
      <Inputs type={type} draft={draft} errors={errors} set={set} />
      <Checkbox
        label={t("value-input-approximate")}
        isSelected={draft.approximate}
        onChange={(approximate) => set({ approximate })}
      />
    </div>
  );
}

interface InputsProps {
  type: ValueType;
  draft: Draft;
  errors: Partial<Record<DraftField, string>>;
  set: (change: Partial<Draft>) => void;
}

function Inputs({ type, draft, errors, set }: InputsProps) {
  const errorId = useId();
  switch (type.type) {
    case "text":
      return (
        <TextField
          label={t("value-input-text")}
          value={draft.text}
          onChange={(text) => set({ text })}
          error={errors.text}
        />
      );
    case "boolean":
      return (
        <Select
          label={t("value-input-flag")}
          options={[
            { id: "yes", label: t("value-yes") },
            { id: "no", label: t("value-no") },
          ]}
          value={draft.flag === "" ? undefined : draft.flag}
          onChange={(flag) => set({ flag: flag === "yes" ? "yes" : "no" })}
          error={errors.flag}
        />
      );
    case "quantity":
      return (
        <div className={styles.pair}>
          <TextField
            label={t("value-input-min-range")}
            inputMode="decimal"
            value={draft.min}
            onChange={(min) => set({ min })}
            error={errors.min}
          />
          <TextField
            label={t("value-input-max")}
            inputMode="decimal"
            value={draft.max}
            onChange={(max) => set({ max })}
            error={errors.max}
          />
        </div>
      );
    case "money":
      return (
        <div className={styles.pair}>
          <TextField
            label={t("value-input-amount", { currency: type.currency })}
            inputMode="decimal"
            value={draft.min}
            onChange={(min) => set({ min })}
            error={errors.min}
          />
          <TextField
            label={t("value-input-amount-max")}
            inputMode="decimal"
            value={draft.max}
            onChange={(max) => set({ max })}
            error={errors.max}
          />
        </div>
      );
    case "date":
      return (
        <TextField
          label={t("value-input-date")}
          type="date"
          value={draft.date}
          onChange={(date) => set({ date })}
          error={errors.date}
        />
      );
    case "date-window":
      return (
        <div className={styles.pair}>
          <TextField
            label={t("value-input-start")}
            type="date"
            value={draft.start}
            onChange={(start) => set({ start })}
            error={errors.start}
          />
          <TextField
            label={t("value-input-end")}
            type="date"
            value={draft.end}
            onChange={(end) => set({ end })}
            error={errors.end}
          />
          {type.granularity ? null : (
            <Select
              label={t("value-input-granularity")}
              options={GRANULARITIES.map((id) => ({
                id,
                label: t(`value-input-granularity-${id}`),
              }))}
              value={draft.granularity === "" ? undefined : draft.granularity}
              onChange={(id) => set({ granularity: GRANULARITIES.find((g) => g === id) ?? "" })}
              error={errors.granularity}
            />
          )}
        </div>
      );
    case "choice": {
      const options = type.values.map((choice) => ({
        id: choice.key,
        label: formatLabel(choice.label),
      }));
      if (!type.multiple) {
        return (
          <Select
            label={t("value-input-choice")}
            options={options}
            value={draft.keys[0]}
            onChange={(key) => set({ keys: [key] })}
            error={errors.keys}
          />
        );
      }
      return (
        <fieldset className={styles.group} aria-describedby={errors.keys ? errorId : undefined}>
          <legend>{t("value-input-choice")}</legend>
          {options.map((option) => (
            <Checkbox
              key={option.id}
              label={option.label}
              isSelected={draft.keys.includes(option.id)}
              onChange={(selected) =>
                set({
                  keys: selected
                    ? [...draft.keys, option.id]
                    : draft.keys.filter((key) => key !== option.id),
                })
              }
            />
          ))}
          {errors.keys && (
            <p id={errorId} className={styles.error}>
              {errors.keys}
            </p>
          )}
        </fieldset>
      );
    }
    case "reference":
      return null;
  }
}
