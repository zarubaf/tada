import { type FormEvent, useRef, useState } from "react";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../../ui/focus";
import { LiveRegion } from "../../ui/LiveRegion";
import { SearchSelect } from "../../ui/SearchSelect";
import { TextField } from "../../ui/TextField";
import { fieldErrors } from "../fieldErrors";
import type { WorkCommands } from "../workCommands";
import { type ChoiceSources, choicesOf } from "./choices";
import type { FieldSpec, FieldValue, RecordDescriptor } from "./descriptor";
import styles from "./RecordPanel.module.css";

type Values = Record<string, FieldValue>;
type Errors = Record<string, string>;

/** A field of text goes in a stacked field; the others (dates, pickers) are rows of the list. */
const isStacked = (spec: FieldSpec) => spec.control === "text" || spec.control === "multiline";

/** The fields of the form in order, in runs of the same layout. */
function runsOf(specs: FieldSpec[]): { stacked: boolean; specs: FieldSpec[] }[] {
  const runs: { stacked: boolean; specs: FieldSpec[] }[] = [];
  for (const spec of specs) {
    const last = runs[runs.length - 1];
    if (last && last.stacked === isStacked(spec)) {
      last.specs.push(spec);
    } else {
      runs.push({ stacked: isStacked(spec), specs: [spec] });
    }
  }
  return runs;
}

function Row({
  spec,
  value,
  error,
  sources,
  onChange,
}: {
  spec: FieldSpec;
  value: FieldValue;
  error: string | undefined;
  sources: ChoiceSources;
  onChange: (value: FieldValue) => void;
}) {
  const choices = choicesOf(spec, sources, undefined, value);
  const Icon = spec.icon;
  const label = t(spec.labelKey);
  const empty = spec.emptyKey ? t(spec.emptyKey) : "";
  return (
    <>
      <dt className={styles.label}>
        {Icon && <Icon size={14} stroke={1.5} aria-hidden="true" />}
        {label}
      </dt>
      <dd className={styles.field} {...(error && { "data-invalid": "" })}>
        <div className={styles.picker}>
          {spec.control === "date" ? (
            <TextField
              label={label}
              labelHidden
              type="date"
              value={value ?? ""}
              onChange={(text) => onChange(text === "" ? null : text)}
              error={error}
            />
          ) : (
            choices.offered && (
              <>
                <SearchSelect
                  label={label}
                  options={choices.options}
                  value={value ?? undefined}
                  onChange={(id) => onChange(id ?? null)}
                  placeholder={empty}
                  variant="quiet"
                  {...(spec.emptyKey && { clearLabel: empty })}
                />
                {error && <p className={styles.error}>{error}</p>}
              </>
            )
          )}
        </div>
      </dd>
    </>
  );
}

/**
 * The create state of the panel: the fields of a kind and „Erstellen“. It checks what it can
 * (a required field) and puts the invalid fields that the server names at their fields. The draft
 * stays through every failure.
 */
export function RecordCreate({
  descriptor,
  commands,
  sources,
  me,
  onCreated,
  announce,
  onCancel,
}: {
  descriptor: RecordDescriptor;
  commands: WorkCommands;
  sources: ChoiceSources;
  /** The member who is signed in: the default owner. */
  me: string;
  /** The readable ID of the record that was created. */
  onCreated: (readableId: string) => void;
  announce: (text: string) => void;
  onCancel: () => void;
}) {
  const create = descriptor.create;
  const [values, setValues] = useState<Values>(() => create?.defaults(me) ?? {});
  const [errors, setErrors] = useState<Errors>({});
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  if (!create) {
    return null;
  }
  const specs = [descriptor.title, ...descriptor.properties, descriptor.description].filter(
    (spec): spec is FieldSpec => spec !== undefined,
  );
  const shown = create.fields
    .map((id) => specs.find((spec) => spec.id === id))
    .filter((spec): spec is FieldSpec => spec !== undefined);
  const set = (id: string, value: FieldValue) =>
    setValues((current) => ({ ...current, [id]: value }));
  const focusInvalid = () => focusAfterCommit(() => firstInvalidField(form.current));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    const missing: Errors = {};
    for (const spec of shown) {
      if (spec.required && (values[spec.id] ?? "").trim() === "") {
        missing[spec.id] = t(`work-error-${spec.pointer}`);
      }
    }
    setFailure(undefined);
    setErrors(missing);
    if (Object.keys(missing).length > 0) {
      focusInvalid();
      return;
    }
    setBusy(true);
    const result = await create.submit(commands, values);
    setBusy(false);
    if (result.ok) {
      announce(t("record-created", { id: result.record.local_id }));
      onCreated(result.record.local_id);
      return;
    }
    const { problem, message } = result.failure;
    const named =
      problem?.code === "validation-failed"
        ? fieldErrors(
            problem,
            shown.map((spec) => spec.pointer),
          )
        : {};
    const invalid: Errors = {};
    for (const spec of shown) {
      const text = named[spec.pointer];
      if (text) {
        invalid[spec.id] = text;
      }
    }
    setErrors(invalid);
    if (Object.keys(invalid).length > 0) {
      focusInvalid();
    } else {
      setFailure(message);
    }
  };

  return (
    <form ref={form} className={styles.body} onSubmit={(event) => void submit(event)} noValidate>
      {runsOf(shown).map((run) =>
        run.stacked ? (
          run.specs.map((spec) => (
            <TextField
              key={spec.id}
              label={
                spec.required ? `${t(spec.labelKey)} ${t("record-required")}` : t(spec.labelKey)
              }
              value={values[spec.id] ?? ""}
              onChange={(text) => set(spec.id, text)}
              error={errors[spec.id]}
              autoComplete="off"
              {...(spec.control === "multiline" && { rows: 4 })}
            />
          ))
        ) : (
          <dl key={run.specs[0]?.id} className={styles.props}>
            {run.specs.map((spec) => (
              <Row
                key={spec.id}
                spec={spec}
                value={values[spec.id] ?? null}
                error={errors[spec.id]}
                sources={sources}
                onChange={(value) => set(spec.id, value)}
              />
            ))}
          </dl>
        ),
      )}
      <LiveRegion kind="alert">{failure}</LiveRegion>
      <div className={styles.actions}>
        <Button type="submit" variant="primary" isPending={busy}>
          {t("record-create")}
        </Button>
        <Button isDisabled={busy} onPress={onCancel}>
          {t("record-cancel")}
        </Button>
      </div>
    </form>
  );
}
