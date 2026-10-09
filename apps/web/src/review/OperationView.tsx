// How the Review Inbox shows the operation of a proposal: a title and the change itself. A value
// goes through `formatValue`, so that the inbox and the event overview read the same (ADR 0049).

import type { Changeset, Field, Operation, Proposal, ValueType } from "../api/client";
import { type DraftEnvironment, ProposalDraft } from "../documents/DraftView";
import { LintWarnings } from "../documents/LintWarnings";
import { formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { KnowledgeState } from "../ui/KnowledgeState";
import styles from "./OperationView.module.css";

/** What the inbox needs to know about a field to show and edit a value of it. */
export interface FieldInfo {
  label: string;
  valueType: ValueType;
}

/**
 * The fields by ID: the catalog of the event and the fields that the changeset adds, because a
 * proposal can set a fact of a field that its own changeset defines.
 */
export function fieldInfos(changeset: Changeset, catalog: Field[]): Map<string, FieldInfo> {
  const fields = new Map<string, FieldInfo>(
    catalog.map((field) => [
      field.id,
      { label: formatLabel(field.label), valueType: field.value_type },
    ]),
  );
  for (const { operation } of changeset.proposals) {
    if (operation.kind === "add-field-definition") {
      fields.set(operation.id, {
        label: formatLabel(operation.label),
        valueType: operation.value_type,
      });
    }
  }
  return fields;
}

function fieldLabel(fieldId: string, fields: Map<string, FieldInfo>): string {
  return fields.get(fieldId)?.label ?? t("inbox-field-unknown");
}

/** A short title that names what the proposal does. */
export function operationTitle(operation: Operation, fields: Map<string, FieldInfo>): string {
  switch (operation.kind) {
    case "create-event":
      return t("inbox-op-create-event");
    case "set-fact":
      return t("inbox-op-set-fact", { label: fieldLabel(operation.field_id, fields) });
    case "add-field-definition":
      return t("inbox-op-add-field", { label: formatLabel(operation.label) });
    case "add-choice-value":
      return t("inbox-op-add-choice", { label: formatLabel(operation.label) });
    case "deprecate-field":
      return t("inbox-op-deprecate-field", { label: fieldLabel(operation.field_id, fields) });
    case "create-open-question":
      return t("inbox-op-create-question");
    case "create-document-draft":
      return operation.document.kind === "new"
        ? t("inbox-op-create-draft", { name: operation.document.name })
        : t("inbox-op-create-draft-existing");
    default:
      // The list of operations is open: a newer server can send a kind that this client lacks.
      return t("inbox-op-unknown");
  }
}

function Rows({ rows }: { rows: [string, string][] }) {
  return (
    <dl className={styles.rows}>
      {rows.map(([label, value]) => (
        <div key={label} className={styles.row}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** The comparison of a fact: the current state and the proposed state side by side. */
function FactComparison({
  proposal,
  operation,
  valueType,
}: {
  proposal: Proposal;
  operation: Extract<Operation, { kind: "set-fact" }>;
  valueType: ValueType | undefined;
}) {
  const { current } = proposal;
  return (
    <dl className={styles.comparison}>
      <div className={styles.side}>
        <dt>{t("inbox-current")}</dt>
        <dd>
          {current ? (
            <KnowledgeState state={current.state}>
              {current.state === "unknown" ? undefined : formatValue(current, valueType)}
            </KnowledgeState>
          ) : (
            <KnowledgeState state="unknown" />
          )}
        </dd>
      </div>
      <div className={styles.side}>
        <dt>{t("inbox-proposed")}</dt>
        <dd className={styles.proposed}>
          <KnowledgeState state="proposed">
            {operation.state === "unknown" ? undefined : formatValue(operation, valueType)}
          </KnowledgeState>
          {operation.state !== "accepted" && <KnowledgeState state={operation.state} />}
        </dd>
      </div>
    </dl>
  );
}

/** The change of the proposal: for a fact the comparison, for the other operations their data. */
export function OperationDetails({
  proposal,
  fields,
  draftEnvironment,
}: {
  proposal: Proposal;
  fields: Map<string, FieldInfo>;
  /** Where a draft is read: its event. It is absent for a changeset of the organization. */
  draftEnvironment: DraftEnvironment | undefined;
}) {
  const { operation, draft } = proposal;
  switch (operation.kind) {
    case "set-fact":
      return (
        <FactComparison
          proposal={proposal}
          operation={operation}
          valueType={fields.get(operation.field_id)?.valueType}
        />
      );
    case "create-event":
      return (
        <Rows
          rows={[
            [t("inbox-row-key"), operation.key],
            [t("inbox-row-name"), operation.name],
            [t("inbox-row-time-zone"), operation.time_zone],
          ]}
        />
      );
    case "add-field-definition":
      return (
        <Rows
          rows={[
            [t("inbox-row-label"), formatLabel(operation.label)],
            [t("inbox-row-field-key"), operation.key],
            [t("inbox-row-value-type"), t(`inbox-value-type-${operation.value_type.type}`)],
            [t("inbox-row-description"), operation.description],
          ]}
        />
      );
    case "add-choice-value":
      return (
        <Rows
          rows={[
            [t("inbox-row-field"), fieldLabel(operation.field_id, fields)],
            [t("inbox-row-label"), formatLabel(operation.label)],
          ]}
        />
      );
    case "deprecate-field":
      return <Rows rows={[[t("inbox-row-field"), fieldLabel(operation.field_id, fields)]]} />;
    case "create-open-question":
      return <Rows rows={[[t("inbox-row-text"), operation.text]]} />;
    case "create-document-draft":
      return (
        <div className={styles.draft}>
          {operation.document.kind === "new" ? (
            <p>{t("inbox-draft-new")}</p>
          ) : (
            <>
              <p>{t("inbox-draft-existing")}</p>
              <Link to={`/documents/${encodeURIComponent(operation.document.document_id)}`}>
                {t("inbox-draft-open")}
              </Link>
            </>
          )}
          {draft && <LintWarnings warnings={draft.lint_warnings} />}
          {draft && draftEnvironment && (
            <ProposalDraft environment={draftEnvironment} draft={draft} />
          )}
        </div>
      );
    default:
      return <p>{t("value-unsupported")}</p>;
  }
}
