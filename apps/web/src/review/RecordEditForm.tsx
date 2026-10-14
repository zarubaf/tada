// „Bearbeiten und annehmen“ for a proposal that creates an action, a commitment, a milestone, a
// person or an institution (ADR 0050): the fields that the reviewer may change, with the same
// pickers as the work registers. The form sends only the fields that differ from the proposal.

import { type FormEvent, useEffect, useRef, useState } from "react";
import type { Api, Proposal } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { firstInvalidField } from "../ui/focus";
import { TextField } from "../ui/TextField";
import { DirectoryGate } from "../work/DirectoryGate";
import { type Directory, useDirectory } from "../work/directory";
import {
  MilestoneSelect,
  NO_MILESTONE,
  NO_WORKSTREAM,
  OwnerSelect,
  WorkstreamSelect,
} from "../work/fields";
import styles from "./EditForm.module.css";
import { DecisionFixedRows } from "./OperationView";
import {
  type FieldErrors,
  type FieldName,
  type Fields,
  message,
  type RecordOperation,
} from "./recordEdit";
import { NO_NAMES, type RecordNames } from "./recordNames";

interface Values {
  title: string;
  description: string;
  text: string;
  condition: string;
  name: string;
  email: string;
  phone: string;
  start_date: string;
  due_date: string;
  date: string;
  owner: string;
  workstream: string;
  milestone: string;
}

function initialValues(operation: RecordOperation): Values {
  const base: Values = {
    title: "",
    description: "",
    text: "",
    condition: "",
    name: "",
    email: "",
    phone: "",
    start_date: "",
    due_date: "",
    date: "",
    owner: "",
    workstream: NO_WORKSTREAM,
    milestone: NO_MILESTONE,
  };
  switch (operation.kind) {
    case "create-action":
      return {
        ...base,
        title: operation.title,
        description: operation.description ?? "",
        owner: operation.owner,
        workstream: operation.workstream ?? NO_WORKSTREAM,
        start_date: operation.start_date ?? "",
        due_date: operation.due_date ?? "",
        milestone: operation.milestone ?? NO_MILESTONE,
      };
    case "create-milestone":
      return {
        ...base,
        title: operation.title,
        description: operation.description ?? "",
        date: operation.date,
        owner: operation.owner,
        workstream: operation.workstream ?? NO_WORKSTREAM,
      };
    case "create-decision":
      return {
        ...base,
        owner: operation.owner,
        workstream: operation.workstream ?? NO_WORKSTREAM,
      };
    case "create-commitment":
      return {
        ...base,
        text: operation.text,
        condition: operation.condition ?? "",
        owner: operation.owner,
        workstream: operation.workstream ?? NO_WORKSTREAM,
        due_date: operation.due_date ?? "",
      };
    default:
      return {
        ...base,
        name: operation.name,
        email: operation.email ?? "",
        phone: operation.phone ?? "",
      };
  }
}

const blankToNull = (text: string) => (text.trim() === "" ? null : text.trim());

/** The fields that differ from the proposal. A condition stays: only „make firm“ ends it. */
function changedFields(
  operation: RecordOperation,
  values: Values,
): { fields: Fields; errors: FieldErrors } {
  const start = initialValues(operation);
  const fields: Fields = {};
  const errors: FieldErrors = {};
  const required = ["title", "text", "name", "date"] as const;
  const optional = ["description", "email", "phone", "start_date", "due_date"] as const;
  for (const field of required) {
    if (values[field].trim() !== start[field]) {
      fields[field] = values[field].trim();
    }
  }
  for (const field of optional) {
    if (values[field].trim() !== start[field]) {
      fields[field] = blankToNull(values[field]);
    }
  }
  if (values.owner !== start.owner) {
    fields.owner = values.owner;
  }
  if (values.workstream !== start.workstream) {
    fields.workstream = values.workstream === NO_WORKSTREAM ? null : values.workstream;
  }
  if (values.milestone !== start.milestone) {
    fields.milestone = values.milestone === NO_MILESTONE ? null : values.milestone;
  }
  if (values.condition.trim() !== start.condition) {
    if (values.condition.trim() === "") {
      errors.condition = message("condition", "condition-fixed");
    } else {
      fields.condition = values.condition.trim();
    }
  }
  return { fields, errors };
}

export interface RecordEditFormProps {
  api: Api;
  /** The event of the proposal, for the owners and the workstreams. */
  eventId: string | null | undefined;
  proposal: Proposal;
  isPending: boolean;
  /** The fields that the server refused in the last apply. */
  serverErrors: FieldErrors;
  /** The names behind the IDs of the changeset, for a workstream or a milestone that the changeset creates. */
  names?: RecordNames;
  onSubmit: (fields: Fields) => void;
  onCancel: () => void;
}

function Form({
  proposal,
  names = NO_NAMES,
  directory,
  isPending,
  serverErrors,
  onSubmit,
  onCancel,
}: Omit<RecordEditFormProps, "api" | "eventId"> & { directory: Directory | undefined }) {
  const operation = proposal.operation as RecordOperation;
  const [values, setValues] = useState(() => initialValues(operation));
  const [clientErrors, setClientErrors] = useState<FieldErrors>({});
  const [attempts, setAttempts] = useState(0);
  const form = useRef<HTMLFormElement>(null);
  const errors = { ...serverErrors, ...clientErrors };
  const set = (field: keyof Values) => (value: string) =>
    setValues((current) => ({ ...current, [field]: value }));

  // When the form opens, focus moves to its first field.
  useEffect(() => {
    form.current?.querySelector<HTMLElement>("input, textarea, button")?.focus();
  }, []);

  // A failed submit or a refusal of the server moves focus to the first invalid field.
  useEffect(() => {
    if (attempts > 0 || Object.keys(serverErrors).length > 0) {
      firstInvalidField(form.current)?.focus();
    }
  }, [attempts, serverErrors]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const result = changedFields(operation, values);
    setClientErrors(result.errors);
    if (Object.keys(result.errors).length > 0) {
      setAttempts((count) => count + 1);
      return;
    }
    onSubmit(result.fields);
  };

  const text = (
    field: keyof Values,
    label: string,
    extra: { rows?: number; type?: string } = {},
  ) => (
    <TextField
      label={label}
      value={values[field]}
      onChange={set(field)}
      error={errors[field as FieldName]}
      autoComplete="off"
      {...extra}
    />
  );
  const owned =
    operation.kind === "create-action" ||
    operation.kind === "create-commitment" ||
    operation.kind === "create-milestone" ||
    operation.kind === "create-decision";

  return (
    <form ref={form} noValidate className={styles.form} onSubmit={submit}>
      <h4 className={styles.title}>{t("inbox-edit-title")}</h4>
      {(operation.kind === "create-action" || operation.kind === "create-milestone") && (
        <>
          {text("title", t("inbox-row-title"))}
          {text("description", t("inbox-row-description"), { rows: 3 })}
        </>
      )}
      {operation.kind === "create-decision" && (
        <>
          <DecisionFixedRows operation={operation} names={names} />
          <p className={styles.note}>{t("inbox-edit-decision-fixed")}</p>
        </>
      )}
      {operation.kind === "create-milestone" && text("date", t("inbox-row-date"), { type: "date" })}
      {operation.kind === "create-commitment" &&
        text("text", t("inbox-row-commitment"), { rows: 3 })}
      {operation.kind === "create-commitment" &&
        operation.condition &&
        text("condition", t("inbox-row-condition"), { rows: 2 })}
      {(operation.kind === "create-person" || operation.kind === "create-institution") && (
        <>
          {text("name", t("inbox-row-name"))}
          {text("email", t("inbox-row-email"))}
          {text("phone", t("inbox-row-phone"))}
        </>
      )}
      {owned && directory && (
        <>
          <OwnerSelect
            directory={directory}
            label={t("work-field-owner")}
            value={values.owner}
            current={operation.owner}
            onChange={set("owner")}
            error={errors.owner}
          />
          <WorkstreamSelect
            directory={directory}
            value={values.workstream}
            current={operation.workstream}
            currentLabel={names.workstream(operation.workstream ?? "")}
            onChange={set("workstream")}
            error={errors.workstream}
          />
        </>
      )}
      {operation.kind === "create-action" &&
        text("start_date", t("work-field-start"), { type: "date" })}
      {(operation.kind === "create-action" || operation.kind === "create-commitment") &&
        text("due_date", t("work-field-due"), { type: "date" })}
      {operation.kind === "create-action" && directory && (
        <MilestoneSelect
          milestones={directory.milestones}
          value={values.milestone}
          current={operation.milestone}
          currentName={operation.milestone ? names.record(operation.milestone) : undefined}
          onChange={set("milestone")}
          error={errors.milestone}
        />
      )}
      <div className={styles.actions}>
        <Button onPress={onCancel}>{t("inbox-edit-cancel")}</Button>
        <Button type="submit" variant="primary" isPending={isPending}>
          {t("inbox-edit-submit")}
        </Button>
      </div>
    </form>
  );
}

function WithDirectory(props: RecordEditFormProps & { eventId: string }) {
  // A new action can name a milestone of the event.
  const milestones = props.proposal.operation.kind === "create-action";
  const { state, reload } = useDirectory(props.api, props.eventId, { milestones });
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Form {...props} directory={directory} />}
    </DirectoryGate>
  );
}

/** The edit form of a proposal that creates a record. */
export function RecordEditForm(props: RecordEditFormProps) {
  const { operation } = props.proposal;
  if (
    (operation.kind === "create-action" ||
      operation.kind === "create-commitment" ||
      operation.kind === "create-milestone" ||
      operation.kind === "create-decision") &&
    props.eventId
  ) {
    return <WithDirectory {...props} eventId={props.eventId} />;
  }
  return <Form {...props} directory={undefined} />;
}
