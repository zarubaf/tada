import { type FormEvent, useRef, useState } from "react";
import { type Api, type Milestone, type MilestoneStatus, problemMessage } from "../api/client";
import { t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import type { Directory } from "./directory";
import { blankToNull, fieldErrors } from "./fieldErrors";
import { NO_WORKSTREAM, OwnerSelect, WorkstreamSelect } from "./fields";
import styles from "./Work.module.css";

const FIELDS = ["title", "description", "date", "owner", "workstream"] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;

interface Input {
  title: string;
  description: string | null;
  date: string;
  owner: string;
  workstream: string | null;
  status: MilestoneStatus;
}

/** The body of a change: only the fields that differ from the saved milestone. */
function changes(record: Milestone, input: Input) {
  return {
    ...(input.title !== record.title && { title: input.title }),
    ...(input.description !== (record.description ?? null) && { description: input.description }),
    ...(input.date !== record.date && { date: input.date }),
    ...(input.owner !== record.owner_user_id && { owner_user_id: input.owner }),
    ...(input.workstream !== (record.workstream_id ?? null) && { workstream_id: input.workstream }),
    ...(input.status !== record.status && { status: input.status }),
  };
}

/**
 * „Meilenstein erfassen“ and the change of a milestone. The page owns the live regions, so the
 * form reports the result: `onSaved` for a saved milestone, `onFailed` for any other failure.
 */
export function MilestoneForm({
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
  /** The member who is signed in: the owner of a new milestone. */
  userId: string;
  /** The milestone to change. Without it, the form creates a milestone. */
  record?: Milestone;
  onSaved: (milestone: Milestone) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [title, setTitle] = useState(record?.title ?? "");
  const [description, setDescription] = useState(record?.description ?? "");
  const [date, setDate] = useState(record?.date ?? "");
  const [owner, setOwner] = useState(record?.owner_user_id ?? userId);
  const [workstream, setWorkstream] = useState(record?.workstream_id ?? NO_WORKSTREAM);
  const [status, setStatus] = useState<MilestoneStatus>(record?.status ?? "open");
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
    const missing: FieldErrors = {
      ...(title.trim() === "" && { title: t("work-error-title") }),
      ...(date === "" && { date: t("work-error-date") }),
    };
    setErrors(missing);
    if (Object.keys(missing).length > 0) {
      focusInvalidField();
      return;
    }
    setBusy(true);
    try {
      const input: Input = {
        title: title.trim(),
        description: blankToNull(description),
        date,
        owner,
        workstream: workstream === NO_WORKSTREAM ? null : workstream,
        status,
      };
      const result = record
        ? await api.PATCH("/api/v1/events/{event_id}/milestones/{id}", {
            params: { path: { ...path, id: record.id } },
            body: { ...changes(record, input), expected_version: record.version },
          })
        : await api.POST("/api/v1/events/{event_id}/milestones", {
            params: { path },
            body: {
              title: input.title,
              date,
              owner_user_id: owner,
              ...(input.description !== null && { description: input.description }),
              ...(input.workstream !== null && { workstream_id: input.workstream }),
            },
          });
      if (result.data) {
        if (!record) {
          setTitle("");
          setDescription("");
          setDate("");
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
        label={t("milestone-field-title")}
        value={title}
        onChange={setTitle}
        error={errors.title}
        autoComplete="off"
        isRequired
      />
      <TextField
        label={t("milestone-field-description")}
        value={description}
        onChange={setDescription}
        error={errors.description}
        rows={3}
      />
      <TextField
        label={t("milestone-field-date")}
        type="date"
        value={date}
        onChange={setDate}
        error={errors.date}
        isRequired
      />
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
      {record && (
        <Select
          label={t("work-field-status")}
          options={[record.status, ...record.next_statuses].map((s) => ({
            id: s,
            label: t(`milestone-status-${s}`),
          }))}
          value={status}
          onChange={(id) => setStatus(id as MilestoneStatus)}
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
