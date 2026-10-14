import { type FocusEvent, type KeyboardEvent, useEffect, useId, useRef } from "react";
import { formatDate } from "../../facts/formatValue";
import { t } from "../../i18n";
import { Button } from "../../ui/Button";
import { SearchSelect } from "../../ui/SearchSelect";
import { TextField } from "../../ui/TextField";
import type { WorkItem } from "../workItem";
import { type ChoiceSources, choicesOf } from "./choices";
import type { FieldSpec, FieldValue } from "./descriptor";
import { FieldView } from "./FieldView";
import styles from "./RecordPanel.module.css";
import { type FieldServices, useFieldEdit } from "./useFieldEdit";

export interface FieldEnv extends FieldServices {
  sources: ChoiceSources;
  today: string;
  /** False in a closed event, for a redacted record, and for a member who may not change it. */
  editable: boolean;
}

/** What a screen reader hears as the name of the button that starts an edit: the field only. */
function editName(spec: FieldSpec) {
  return t("record-edit-field", { field: t(spec.labelKey) });
}

/** The text of a value for a name: the words for no value when there is none. */
function valueText(spec: FieldSpec, value: FieldValue): string {
  if (value === null || value === "") {
    return spec.emptyKey ? t(spec.emptyKey) : "";
  }
  return spec.control === "date" ? formatDate(value) : value;
}

/** The visible sign of a save. A screen reader hears the same words from the live region. */
function Saving() {
  return (
    <span className={styles.saving} aria-hidden="true">
      {t("record-saving")}
    </span>
  );
}

/** The label of the saved choice of a picker: what the record holds now. */
function pickedText(spec: FieldSpec, sources: ChoiceSources, item: WorkItem, value: FieldValue) {
  const label = choicesOf(spec, sources, item, value).options.find(
    (option) => option.id === value,
  )?.label;
  return label ?? (spec.emptyKey ? t(spec.emptyKey) : "");
}

interface TextEditorProps {
  spec: FieldSpec;
  draft: FieldValue;
  busy: boolean;
  /** A conflict: the draft saves on purpose only, never by a blur. */
  conflict: boolean;
  /** What the record holds now, shown beside the draft in a conflict. */
  theirs: string;
  error: string | undefined;
  onChange: (draft: FieldValue) => void;
  /** `focus` is true when the member saved by key: focus then returns to the display. */
  onCommit: (focus: boolean) => void;
  onCancel: (focus: boolean) => void;
}

/**
 * The input of a text, a long text or a date. Enter saves, Escape cancels, a blur saves a single
 * line. After a conflict only Enter and „Speichern“ save.
 */
function TextEditor({
  spec,
  draft,
  busy,
  conflict,
  theirs,
  error,
  onChange,
  onCommit,
  onCancel,
}: TextEditorProps) {
  const multiline = spec.control === "multiline";
  const container = useRef<HTMLDivElement>(null);

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      // The first Escape ends the edit; the panel closes on the next one.
      event.preventDefault();
      onCancel(true);
    } else if (event.key === "Enter" && (!multiline || event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      onCommit(true);
    }
  };

  // A blur saves a single line, unless focus moves to a button of this editor. A date that is only
  // half typed has no value yet; it is not a request to clear the field.
  const onBlur = (event: FocusEvent<HTMLDivElement>) => {
    const next = event.relatedTarget;
    if (multiline || conflict || (next instanceof Node && container.current?.contains(next))) {
      return;
    }
    const input = event.target;
    if (input instanceof HTMLInputElement && input.validity.badInput) {
      onCancel(false);
      return;
    }
    onCommit(false);
  };

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: the keys belong to the input inside
    <div ref={container} onKeyDown={onKeyDown} onBlur={onBlur}>
      <TextField
        label={t(spec.labelKey)}
        labelHidden
        value={draft ?? ""}
        // A title wraps, so it has room for two lines, but it has no line break.
        onChange={(text) => onChange(multiline ? text : text.replace(/\s*\n\s*/g, " "))}
        isReadOnly={busy}
        error={error}
        autoFocus
        autoComplete="off"
        {...(spec.control === "date" && { type: "date" })}
        {...(spec.control === "date" ? {} : { rows: multiline ? 5 : 2 })}
      />
      {conflict && <p className={styles.theirs}>{t("record-now-saved", { value: theirs })}</p>}
      {(multiline || conflict) && (
        <div className={styles.actions}>
          <Button variant="primary" isPending={busy} onPress={() => onCommit(true)}>
            {t("record-save")}
          </Button>
          <Button isDisabled={busy} onPress={() => onCancel(true)}>
            {t("record-cancel")}
          </Button>
        </div>
      )}
    </div>
  );
}

/**
 * One field of the panel. It shows the value as text, and as the control of its kind when the
 * member may change it: a click or Enter edits a text or a date, and a picker opens its list. A
 * save marks the field busy; a failure shows its message here and keeps the draft.
 */
export function FieldEditor({
  spec,
  item,
  env,
  heading = false,
  valueId,
  focusOnMount = false,
}: {
  spec: FieldSpec;
  item: WorkItem;
  env: FieldEnv;
  /** The field is the heading of the panel, so its text is the display button. */
  heading?: boolean;
  /**
   * The ID of the element that holds the value. The button points at it as its description, and
   * the page can name a heading by it.
   */
  valueId?: string;
  /** Focus goes to the display button once, for the title of a record that was just created. */
  focusOnMount?: boolean;
}) {
  const edit = useFieldEdit(spec, item, env);
  const value = spec.read(item);
  const picker =
    spec.control === "status" ||
    spec.control === "person" ||
    spec.control === "area" ||
    spec.control === "milestone";
  const shown = edit.phase === "idle" ? value : edit.draft;
  const choices = choicesOf(spec, env.sources, item, shown);
  const editable = env.editable && spec.save !== undefined && choices.offered;
  const display = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const errorId = useId();
  const ownValueId = useId();
  const valueElement = valueId ?? ownValueId;

  // biome-ignore lint/correctness/useExhaustiveDependencies: once, when the field appears
  useEffect(() => {
    if (focusOnMount) {
      display.current?.focus();
    }
  }, []);

  // After an edit by keyboard the display button is back; focus goes to it.
  const idle = edit.phase === "idle";
  useEffect(() => {
    if (idle && restoreFocus.current) {
      restoreFocus.current = false;
      display.current?.focus();
    }
  }, [idle]);

  const view = (
    <FieldView spec={spec} value={shown} choices={choices} item={item} today={env.today} />
  );
  if (!editable) {
    return heading ? view : <span className={styles.plain}>{view}</span>;
  }

  const busy = edit.phase === "saving";

  if (picker) {
    const emptyText = spec.emptyKey ? t(spec.emptyKey) : "";
    return (
      <div className={styles.picker} aria-busy={busy || undefined}>
        <SearchSelect
          label={t(spec.labelKey)}
          options={choices.options}
          value={shown ?? undefined}
          onChange={(id) => edit.commit(id ?? null)}
          isPending={busy}
          placeholder={emptyText}
          variant="quiet"
          searchable={spec.control !== "status"}
          {...(spec.emptyKey && { clearLabel: emptyText })}
        />
        {(edit.phase === "failed" || edit.phase === "conflict") && (
          // biome-ignore lint/a11y/useSemanticElements: a fieldset groups form fields, not a picker with its error
          <div role="group" aria-label={t(spec.labelKey)} aria-describedby={errorId}>
            <p id={errorId} className={styles.error}>
              {edit.error}
            </p>
            {edit.phase === "conflict" && (
              <p className={styles.theirs}>
                {t("record-now-saved", { value: pickedText(spec, env.sources, item, value) })}
              </p>
            )}
            <div className={styles.actions}>
              <Button onPress={() => edit.commit()}>{t("record-retry")}</Button>
              <Button onPress={edit.cancel}>{t("record-discard")}</Button>
            </div>
          </div>
        )}
      </div>
    );
  }

  if (edit.phase === "idle") {
    return (
      <button
        ref={display}
        type="button"
        className={heading ? `${styles.display} ${styles.displayTitle}` : styles.display}
        aria-label={editName(spec)}
        aria-describedby={valueElement}
        onClick={edit.begin}
      >
        <span id={valueElement} className={styles.valueText}>
          {view}
        </span>
      </button>
    );
  }

  return (
    <div aria-busy={busy || undefined} className={styles.editing}>
      <TextEditor
        spec={spec}
        draft={edit.draft}
        busy={busy}
        conflict={edit.phase === "conflict"}
        theirs={valueText(spec, value)}
        error={edit.error}
        onChange={edit.change}
        onCommit={(focus) => {
          restoreFocus.current = focus;
          edit.commit();
        }}
        onCancel={(focus) => {
          restoreFocus.current = focus;
          edit.cancel();
        }}
      />
      {busy && <Saving />}
    </div>
  );
}
