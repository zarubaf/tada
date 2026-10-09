import { type FormEvent, useRef, useState } from "react";
import { type Api, problemMessage, type Workstream, type WorkstreamStatus } from "../api/client";
import { t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import type { Directory } from "./directory";
import { fieldErrors } from "./fieldErrors";
import { OwnerSelect } from "./fields";
import styles from "./Work.module.css";

const FIELDS = ["name", "lead"] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;
const STATUSES: WorkstreamStatus[] = ["active", "closed"];

/** „Arbeitsbereich anlegen“ and the change of a workstream, for an event manager. */
export function WorkstreamForm({
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
  record?: Workstream;
  onSaved: (workstream: Workstream) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [name, setName] = useState(record?.name ?? "");
  const [lead, setLead] = useState(record?.lead_user_id ?? userId);
  const [status, setStatus] = useState<WorkstreamStatus>(record?.status ?? "active");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const focusInvalidField = () => focusAfterCommit(() => firstInvalidField(form.current));
  const path = { event_id: eventId };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    onStart();
    if (busy) {
      return;
    }
    if (name.trim() === "") {
      setErrors({ name: t("work-error-name") });
      focusInvalidField();
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      const result = record
        ? await api.PATCH("/api/v1/events/{event_id}/workstreams/{id}", {
            params: { path: { ...path, id: record.id } },
            body: {
              ...(name.trim() !== record.name && { name: name.trim() }),
              ...(lead !== record.lead_user_id && { lead_user_id: lead }),
              ...(status !== record.status && { status }),
              expected_version: record.version,
            },
          })
        : await api.POST("/api/v1/events/{event_id}/workstreams", {
            params: { path },
            body: { name: name.trim(), lead_user_id: lead },
          });
      if (result.data) {
        if (!record) {
          setName("");
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
        label={t("workstream-field-name")}
        value={name}
        onChange={setName}
        error={errors.name}
        autoComplete="off"
        isRequired
      />
      <OwnerSelect
        directory={directory}
        label={t("workstream-field-lead")}
        value={lead}
        current={record?.lead_user_id}
        onChange={setLead}
        error={errors.lead}
      />
      {record && (
        <Select
          label={t("work-field-status")}
          options={STATUSES.map((s) => ({ id: s, label: t(`workstream-status-${s}`) }))}
          value={status}
          onChange={(id) => setStatus(id as WorkstreamStatus)}
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
