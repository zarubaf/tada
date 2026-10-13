import { type FormEvent, useRef, useState } from "react";
import { type Action, type ActionStatus, type Api, problemMessage } from "../api/client";
import { t } from "../i18n";
import { type SaveFailure, saveFailure } from "../registers/saveFailure";
import { Button } from "../ui/Button";
import { firstInvalidField, useFocusAfterCommit } from "../ui/focus";
import { Select } from "../ui/Select";
import { TextField } from "../ui/TextField";
import type { Directory } from "./directory";
import { blankToNull, fieldErrors } from "./fieldErrors";
import {
  MilestoneSelect,
  NO_MILESTONE,
  NO_WORKSTREAM,
  OwnerSelect,
  WorkstreamSelect,
} from "./fields";
import styles from "./Work.module.css";

const FIELDS = [
  "title",
  "description",
  "owner",
  "workstream",
  "start_date",
  "due",
  "milestone",
] as const;
type Field = (typeof FIELDS)[number];
type FieldErrors = Partial<Record<Field, string>>;

/** The body of a change: only the fields that differ from the saved record. */
function changes(
  record: Action,
  input: {
    title: string;
    description: string | null;
    owner: string;
    workstream: string | null;
    start: string | null;
    due: string | null;
    milestone: string | null;
    status: ActionStatus;
  },
) {
  return {
    ...(input.title !== record.title && { title: input.title }),
    ...(input.description !== (record.description ?? null) && { description: input.description }),
    ...(input.owner !== record.owner_user_id && { owner_user_id: input.owner }),
    ...(input.workstream !== (record.workstream_id ?? null) && { workstream_id: input.workstream }),
    ...(input.start !== (record.start_date ?? null) && { start_date: input.start }),
    ...(input.due !== (record.due_date ?? null) && { due_date: input.due }),
    ...(input.milestone !== (record.milestone_id ?? null) && { milestone_id: input.milestone }),
    ...(input.status !== record.status && { status: input.status }),
  };
}

/**
 * „Aufgabe erfassen“ and the change of an action. The page owns the live regions, so the form
 * reports the result: `onSaved` for a saved action, `onFailed` for any other failure.
 */
export function ActionForm({
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
  /** The member who is signed in: the owner of a new action. */
  userId: string;
  /** The action to change. Without it, the form creates an action. */
  record?: Action;
  onSaved: (action: Action) => void;
  onFailed: (failure: SaveFailure) => void;
  onStart: () => void;
  onCancel?: () => void;
}) {
  const [title, setTitle] = useState(record?.title ?? "");
  const [description, setDescription] = useState(record?.description ?? "");
  const [owner, setOwner] = useState(record?.owner_user_id ?? userId);
  const [workstream, setWorkstream] = useState(record?.workstream_id ?? NO_WORKSTREAM);
  const [start, setStart] = useState(record?.start_date ?? "");
  const [due, setDue] = useState(record?.due_date ?? "");
  const [milestone, setMilestone] = useState(record?.milestone_id ?? NO_MILESTONE);
  const [status, setStatus] = useState<ActionStatus>(record?.status ?? "open");
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
    if (title.trim() === "") {
      setErrors({ title: t("work-error-title") });
      focusInvalidField();
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      const input = {
        title: title.trim(),
        description: blankToNull(description),
        owner,
        workstream: workstream === NO_WORKSTREAM ? null : workstream,
        start: blankToNull(start),
        due: blankToNull(due),
        milestone: milestone === NO_MILESTONE ? null : milestone,
        status,
      };
      const result = record
        ? await api.PATCH("/api/v1/events/{event_id}/actions/{id}", {
            params: { path: { ...path, id: record.id } },
            body: { ...changes(record, input), expected_version: record.version },
          })
        : await api.POST("/api/v1/events/{event_id}/actions", {
            params: { path },
            body: {
              title: input.title,
              owner_user_id: owner,
              ...(input.description !== null && { description: input.description }),
              ...(input.workstream !== null && { workstream_id: input.workstream }),
              ...(input.start !== null && { start_date: input.start }),
              ...(input.due !== null && { due_date: input.due }),
              ...(input.milestone !== null && { milestone_id: input.milestone }),
            },
          });
      if (result.data) {
        if (!record) {
          setTitle("");
          setDescription("");
          setStart("");
          setDue("");
          setWorkstream(NO_WORKSTREAM);
          setMilestone(NO_MILESTONE);
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
        label={t("action-field-title")}
        value={title}
        onChange={setTitle}
        error={errors.title}
        autoComplete="off"
        isRequired
      />
      <TextField
        label={t("action-field-description")}
        value={description}
        onChange={setDescription}
        error={errors.description}
        rows={3}
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
      <TextField
        label={t("work-field-start")}
        type="date"
        value={start}
        onChange={setStart}
        error={errors.start_date}
      />
      <TextField
        label={t("work-field-due")}
        type="date"
        value={due}
        onChange={setDue}
        error={errors.due}
      />
      <MilestoneSelect
        milestones={directory.milestones}
        value={milestone}
        current={record?.milestone_id}
        onChange={setMilestone}
        error={errors.milestone}
      />
      {record && (
        <Select
          label={t("work-field-status")}
          options={[record.status, ...record.next_statuses].map((s) => ({
            id: s,
            label: t(`action-status-${s}`),
          }))}
          value={status}
          onChange={(id) => setStatus(id as ActionStatus)}
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
