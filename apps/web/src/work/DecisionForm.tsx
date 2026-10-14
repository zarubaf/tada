import { type FormEvent, useRef, useState } from "react";
import { type Api, type Decision, type Problem, problemMessage } from "../api/client";
import { hasMessage, t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { TextField } from "../ui/TextField";
import type { Choice, Directory } from "./directory";
import { NO_WORKSTREAM, OwnerSelect, WorkstreamSelect } from "./fields";
import { MAX_PARTIES, PartiesSelect } from "./PartiesSelect";
import styles from "./Work.module.css";

const MAX_WORDING = 2000;

/** A control character other than the line break and the tab, which a wording may hold. */
const FORBIDDEN_CHARACTER = /[\p{Cc}]/u;
const hasForbiddenCharacter = (text: string) =>
  FORBIDDEN_CHARACTER.test(text.replace(/[\n\t]/g, ""));

const FIELDS = ["wording", "decided_on", "decided_by", "owner", "workstream"] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;

/** The field of the form behind a pointer of the server. */
const POINTER_FIELDS: Record<string, Field> = {
  wording: "wording",
  decided_on: "decided_on",
  decided_by: "decided_by",
  owner_user_id: "owner",
  workstream_id: "workstream",
};

/**
 * The refused fields of a `validation-failed` problem. The pointers of a new decision are
 * `/wording` and `/decided_by/2`; those of a successor start with `/successor`.
 */
function decisionFieldErrors(problem: Problem | undefined): FieldErrors {
  const result: FieldErrors = {};
  for (const { pointer, code } of problem?.errors ?? []) {
    const name = /^(?:\/successor)?\/([a-z_]+)(?:\/\d+)?$/.exec(pointer)?.[1] ?? "";
    const field = POINTER_FIELDS[name];
    if (field) {
      result[field] = fieldMessage(field, code);
    }
  }
  return result;
}

function fieldMessage(field: Field, code: string): string {
  const candidates = [
    `decision-error-${field}-${code}`,
    `work-error-${field}-${code}`,
    `decision-error-${field}`,
    `work-error-${field}`,
  ];
  return t(candidates.find((candidate) => hasMessage(candidate)) ?? "problem-validation-failed");
}

/** The party of a decision as a choice of the picker. */
function partyChoice(party: Decision["decided_by"][number]): Choice {
  return {
    id: `${party.kind}:${party.id}`,
    label: party.redacted ? party.local_id : `${party.name} (${party.local_id})`,
  };
}

/** What the note of the replace form says: the state of the form, not a one-time message. */
function replaceNote(ended: boolean, stale: boolean) {
  if (ended) {
    return "decision-replace-ended";
  }
  return stale ? "decision-replace-stale" : "decision-replace-note";
}

/**
 * „Entscheid erfassen“, and the new decision that replaces another („Ersetzen“). The wording
 * has no edit (ADR 0081): a decision that changes is a new one. A successor starts with the words,
 * the parties, the owner and the workstream of the decision that it replaces; the date is new.
 * The page owns the live regions, so the form reports the result: `onSaved` for the new decision,
 * `onFailed` for any other failure.
 */
export function DecisionForm({
  api,
  eventId,
  directory,
  userId,
  replaces,
  onSaved,
  onFailed,
  onStart,
  onAnnounce,
  onCancel,
}: {
  api: Api;
  eventId: string;
  directory: Directory;
  /** The member who is signed in: the owner of a new decision. */
  userId: string;
  /** The decision in force that the new one replaces. Without it, the form records a decision. */
  replaces?: Decision;
  onSaved: (decision: Decision) => void;
  /** The form stays busy until the page has dealt with the failure (for example a reload). */
  onFailed: (failure: SaveFailure) => Promise<void>;
  onStart: () => void;
  /** Tells a change that does not move focus, for the polite region of the page. */
  onAnnounce: (message: string) => void;
  onCancel?: () => void;
}) {
  const [wording, setWording] = useState(replaces?.wording ?? "");
  const [decidedOn, setDecidedOn] = useState("");
  const [parties, setParties] = useState<Choice[]>(replaces?.decided_by.map(partyChoice) ?? []);
  const [owner, setOwner] = useState(replaces?.owner_user_id ?? userId);
  const [workstream, setWorkstream] = useState(replaces?.workstream_id ?? NO_WORKSTREAM);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  // The version that a conflict refused: the page has not loaded a newer one yet. While the form
  // is busy the button stays on, so focus is not lost; the page moves focus before the form frees.
  const [refusedVersion, setRefusedVersion] = useState<number>();
  // The decision that the form replaces ended meanwhile, or the member may no longer end it: the
  // typed text stays, a send cannot work.
  const ended = replaces !== undefined && (!replaces.can_end || replaces.status !== "in-force");
  const stale = replaces !== undefined && replaces.version === refusedVersion;
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const focusInvalidField = () => focusAfterCommit(() => firstInvalidField(form.current));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) {
      return;
    }
    onStart();
    const missing: FieldErrors = {
      ...(wording.trim() === "" && { wording: fieldMessage("wording", "required") }),
      // The server counts the characters of the trimmed text, not the UTF-16 units.
      ...([...wording.trim()].length > MAX_WORDING && {
        wording: fieldMessage("wording", "too-long"),
      }),
      ...(hasForbiddenCharacter(wording.trim()) && {
        wording: fieldMessage("wording", "control-character"),
      }),
      ...(decidedOn === "" && { decided_on: fieldMessage("decided_on", "required") }),
      ...(parties.length === 0 && { decided_by: fieldMessage("decided_by", "required") }),
      ...(parties.length > MAX_PARTIES && { decided_by: fieldMessage("decided_by", "too-long") }),
    };
    setErrors(missing);
    if (Object.keys(missing).length > 0) {
      focusInvalidField();
      return;
    }
    setBusy(true);
    let failure: SaveFailure | undefined;
    try {
      const chosenWorkstream = workstream === NO_WORKSTREAM ? null : workstream;
      const decision = {
        wording: wording.trim(),
        decided_on: decidedOn,
        decided_by: parties.map(({ id }) => {
          const [kind, uuid] = id.split(":");
          return {
            kind: kind === "person" ? ("person" as const) : ("institution" as const),
            id: uuid ?? "",
          };
        }),
        owner_user_id: owner,
        ...(chosenWorkstream !== null && { workstream_id: chosenWorkstream }),
      };
      const result = replaces
        ? await api.POST("/api/v1/events/{event_id}/decisions/{id}/supersede", {
            params: { path: { event_id: eventId, id: replaces.id } },
            body: { successor: decision, expected_version: replaces.version },
          })
        : await api.POST("/api/v1/events/{event_id}/decisions", {
            params: { path: { event_id: eventId } },
            body: decision,
          });
      if (result.data) {
        if (!replaces) {
          setWording("");
          setDecidedOn("");
          setParties([]);
          setWorkstream(NO_WORKSTREAM);
          focusAfterCommit(() => form.current?.querySelector<HTMLElement>("textarea"));
        }
        onSaved(result.data);
      } else {
        const invalid =
          result.error?.code === "validation-failed" ? decisionFieldErrors(result.error) : {};
        setErrors(invalid);
        if (Object.keys(invalid).length > 0) {
          focusInvalidField();
        } else {
          failure = saveFailure(result);
        }
      }
    } catch {
      failure = { message: problemMessage(undefined), conflict: false };
    }
    if (failure) {
      if (failure.conflict && replaces) {
        setRefusedVersion(replaces.version);
      }
      // The page may load the record again: the form stays busy until it is done.
      await onFailed(failure);
    }
    setBusy(false);
  };

  return (
    <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
      {replaces && <p>{t(replaceNote(ended, stale), { id: replaces.local_id })}</p>}
      <TextField
        label={t("decision-field-wording")}
        help={t("decision-field-wording-help")}
        value={wording}
        onChange={setWording}
        error={errors.wording}
        rows={5}
        isRequired
      />
      <TextField
        label={t("decision-field-decided-on")}
        type="date"
        value={decidedOn}
        onChange={setDecidedOn}
        error={errors.decided_on}
        isRequired
      />
      <PartiesSelect
        api={api}
        label={t("decision-field-decided-by")}
        listLabel={t("decision-parties-list")}
        value={parties}
        onChange={setParties}
        onAnnounce={onAnnounce}
        error={errors.decided_by}
      />
      <OwnerSelect
        directory={directory}
        label={t("work-field-owner")}
        value={owner}
        current={replaces?.owner_user_id}
        onChange={setOwner}
        error={errors.owner}
      />
      <WorkstreamSelect
        directory={directory}
        value={workstream}
        current={replaces?.workstream_id}
        onChange={setWorkstream}
        error={errors.workstream}
      />
      <div className={styles.actions}>
        {onCancel && (
          <Button onPress={onCancel} isDisabled={busy}>
            {t("work-cancel")}
          </Button>
        )}
        <Button
          type="submit"
          variant="primary"
          isPending={busy}
          isDisabled={(ended || stale) && !busy}
        >
          {replaces ? t("decision-replace") : t("work-create")}
        </Button>
      </div>
    </form>
  );
}
