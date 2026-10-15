import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import type { PmPolicy } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { ComboBox } from "../ui/ComboBox";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { TextField } from "../ui/TextField";
import type { PolicySave } from "./PmPage";
import styles from "./PmPage.module.css";
import {
  dayBit,
  formOf,
  POLICY_FIELDS,
  type PolicyErrors,
  type PolicyField,
  type PolicyForm,
  type PolicyValues,
  valuesOf,
} from "./pmPolicy";
import { timeZoneLabel, timeZonesWith } from "./timeZones";

const DAYS = [0, 1, 2, 3, 4, 5, 6];
const TIME_FIELDS: PolicyField[] = ["local_time", "quiet_start", "quiet_end"];
const WEEKDAYS_ERROR = "pm-weekdays-error";

export interface PmPolicyFormProps {
  policy: PmPolicy;
  busy: boolean;
  onSave: (values: PolicyValues, expectedVersion: number) => Promise<PolicySave>;
  onStatus: (message: string | undefined) => void;
  onFailure: (message: string | undefined) => void;
}

/** The fields that the server names with a JSON pointer such as `/cooldown_hours`. */
function serverErrors(save: Extract<PolicySave, { kind: "invalid" }>): PolicyErrors {
  const errors: PolicyErrors = {};
  for (const error of save.problem.errors ?? []) {
    const field = error.pointer.slice(1) as PolicyField;
    if (POLICY_FIELDS.includes(field)) {
      errors[field] = true;
    }
  }
  return errors;
}

/**
 * The policy of the reminders as one form: the daily check, the quiet hours and the intervals. The
 * pause and the AI switch are not in the form: they have an immediate effect.
 */
export function PmPolicyForm({ policy, busy, onSave, onStatus, onFailure }: PmPolicyFormProps) {
  const [form, setForm] = useState<PolicyForm>(() => formOf(policy));
  // The policy that the form edits are based on. A save sends its version (ADR 0044), so that a
  // change of another manager is never overwritten without a conflict.
  const [base, setBase] = useState(policy);
  const [zoneText, setZoneText] = useState(() => timeZoneLabel(policy.time_zone));
  const [errors, setErrors] = useState<PolicyErrors>({});
  const formRef = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const show = (next: PmPolicy) => {
    setBase(next);
    setForm(formOf(next));
    setZoneText(timeZoneLabel(next.time_zone));
    setErrors({});
  };

  // A new version from elsewhere (a switch, a reload) shows in the form unless the member edits it.
  // With edits, the form moves to the new version only if no field of the form changed in it: then
  // only a switch changed, and the edits still apply. Otherwise the save gets a conflict.
  const same = (a: PmPolicy, b: PmPolicy) =>
    JSON.stringify(formOf(a)) === JSON.stringify(formOf(b));
  const dirty = JSON.stringify(form) !== JSON.stringify(formOf(base));
  useEffect(() => {
    if (policy.version === base.version) {
      return;
    }
    if (!dirty) {
      show(policy);
    } else if (same(policy, base)) {
      setBase(policy);
    }
  });

  const zones = useMemo(() => timeZonesWith(form.time_zone), [form.time_zone]);
  const savedLabel = timeZoneLabel(form.time_zone);
  const query = zoneText === savedLabel ? "" : zoneText.trim().toLowerCase();
  const zoneOptions = zones
    .map((zone) => ({ id: zone, label: timeZoneLabel(zone) }))
    .filter((option) => option.label.toLowerCase().includes(query));

  const focusFirstError = (found: PolicyErrors) => {
    focusAfterCommit(() => {
      // The days have no `aria-invalid`; they come second in the form.
      if (found.weekdays && !found.local_time) {
        return formRef.current?.querySelector<HTMLElement>("fieldset input") ?? null;
      }
      return firstInvalidField(formRef.current);
    });
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    onFailure(undefined);
    onStatus(undefined);
    // Text that matches no zone of the list is no choice.
    const checked = valuesOf(zoneText === savedLabel ? form : { ...form, time_zone: "" }, policy);
    if ("errors" in checked) {
      setErrors(checked.errors);
      focusFirstError(checked.errors);
      return;
    }
    setErrors({});
    const result = await onSave(checked.values, base.version);
    if (result.kind === "saved") {
      show(result.policy);
      onStatus(t("pm-saved"));
    } else if (result.kind === "invalid") {
      const found = serverErrors(result);
      if (Object.keys(found).length === 0) {
        onFailure(t("problem-validation-failed"));
      }
      setErrors(found);
      focusFirstError(found);
    } else if (result.kind === "conflict") {
      // Without the new values the form keeps the edits, and the text says so.
      if (result.policy) {
        show(result.policy);
      }
      onFailure(t(result.policy ? "pm-conflict" : "pm-conflict-reload-failed"));
    }
  };

  const error = (field: PolicyField) => (errors[field] ? t(`pm-error-${field}`) : undefined);
  const text = (
    field: Exclude<PolicyField, "weekdays" | "time_zone">,
    label: string,
    help?: string,
  ) => (
    <TextField
      label={label}
      {...(help === undefined ? {} : { help })}
      value={form[field]}
      onChange={(value) => {
        setForm({ ...form, [field]: value });
        setErrors({ ...errors, [field]: undefined });
      }}
      error={error(field)}
      inputMode={TIME_FIELDS.includes(field) ? "text" : "numeric"}
    />
  );

  return (
    <section className={styles.section} aria-labelledby="pm-reminders-title">
      <h3 id="pm-reminders-title" className={styles.heading}>
        {t("pm-reminders-title")}
      </h3>
      <form ref={formRef} className={styles.form} onSubmit={(event) => void save(event)} noValidate>
        <div className={styles.group}>
          <h4 className={styles.subheading}>{t("pm-check-title")}</h4>
          <div className={styles.short}>
            {text("local_time", t("pm-time-label"), t("pm-time-help"))}
          </div>
          <fieldset
            className={styles.days}
            {...(errors.weekdays ? { "aria-describedby": WEEKDAYS_ERROR } : {})}
          >
            <legend className={styles.legend}>{t("pm-weekdays-label")}</legend>
            <div className={styles.dayList}>
              {DAYS.map((day) => (
                <Checkbox
                  key={day}
                  label={t(`pm-weekday-${day}`)}
                  isSelected={(form.weekdays & dayBit(day)) !== 0}
                  onChange={(selected) => {
                    setForm({ ...form, weekdays: form.weekdays ^ dayBit(day) });
                    if (selected) {
                      setErrors({ ...errors, weekdays: undefined });
                    }
                  }}
                />
              ))}
            </div>
            {errors.weekdays && (
              <p id={WEEKDAYS_ERROR} className={styles.error}>
                {t("pm-error-weekdays")}
              </p>
            )}
          </fieldset>
          <ComboBox
            label={t("pm-zone-label")}
            help={t("pm-zone-help")}
            options={zoneOptions}
            inputValue={zoneText}
            onInputChange={(value) => {
              setZoneText(value);
              setErrors({ ...errors, time_zone: undefined });
            }}
            selectedKey={form.time_zone}
            onSelectionChange={(zone) => {
              if (zone) {
                setForm({ ...form, time_zone: zone });
                setZoneText(timeZoneLabel(zone));
              }
            }}
            error={error("time_zone")}
          />
        </div>
        <div className={`${styles.group} ${styles.short}`}>
          <h4 className={styles.subheading}>{t("pm-pace-title")}</h4>
          <div className={styles.quiet}>
            {text("quiet_start", t("pm-quiet-start-label"))}
            {text("quiet_end", t("pm-quiet-end-label"))}
          </div>
          <p className={styles.help}>{t("pm-quiet-help")}</p>
          {text("cooldown_hours", t("pm-cooldown-label"), t("pm-cooldown-help"))}
          {text("follow_up_working_days", t("pm-follow-up-label"), t("pm-follow-up-help"))}
          {text("look_ahead_working_days", t("pm-look-ahead-label"), t("pm-look-ahead-help"))}
        </div>
        <div className={styles.actions}>
          <Button type="submit" variant="primary" isPending={busy}>
            {t("pm-save")}
          </Button>
        </div>
      </form>
    </section>
  );
}
