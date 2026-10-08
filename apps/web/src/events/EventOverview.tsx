import { useRef, useState } from "react";
import type { Api, Fact, Field } from "../api/client";
import { EvidencePanel } from "../evidence/EvidencePanel";
import { formatDateTime, formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { KnowledgeState } from "../ui/KnowledgeState";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventOverview.module.css";
import { useEventContext } from "./eventContext";

/** One line of „Fakten“: a field of the event and its current fact, if it has one. */
interface Row {
  key: string;
  label: string;
  field: Field | undefined;
  fact: Fact | undefined;
}

/**
 * The rows of the facts: each active field of the catalog in its order, and each fact whose field
 * the catalog does not list. A field without a fact is a row too, so that it reads „Unbekannt“.
 */
function rowsOf(fields: Field[], facts: Fact[]): Row[] {
  const byField = new Map(facts.map((fact) => [fact.field_key, fact]));
  const rows: Row[] = fields
    .filter((field) => field.status === "active" || byField.has(field.key))
    .map((field) => ({
      key: field.key,
      label: formatLabel(field.label),
      field,
      fact: byField.get(field.key),
    }));
  const listed = new Set(fields.map((field) => field.key));
  for (const fact of facts) {
    if (!listed.has(fact.field_key)) {
      rows.push({ key: fact.field_key, label: fact.field_key, field: undefined, fact });
    }
  }
  return rows;
}

/**
 * „Übersicht“ of an event: the open questions and the facts with the state of knowledge of each
 * value (Overview template). Open proposals are not facts; they have a subsection of their own.
 * It reads the profile that `EventPage` loads.
 */
export function EventOverview({ api }: { api: Api }) {
  const { event, profile, reloadProfile } = useEventContext();
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);
  // The value whose evidence the sheet shows.
  const [shown, setShown] = useState<Row>();
  // The evidence buttons, so that focus returns to the one that opened the sheet.
  const buttons = useRef(new Map<string, HTMLElement>());
  const focusAfterCommit = useFocusAfterCommit();

  if (profile.kind === "loading") {
    return (
      <div className={styles.skeleton} role="status" aria-label={t("event-overview-loading")}>
        <Skeleton />
        <Skeleton />
        <Skeleton />
      </div>
    );
  }
  if (profile.kind === "failed") {
    return (
      <InlineError
        message={profile.message}
        requestId={profile.requestId}
        announce={retried ? "focus" : "alert"}
        onRetry={() => retry(reloadProfile)}
      />
    );
  }

  const { facts, proposals, open_questions } = profile.profile;
  const rows = rowsOf(profile.fields, facts);
  const fieldOf = (id: string) => profile.fields.find((field) => field.id === id);

  return (
    <div className={styles.overview}>
      <section className={styles.section} aria-labelledby="overview-questions">
        <h2 id="overview-questions" ref={heading} tabIndex={-1} className={styles.heading}>
          {t("event-overview-questions")}
        </h2>
        {open_questions.length === 0 ? (
          <p>{t("event-overview-questions-none")}</p>
        ) : (
          <ul className={styles.questions}>
            {open_questions.map((question) => (
              <li key={question.id}>
                <span className={styles.id}>{question.local_id}</span> {question.text}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className={styles.section} aria-labelledby="overview-facts">
        <h2 id="overview-facts" className={styles.heading}>
          {t("event-overview-facts")}
        </h2>
        <dl className={styles.facts}>
          {rows.map((row) => {
            const state = row.fact?.state ?? "unknown";
            return (
              <div key={row.key} className={styles.row}>
                <dt>{row.label}</dt>
                <dd>
                  <KnowledgeState state={state}>
                    {state === "unknown"
                      ? undefined
                      : formatValue(row.fact ?? {}, row.field?.value_type)}
                  </KnowledgeState>
                  {row.fact && (
                    <Button
                      ref={(button) => {
                        if (button) {
                          buttons.current.set(row.key, button);
                        } else {
                          buttons.current.delete(row.key);
                        }
                      }}
                      aria-label={t("event-overview-evidence-of", { label: row.label })}
                      onPress={() => setShown(row)}
                    >
                      {t("event-overview-evidence")}
                    </Button>
                  )}
                </dd>
              </div>
            );
          })}
        </dl>

        <section className={styles.proposals} aria-labelledby="overview-proposals">
          <h3 id="overview-proposals" className={styles.subheading}>
            {t("event-overview-proposals")}
          </h3>
          {proposals.length === 0 ? (
            <p>{t("event-overview-proposals-none")}</p>
          ) : (
            <>
              <p className={styles.note}>{t("event-overview-proposals-note")}</p>
              <ul className={styles.proposalList}>
                {proposals.map((proposal) => {
                  const field = fieldOf(proposal.field_id);
                  return (
                    <li key={proposal.id} className={styles.proposal}>
                      <span>{field ? formatLabel(field.label) : proposal.field_id}</span>
                      <span className={styles.proposed}>
                        <KnowledgeState state="proposed">
                          {proposal.state === "unknown"
                            ? undefined
                            : formatValue(proposal, field?.value_type)}
                        </KnowledgeState>
                        {proposal.state !== "accepted" && <KnowledgeState state={proposal.state} />}
                      </span>
                      <time className={styles.note} dateTime={proposal.created_at}>
                        {t("event-overview-proposed-at", {
                          time: formatDateTime(proposal.created_at, event.time_zone),
                        })}
                      </time>
                    </li>
                  );
                })}
              </ul>
            </>
          )}
        </section>
      </section>

      {shown?.fact && (
        <EvidencePanel
          api={api}
          eventId={event.id}
          timeZone={event.time_zone}
          title={shown.label}
          valueText={formatValue(shown.fact, shown.field?.value_type)}
          fact={shown.fact}
          onClose={() => {
            setShown(undefined);
            focusAfterCommit(() => buttons.current.get(shown.key));
          }}
        />
      )}
    </div>
  );
}
