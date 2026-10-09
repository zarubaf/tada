import { type FormEvent, useEffect, useRef, useState } from "react";
import { type Api, type Commitment, type CommitmentStatus, problemMessage } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import type { Directory } from "./directory";
import { blankToNull, fieldErrors, type SaveFailure, saveFailure } from "./fieldErrors";
import { NO_WORKSTREAM, OwnerSelect, WorkstreamSelect } from "./fields";
import { commitmentStatusChoices } from "./status";
import styles from "./Work.module.css";

const FIELDS = ["text", "condition", "promisor", "owner", "workstream", "due"] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;

type Promisors =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  /** The ID of an option is `person:{uuid}` or `institution:{uuid}`. */
  | { kind: "loaded"; options: { id: string; label: string }[] };

/** The persons and institutions of the organization, for the choice of the promisor. */
function usePromisors(api: Api, enabled: boolean): Promisors {
  const [state, setState] = useState<Promisors>({ kind: "loading" });
  useEffect(() => {
    if (!enabled) {
      return;
    }
    let current = true;
    void (async () => {
      let next: Promisors;
      try {
        const query = { limit: 200 };
        const [persons, institutions] = await Promise.all([
          api.GET("/api/v1/persons", { params: { query } }),
          api.GET("/api/v1/institutions", { params: { query } }),
        ]);
        next =
          persons.data && institutions.data
            ? {
                kind: "loaded",
                options: [
                  ...persons.data.items.map((p) => ({
                    id: `person:${p.id}`,
                    label: `${p.name} (${p.local_id})`,
                  })),
                  ...institutions.data.items.map((i) => ({
                    id: `institution:${i.id}`,
                    label: `${i.name} (${i.local_id})`,
                  })),
                ],
              }
            : { kind: "failed", message: problemMessage(persons.error ?? institutions.error) };
      } catch {
        next = { kind: "failed", message: problemMessage(undefined) };
      }
      if (current) {
        setState(next);
      }
    })();
    return () => {
      current = false;
    };
  }, [api, enabled]);
  return state;
}

/** The body of a change: only the fields that differ from the saved record. */
function changes(
  record: Commitment,
  input: {
    text: string;
    owner: string;
    workstream: string | null;
    due: string | null;
    status: CommitmentStatus;
  },
) {
  return {
    ...(input.text !== record.text && { text: input.text }),
    ...(input.owner !== record.owner_user_id && { owner_user_id: input.owner }),
    ...(input.workstream !== (record.workstream_id ?? null) && { workstream_id: input.workstream }),
    ...(input.due !== (record.due_date ?? null) && { due_date: input.due }),
    ...(input.status !== record.status && { status: input.status }),
  };
}

/**
 * „Zusage erfassen“ and the change of a commitment. The condition and the promisor never change,
 * so a change shows them as text. The status `firm` has its own command („Verbindlich machen“).
 */
export function CommitmentForm({
  api,
  eventId,
  directory,
  userId,
  record,
  onSaved,
  onFailed,
  onStart,
  onCancel,
}: {
  api: Api;
  eventId: string;
  directory: Directory;
  userId: string;
  record?: Commitment;
  onSaved: (commitment: Commitment) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [text, setText] = useState(record?.text ?? "");
  const [condition, setCondition] = useState("");
  const [promisor, setPromisor] = useState<string>();
  const [owner, setOwner] = useState(record?.owner_user_id ?? userId);
  const [workstream, setWorkstream] = useState(record?.workstream_id ?? NO_WORKSTREAM);
  const [due, setDue] = useState(record?.due_date ?? "");
  const [status, setStatus] = useState<CommitmentStatus>(record?.status ?? "firm");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const focusInvalidField = () => focusAfterCommit(() => firstInvalidField(form.current));
  const promisors = usePromisors(api, record === undefined);
  const path = { event_id: eventId };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    onStart();
    if (busy) {
      return;
    }
    const missing: FieldErrors = {
      ...(text.trim() === "" && { text: t("work-error-text") }),
      ...(!record && promisor === undefined && { promisor: t("work-error-promisor") }),
    };
    if (Object.keys(missing).length > 0) {
      setErrors(missing);
      focusInvalidField();
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      const input = {
        text: text.trim(),
        owner,
        workstream: workstream === NO_WORKSTREAM ? null : workstream,
        due: blankToNull(due),
        status,
      };
      const [kind, id] = (promisor ?? "").split(":");
      const result = record
        ? await api.PATCH("/api/v1/events/{event_id}/commitments/{id}", {
            params: { path: { ...path, id: record.id } },
            body: { ...changes(record, input), expected_version: record.version },
          })
        : await api.POST("/api/v1/events/{event_id}/commitments", {
            params: { path },
            body: {
              text: input.text,
              owner_user_id: owner,
              promisor: { kind: kind === "person" ? "person" : "institution", id: id ?? "" },
              ...(blankToNull(condition) !== null && { condition: blankToNull(condition) }),
              ...(input.workstream !== null && { workstream_id: input.workstream }),
              ...(input.due !== null && { due_date: input.due }),
            },
          });
      if (result.data) {
        if (!record) {
          setText("");
          setCondition("");
          setPromisor(undefined);
          setDue("");
          setWorkstream(NO_WORKSTREAM);
          focusAfterCommit(() => form.current?.querySelector<HTMLElement>("input"));
        }
        onSaved(result.data);
      } else {
        const invalid =
          result.error?.code === "validation-failed" ? fieldErrors(result.error, FIELDS) : {};
        setErrors(invalid);
        if (Object.keys(invalid).length > 0) {
          focusInvalidField();
        } else {
          onFailed(saveFailure(result));
        }
      }
    } catch {
      onFailed({ message: problemMessage(undefined), conflict: false });
    }
    setBusy(false);
  };

  return (
    <form ref={form} className={styles.form} onSubmit={(event) => void submit(event)} noValidate>
      <TextField
        label={t("commitment-field-text")}
        value={text}
        onChange={setText}
        error={errors.text}
        autoComplete="off"
        isRequired
      />
      {record ? (
        <>
          <p>
            <span className={styles.fixedLabel}>{t("commitment-field-promisor-fixed")}: </span>
            {record.promisor.name}
          </p>
          {record.condition && (
            <p>
              <span className={styles.fixedLabel}>{t("commitment-field-condition-fixed")}: </span>
              {record.condition}
            </p>
          )}
        </>
      ) : (
        <>
          {promisors.kind === "loading" && <Skeleton />}
          {promisors.kind === "loaded" && (
            <Select
              label={t("commitment-field-promisor")}
              placeholder={t("commitment-field-promisor-placeholder")}
              options={promisors.options}
              value={promisor}
              onChange={setPromisor}
              error={errors.promisor}
            />
          )}
          {promisors.kind === "failed" && <p role="alert">{promisors.message}</p>}
          <TextField
            label={t("commitment-field-condition")}
            value={condition}
            onChange={setCondition}
            error={errors.condition}
            help={t("commitment-field-condition-help")}
            autoComplete="off"
          />
        </>
      )}
      <OwnerSelect
        directory={directory}
        label={t("work-field-owner")}
        value={owner}
        current={record?.owner_user_id}
        onChange={setOwner}
        error={errors.owner}
      />
      <WorkstreamSelect
        directory={directory}
        value={workstream}
        current={record?.workstream_id}
        onChange={setWorkstream}
        error={errors.workstream}
      />
      <TextField
        label={t("work-field-due")}
        type="date"
        value={due}
        onChange={setDue}
        error={errors.due}
      />
      {record && (
        <Select
          label={t("work-field-status")}
          options={commitmentStatusChoices(record.status).map((s) => ({
            id: s,
            label: t(`commitment-status-${s}`),
          }))}
          value={status}
          onChange={(id) => setStatus(id as CommitmentStatus)}
        />
      )}
      <div className={styles.actions}>
        {onCancel && (
          <Button onPress={onCancel} isDisabled={busy}>
            {t("work-cancel")}
          </Button>
        )}
        <Button type="submit" variant="primary" isPending={busy}>
          {record ? t("work-save") : t("work-create")}
        </Button>
      </div>
    </form>
  );
}
