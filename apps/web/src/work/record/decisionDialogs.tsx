// The two actions that end a decision in force (ADR 0081): „Ersetzen“ records a new decision that
// replaces it, and „Widerrufen“ ends it for a reason. Only an event manager runs them; the server
// says per decision whether the member may (`can_end`). Both are dialogs of `RecordActionDialog`.
import { useState } from "react";
import { uuidv7 } from "../../api/uuid";
import { hasMessage, t } from "../../i18n";
import { SearchSelect } from "../../ui/SearchSelect";
import { TextField } from "../../ui/TextField";
import { partyLabel } from "../usePromisorSearch";
import type { WorkItem } from "../workItem";
import { choicesOf } from "./choices";
import { areaField, ownerField } from "./commonFields";
import { wordingError } from "./decisionInput";
import { type ActionDialogProps, type FieldSpec, type FieldValue, peopleValue } from "./descriptor";
import { PeopleField } from "./PeopleField";
import { partiesOf, partyId } from "./parties";
import { RecordActionDialog } from "./RecordActionDialog";
import styles from "./RecordActionDialog.module.css";
import { type Refusal, refusalOf } from "./refusal";

const target = (item: WorkItem) => ({ id: item.id, version: item.version });

function asDecision(item: WorkItem) {
  if (item.kind !== "decision") {
    throw new Error(`a decision dialog met a ${item.kind}`);
  }
  return item;
}

export function RevokeDecisionDialog(props: ActionDialogProps) {
  const { item, env, onDone, onCancel } = props;
  return (
    <RecordActionDialog
      title={t("decision-revoke-title", { id: item.readableId })}
      description={t("decision-revoke-text")}
      reason={{ label: t("decision-revoke-reason"), help: t("decision-revoke-reason-help") }}
      confirmLabel={t("decision-revoke")}
      danger
      onCancel={onCancel}
      onConfirm={async (reason) => {
        const result = await env.commands.decision.revoke(target(item), reason);
        if (result.ok) {
          onDone(t("decision-revoked", { id: item.readableId }));
        }
        return refusalOf(result, props);
      }}
    />
  );
}

type Field = "wording" | "decidedOn" | "parties" | "owner" | "area";
type Errors = Partial<Record<Field, string>>;

/** The form field behind a pointer of the server; those of a successor may start with `/successor`. */
const FIELDS: Record<string, Field> = {
  wording: "wording",
  decided_on: "decidedOn",
  decided_by: "parties",
  owner: "owner",
  owner_user_id: "owner",
  workstream: "area",
  workstream_id: "area",
};
const POINTERS: Record<Field, string> = {
  wording: "wording",
  decidedOn: "decided_on",
  parties: "decided_by",
  owner: "owner",
  area: "workstream",
};

/** The owner and the area as fields: the successor starts with those of the old decision. */
const OWNER = ownerField(() => null);
const AREA = areaField(() => null);

/** A picker with a label above it, for a field of a dialog that is a choice from a list. */
function Pick({
  spec,
  choices,
  value,
  onChange,
  error,
}: {
  spec: FieldSpec;
  choices: ReturnType<typeof choicesOf>;
  value: string | null;
  onChange: (value: string | null) => void;
  error: string | undefined;
}) {
  const label = t(spec.labelKey);
  const empty = spec.emptyKey ? t(spec.emptyKey) : "";
  return (
    <div className={styles.pick} {...(error && { "data-invalid": "" })}>
      <span className={styles.pickLabel} aria-hidden="true">
        {label}
      </span>
      <SearchSelect
        label={label}
        options={choices.options}
        value={value ?? undefined}
        onChange={(id) => onChange(id ?? null)}
        placeholder={empty}
        {...(spec.emptyKey && { clearLabel: empty })}
      />
      {error && <p className={styles.pickError}>{error}</p>}
    </div>
  );
}

/** The message for a field: the one for its code if the catalog has it, else the general one. */
function message(field: Field, code?: string): string {
  const specific = `work-error-${POINTERS[field]}-${code}`;
  return t(code && hasMessage(specific) ? specific : `work-error-${POINTERS[field]}`);
}

/** The refused fields of a `validation-failed` problem of a successor. */
function refused(errors: { pointer: string; code: string }[] | undefined): Errors {
  const result: Errors = {};
  for (const { pointer, code } of errors ?? []) {
    const name = /^(?:\/successor)?\/([a-z_]+)(?:\/\d+)?$/.exec(pointer)?.[1] ?? "";
    const field = FIELDS[name];
    if (field) {
      result[field] = message(field, code);
    }
  }
  return result;
}

/**
 * „Ersetzen“: the new decision starts with the wording, the parties, the owner and the area of the
 * old one; the day is new. The owner and the area are fields, because the old ones may no longer
 * be allowed (the area closed, the owner lost the role). The old decision keeps its wording and
 * becomes „ersetzt“.
 */
export function SupersedeDecisionDialog(props: ActionDialogProps) {
  const { item, env, onDone, onCancel } = props;
  const old = asDecision(item).record;
  const api = env.sources.api;
  // One ID for every try of this dialog.
  const [id] = useState(uuidv7);
  const [wording, setWording] = useState(old.wording);
  const [decidedOn, setDecidedOn] = useState("");
  const [parties, setParties] = useState<FieldValue>(() =>
    peopleValue(old.decided_by.map(partyId)),
  );
  const [owner, setOwner] = useState<string | null>(old.owner_user_id);
  const [area, setArea] = useState<string | null>(old.workstream_id ?? null);
  const [errors, setErrors] = useState<Errors>({});
  const directory = env.sources.directory;
  const seed = old.decided_by.map((party) => ({
    id: partyId(party),
    label: party.redacted ? t("redacted") : partyLabel(party.name, party.local_id),
    ...(!party.redacted && { name: party.name }),
  }));

  const confirm = async (): Promise<Refusal | undefined> => {
    const missing: Errors = {
      ...(wording.trim() === "" && { wording: message("wording") }),
      ...(wordingError(wording) && { wording: wordingError(wording) }),
      ...(decidedOn === "" && { decidedOn: message("decidedOn") }),
      ...(parties === null && { parties: message("parties") }),
    };
    setErrors(missing);
    if (Object.keys(missing).length > 0) {
      return {};
    }
    const result = await env.commands.decision.supersede(target(item), {
      id,
      wording: wording.trim(),
      decided_on: decidedOn,
      decided_by: partiesOf(parties),
      owner_user_id: owner ?? old.owner_user_id,
      ...(area && { workstream_id: area }),
    });
    if (result.ok) {
      // The replaced decision changed too: the panel shows its new state before the dialog closes.
      const words = { id: result.record.local_id, old: item.readableId };
      const loaded = await env.refresh("decision");
      if (loaded) {
        onDone(t("decision-replaced", words));
      } else {
        onDone(t("decision-replaced-stale", words), "alert");
      }
      return undefined;
    }
    setErrors({});
    return refusalOf(result, props, (problem) => {
      const invalid = refused(problem.errors);
      setErrors(invalid);
      return Object.keys(invalid).length > 0 ? {} : undefined;
    });
  };

  return (
    <RecordActionDialog
      title={t("decision-replace-title", { id: item.readableId })}
      description={`${t("decision-replace-note", { id: item.readableId })} ${t("decision-replace-inherit")}`}
      confirmLabel={t("decision-replace")}
      onCancel={onCancel}
      onConfirm={confirm}
    >
      <TextField
        label={`${t("record-field-wording")} ${t("record-required")}`}
        help={t("decision-field-wording-help")}
        value={wording}
        onChange={setWording}
        error={errors.wording}
        rows={5}
        autoFocus
      />
      <TextField
        label={`${t("record-field-decided-on")} ${t("record-required")}`}
        type="date"
        value={decidedOn}
        onChange={setDecidedOn}
        error={errors.decidedOn}
      />
      {api && (
        <PeopleField
          label={`${t("record-field-parties")} ${t("record-required")}`}
          api={api}
          value={parties}
          onChange={setParties}
          seed={seed}
          error={errors.parties}
        />
      )}
      {directory && (
        <>
          <Pick
            spec={OWNER}
            choices={choicesOf(OWNER, env.sources, item, owner)}
            value={owner}
            onChange={setOwner}
            error={errors.owner}
          />
          <Pick
            spec={AREA}
            choices={choicesOf(AREA, env.sources, item, area)}
            value={area}
            onChange={setArea}
            error={errors.area}
          />
        </>
      )}
    </RecordActionDialog>
  );
}
