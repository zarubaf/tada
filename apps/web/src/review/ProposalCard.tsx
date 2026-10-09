import { IconAlertTriangle, IconClockExclamation } from "@tabler/icons-react";
import { type ReactNode, type Ref, useId } from "react";
import type { ApplyEdit, Proposal } from "../api/client";
import type { DraftEnvironment } from "../documents/DraftView";
import { Excerpt } from "../evidence/Excerpt";
import { isEditable } from "../facts/valueDraft";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { KnowledgeState } from "../ui/KnowledgeState";
import type { Conflict } from "./conflict";
import { EditForm } from "./EditForm";
import { type FieldInfo, OperationDetails, operationTitle } from "./OperationView";
import styles from "./ProposalCard.module.css";
import type { RecordNames } from "./recordNames";

export interface ProposalCardProps {
  proposal: Proposal;
  fields: Map<string, FieldInfo>;
  /** Where a draft of the proposal is read. */
  draftEnvironment: DraftEnvironment | undefined;
  conflict: Conflict | undefined;
  /** The names behind the IDs of a work proposal. */
  names: RecordNames;
  /** The existing record that the member chose instead of a new one, or nothing. */
  linkedRecord: string | undefined;
  onLink: (recordId: string | undefined) => void;
  /** The titles of the proposals this one needs. */
  needs: string[];
  /** The titles of the selected proposals that need this one. */
  neededBy: string[];
  selected: boolean;
  onSelect: (selected: boolean) => void;
  /** The shortcuts `A`, `E` and `R` act on the active proposal. */
  active: boolean;
  onActivate: () => void;
  /** Makes „Annehmen“ the one primary button of the view. */
  primaryAccept: boolean;
  /** A request runs: the buttons keep focus and ignore presses. */
  isPending: boolean;
  isEditing: boolean;
  onAccept: () => void;
  onEdit: () => void;
  onReject: () => void;
  onEditSubmit: (state: ApplyEdit["state"]) => void;
  onEditCancel: () => void;
  /** For the focus that returns to „Bearbeiten und annehmen“ when the form closes. */
  editButtonRef: Ref<HTMLButtonElement>;
}

/** The value type of the value that the reviewer can edit, or nothing. */
export function editableField(
  proposal: Proposal,
  fields: Map<string, FieldInfo>,
): FieldInfo | undefined {
  const { operation } = proposal;
  if (operation.kind !== "set-fact" || operation.state === "unknown") {
    return undefined;
  }
  const field = fields.get(operation.field_id);
  return field && isEditable(field.valueType) ? field : undefined;
}

function Hint({ show, children }: { show: boolean; children: string }) {
  return show ? <kbd className={styles.hint}>{children}</kbd> : null;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className={styles.section} aria-labelledby={id}>
      <h4 id={id} className={styles.sectionTitle}>
        {title}
      </h4>
      {children}
    </section>
  );
}

/**
 * One proposal of a changeset, from top to bottom: the change as a comparison, the source with the
 * cited passage, the reason and the assumption, and the actions (doc/design/components.md, „Review
 * Inbox“). A conflict disables „Annehmen“ and names the reason.
 */
export function ProposalCard(props: ProposalCardProps) {
  const { proposal, fields, conflict, needs, neededBy, selected, active, isPending, isEditing } =
    props;
  const duplicates = proposal.duplicates ?? [];
  const titleId = useId();
  const conflictId = useId();
  const unavailableId = useId();
  const title = operationTitle(proposal.operation, fields);
  // The member reviews an open proposal only when the routing gives it to them (ADR 0067).
  const isOpen = proposal.status === "open" && proposal.routed_to_me;
  const elsewhere = proposal.status === "open" && !proposal.routed_to_me;
  const field = editableField(proposal, fields);
  const isFact = proposal.operation.kind === "set-fact";
  const conflicting = isOpen && conflict !== undefined;
  const state = proposal.operation.kind === "set-fact" ? proposal.operation.state : "accepted";

  return (
    // The focus of a control inside makes the proposal the active one, for the shortcuts.
    <article
      className={styles.card}
      aria-labelledby={titleId}
      data-active={(isOpen && active) || undefined}
      data-conflict={conflicting || undefined}
      onFocusCapture={props.onActivate}
      onClickCapture={props.onActivate}
    >
      <header className={styles.header}>
        {isOpen && (
          <Checkbox
            label={t("inbox-select", { title })}
            isSelected={selected}
            onChange={props.onSelect}
            isDisabled={conflicting}
            labelHidden
          />
        )}
        <h3 id={titleId} className={styles.title}>
          {title}
        </h3>
        {proposal.status === "open" && proposal.overdue && (
          <span className={styles.overdue}>
            <IconClockExclamation size={16} stroke={1.5} aria-hidden="true" />
            {t("inbox-overdue")}
          </span>
        )}
        {elsewhere && <span className={styles.status}>{t("inbox-other-review")}</span>}
        {proposal.status !== "open" && (
          <span className={styles.status}>
            {proposal.status === "conflict" ? (
              <KnowledgeState state="conflict" />
            ) : (
              t(`inbox-status-${proposal.status}`)
            )}
          </span>
        )}
      </header>

      {conflict && (
        <p id={conflictId} className={styles.conflict}>
          <IconAlertTriangle size={16} stroke={1.5} aria-hidden="true" />
          <span>{t(`inbox-conflict-${conflict}`)}</span>
        </p>
      )}

      <Section title={t("inbox-section-change")}>
        <OperationDetails
          proposal={proposal}
          fields={fields}
          draftEnvironment={props.draftEnvironment}
          names={props.names}
        />
        {isOpen && duplicates.length > 0 && (
          <fieldset className={styles.duplicates}>
            <legend>{t("inbox-duplicates")}</legend>
            {duplicates.map((duplicate) => (
              <Checkbox
                key={duplicate.id}
                label={t("inbox-use-existing", {
                  name: duplicate.name,
                  id: duplicate.local_id,
                })}
                isSelected={props.linkedRecord === duplicate.id}
                onChange={(on) => props.onLink(on ? duplicate.id : undefined)}
              />
            ))}
          </fieldset>
        )}
        {needs.length > 0 && (
          <p className={styles.note}>{t("inbox-depends-on", { titles: needs.join(", ") })}</p>
        )}
        {selected && neededBy.length > 0 && (
          <p className={styles.note}>{t("inbox-needed-by", { titles: neededBy.join(", ") })}</p>
        )}
      </Section>

      <Section title={t("inbox-section-source")}>
        {proposal.evidence.length === 0 ? (
          <p>{t("inbox-source-none")}</p>
        ) : (
          <ul className={styles.sources}>
            {proposal.evidence.map((evidence) => (
              <li
                key={`${evidence.source_version_id}:${evidence.passage.start}:${evidence.passage.end}`}
                className={styles.source}
              >
                <Excerpt
                  before={evidence.excerpt.before}
                  quote={evidence.excerpt.quote}
                  after={evidence.excerpt.after}
                />
                <p className={styles.note}>
                  {t("inbox-source-version", { id: evidence.source_version_id.slice(0, 8) })}
                  {typeof evidence.passage.page === "number" &&
                    `, ${t("inbox-source-page", { page: evidence.passage.page })}`}
                </p>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t("inbox-section-reason")}>
        <p>{proposal.reason}</p>
        {state === "assumption" && <p className={styles.note}>{t("inbox-assumption-note")}</p>}
        {state === "unknown" && <p className={styles.note}>{t("inbox-unknown-note")}</p>}
        {proposal.operation.kind === "set-fact" && proposal.operation.approximate && (
          <p className={styles.note}>{t("inbox-approximate-note")}</p>
        )}
      </Section>

      {isOpen && isEditing && field && (
        <EditForm
          proposal={proposal}
          valueType={field.valueType}
          isPending={isPending}
          onSubmit={props.onEditSubmit}
          onCancel={props.onEditCancel}
        />
      )}

      {isOpen && (
        <div className={styles.actions}>
          <Button
            variant={props.primaryAccept ? "primary" : "secondary"}
            isPending={isPending}
            isDisabled={conflicting}
            aria-label={t("inbox-accept")}
            aria-describedby={conflicting ? conflictId : undefined}
            onPress={props.onAccept}
          >
            {t("inbox-accept")}
            <Hint show={active}>A</Hint>
          </Button>
          {isFact && (
            <Button
              ref={props.editButtonRef}
              aria-label={t("inbox-edit")}
              isDisabled={conflicting || !field}
              aria-describedby={conflicting ? conflictId : field ? undefined : unavailableId}
              onPress={props.onEdit}
            >
              {t("inbox-edit")}
              <Hint show={active}>E</Hint>
            </Button>
          )}
          <Button
            variant="danger"
            isPending={isPending}
            aria-label={t("inbox-reject")}
            onPress={props.onReject}
          >
            {t("inbox-reject")}
            <Hint show={active}>R</Hint>
          </Button>
          {isFact && !field && !conflicting && (
            <p id={unavailableId} className={styles.note}>
              {t("inbox-edit-unavailable")}
            </p>
          )}
        </div>
      )}
    </article>
  );
}
