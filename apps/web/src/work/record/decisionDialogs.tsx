// The two actions that end a decision in force (ADR 0081): „Ersetzen“ records a new decision that
// replaces it, and „Widerrufen“ ends it for a reason. Only an event manager runs them; the server
// says per decision whether the member may (`can_end`). Both are dialogs of `RecordActionDialog`.
import { useState } from "react";
import { uuidv7 } from "../../api/uuid";
import { hasMessage, t } from "../../i18n";
import { TextField } from "../../ui/TextField";
import { fieldErrors } from "../fieldErrors";
import type { WorkItem } from "../workItem";
import { partiesOf, partyId, wordingError } from "./decisionInput";
import { type ActionDialogProps, type FieldValue, peopleValue } from "./descriptor";
import { PeopleField } from "./PeopleField";
import { RecordActionDialog, type Refusal } from "./RecordActionDialog";

const target = (item: WorkItem) => ({ id: item.id, version: item.version });

function asDecision(item: WorkItem) {
  if (item.kind !== "decision") {
    throw new Error(`a decision dialog met a ${item.kind}`);
  }
  return item;
}

export function RevokeDecisionDialog({ item, env, onDone, onCancel }: ActionDialogProps) {
  return (
    <RecordActionDialog
      title={t("decision-revoke-title", { id: item.readableId })}
      description={t("decision-revoke-text")}
      reason={{ label: t("decision-revoke-reason"), help: t("decision-revoke-reason-help") }}
      confirmLabel={t("decision-revoke")}
      danger
      onCancel={onCancel}
      onConfirm={async (reason): Promise<Refusal | undefined> => {
        const result = await env.commands.decision.revoke(target(item), reason);
        if (result.ok) {
          onDone(t("decision-revoked", { id: item.readableId }));
          return undefined;
        }
        const { failure } = result;
        if (failure.reason === "conflict") {
          void env.refresh("decision");
        }
        const invalid =
          failure.problem?.code === "validation-failed"
            ? fieldErrors(failure.problem, ["reason"] as const).reason
            : undefined;
        return invalid ? { reason: invalid } : { message: failure.message };
      }}
    />
  );
}

type Field = "wording" | "decidedOn" | "parties";
type Errors = Partial<Record<Field, string>>;

/** The form field behind a pointer of the server; those of a successor start with `/successor`. */
const FIELDS: Record<string, Field> = {
  wording: "wording",
  decided_on: "decidedOn",
  decided_by: "parties",
};
const POINTERS: Record<Field, string> = {
  wording: "wording",
  decidedOn: "decided_on",
  parties: "decided_by",
};

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
 * „Ersetzen“: the new decision starts with the wording and the parties of the old one; the day is
 * new. It takes the owner and the area of the old decision, and a manager can change them on the
 * new decision afterwards. The old decision keeps its wording and becomes „ersetzt“.
 */
export function SupersedeDecisionDialog({ item, env, onDone, onCancel }: ActionDialogProps) {
  const old = asDecision(item).record;
  const api = env.sources.api;
  // One ID for every try of this dialog.
  const [id] = useState(uuidv7);
  const [wording, setWording] = useState(old.wording);
  const [decidedOn, setDecidedOn] = useState("");
  const [parties, setParties] = useState<FieldValue>(() =>
    peopleValue(old.decided_by.map(partyId)),
  );
  const [errors, setErrors] = useState<Errors>({});
  const seed = old.decided_by.map((party) => ({
    id: partyId(party),
    label: party.redacted ? party.local_id : `${party.name} (${party.local_id})`,
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
      owner_user_id: old.owner_user_id,
      ...(old.workstream_id && { workstream_id: old.workstream_id }),
    });
    if (result.ok) {
      // The replaced decision changed too.
      void env.refresh("decision");
      onDone(t("decision-replaced", { id: result.record.local_id, old: item.readableId }));
      return undefined;
    }
    const { failure } = result;
    if (failure.reason === "conflict") {
      void env.refresh("decision");
    }
    const invalid =
      failure.problem?.code === "validation-failed" ? refused(failure.problem.errors) : {};
    setErrors(invalid);
    return Object.keys(invalid).length > 0 ? {} : { message: failure.message };
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
        label={`${t("decision-replace-decided-on")} ${t("record-required")}`}
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
    </RecordActionDialog>
  );
}
