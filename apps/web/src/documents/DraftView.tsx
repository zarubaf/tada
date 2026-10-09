import { type ReactNode, useCallback, useId, useMemo, useRef, useState } from "react";
import type { Api, DraftRendering, Fact, Field, LinkTarget } from "../api/client";
import { type ProfileState, useEventProfile } from "../events/eventContext";
import { EvidencePanel } from "../evidence/EvidencePanel";
import { Excerpt } from "../evidence/Excerpt";
import { formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { useFocusAfterCommit } from "../ui/focus";
import { KnowledgeState } from "../ui/KnowledgeState";
import { LazyMarkdown } from "../ui/LazyMarkdown";
import { Skeleton } from "../ui/Skeleton";
import styles from "./DraftView.module.css";
import { sourceNumbers } from "./draftLinks";

/** Where a draft is read: the event, for its facts and their time zone. */
export interface DraftEnvironment {
  api: Api;
  eventId: string;
  /** The IANA time zone of the event, for the times of the evidence. */
  timeZone: string;
}

export interface DraftViewProps extends DraftEnvironment {
  draft: DraftRendering;
  /**
   * The level of a `#` heading of the draft, and of „Quellen“: one below the heading that the
   * draft sits under on the page.
   */
  headingLevel?: 2 | 3 | 4 | 5 | 6;
  /** Called when the text is in the page. */
  onShown?: () => void;
  /** The facts and fields of the event: the value types come from the fields. */
  profile: ProfileState;
}

type FactTarget = Extract<LinkTarget, { kind: "fact" }>;

/** The value that the evidence sheet shows. */
interface Shown {
  href: string;
  fact: Fact;
  label: string;
  valueText: string;
}

/** The current fact and its field for a cited fact, as far as the profile knows them. */
function lookup(profile: ProfileState, factId: string): { fact?: Fact; field?: Field } {
  if (profile.kind !== "loaded") {
    return {};
  }
  const fact = profile.profile.facts.find((candidate) => candidate.id === factId);
  const field = fact && profile.fields.find((candidate) => candidate.id === fact.field_id);
  return { ...(fact && { fact }), ...(field && { field }) };
}

function Removed() {
  return <span className={styles.removed}>{t("draft-removed")}</span>;
}

/**
 * A cited fact: the value of the cited version with its state of knowledge. If the cited version is
 * the current one, a press opens its evidence. An older version has no evidence here: the fact has
 * a newer version since, and the note says so.
 */
function FactLink({
  href,
  target,
  profile,
  onOpen,
  buttons,
}: {
  href: string;
  target: FactTarget;
  profile: ProfileState;
  onOpen: (shown: Shown) => void;
  buttons: React.RefObject<Map<string, HTMLButtonElement>>;
}) {
  const { fact, field } = lookup(profile, target.fact_id);
  const valueText = target.state === "unknown" ? undefined : formatValue(target, field?.value_type);
  const mark = <KnowledgeState state={target.state}>{valueText}</KnowledgeState>;
  if (!fact) {
    return mark;
  }
  if (fact.version !== target.version) {
    return (
      <>
        {mark}{" "}
        <span className={styles.note}>{t("draft-fact-older", { version: target.version })}</span>
      </>
    );
  }
  const label = field ? formatLabel(field.label) : fact.field_key;
  // The name holds the visible words: the value and, for a state other than accepted, its label.
  const visible = [valueText, target.state !== "accepted" && t(`knowledge-${target.state}`)]
    .filter(Boolean)
    .join(" ");
  return (
    <Button
      ref={(button) => {
        if (button) {
          buttons.current.set(href, button);
        } else {
          buttons.current.delete(href);
        }
      }}
      aria-label={t("draft-fact-evidence", { value: visible, label })}
      onPress={() => onOpen({ href, fact, label, valueText: valueText ?? t("knowledge-unknown") })}
    >
      {mark}
    </Button>
  );
}

/**
 * A draft as the reader sees it (ADR 0051, ADR 0058). Each `tada:` link resolves from the map of
 * the server and nowhere else: a fact shows its formatted value with its state of knowledge, a
 * source shows its words with a number, and a target that the reader cannot see shows „entfernt“.
 * The sources of the numbers follow the text.
 */
export function DraftView({
  api,
  eventId,
  timeZone,
  draft,
  profile,
  headingLevel = 2,
  onShown,
}: DraftViewProps) {
  const sourcesId = useId();
  const [shown, setShown] = useState<Shown>();
  // The evidence buttons, so that focus returns to the one that opened the sheet.
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const focusAfterCommit = useFocusAfterCommit();
  const numbers = useMemo(() => sourceNumbers(draft.markdown, draft.links), [draft]);

  const renderLink = useCallback(
    (href: string, words: ReactNode) => {
      const target = draft.links[href];
      switch (target?.kind) {
        case undefined:
        case "hidden":
          return <Removed />;
        case "fact":
          return (
            <FactLink
              href={href}
              target={target}
              profile={profile}
              onOpen={setShown}
              buttons={buttons}
            />
          );
        case "source": {
          const number = numbers.get(href);
          return (
            <>
              <span className={styles.cited}>{words}</span>
              {number !== undefined && (
                <sup className={styles.sourceMark}>
                  <span aria-hidden="true">[{number}]</span>
                  <span className={styles.hidden}>{t("draft-source-number", { number })}</span>
                </sup>
              )}
            </>
          );
        }
        default:
          // The list of kinds is open: a newer server can send a kind that this client lacks.
          return <span className={styles.note}>{t("value-unsupported")}</span>;
      }
    },
    [draft.links, numbers, profile],
  );

  if (profile.kind === "loading") {
    return <Skeleton />;
  }
  const Heading = `h${headingLevel}` as const;
  return (
    <div className={styles.draft}>
      <LazyMarkdown renderLink={renderLink} headingLevel={headingLevel} onShown={onShown}>
        {draft.markdown}
      </LazyMarkdown>
      {numbers.size > 0 && (
        <section className={styles.sources} aria-labelledby={sourcesId}>
          <Heading id={sourcesId} className={styles.heading}>
            {t("draft-sources")}
          </Heading>
          <ol className={styles.list}>
            {[...numbers].map(([href, number]) => {
              const target = draft.links[href];
              return target?.kind === "source" ? (
                <li key={href} value={number}>
                  <Excerpt quote={target.passage.quote} />
                  <p className={styles.note}>
                    {t("draft-source-version", { id: target.source_version_id.slice(0, 8) })}
                    {typeof target.passage.page === "number" &&
                      `, ${t("evidence-page", { page: target.passage.page })}`}
                  </p>
                </li>
              ) : null;
            })}
          </ol>
        </section>
      )}
      {shown && (
        <EvidencePanel
          api={api}
          eventId={eventId}
          timeZone={timeZone}
          title={shown.label}
          valueText={shown.valueText}
          fact={shown.fact}
          onClose={() => {
            setShown(undefined);
            focusAfterCommit(() => buttons.current.get(shown.href));
          }}
        />
      )}
    </div>
  );
}

/** A draft proposal in the Review Inbox: it loads the facts of its event itself. */
export function ProposalDraft({
  environment,
  draft,
}: {
  environment: DraftEnvironment;
  draft: DraftRendering;
}) {
  const { profile } = useEventProfile(environment.api, environment.eventId);
  // The card of the proposal has a title at h3 and sections at h4.
  return <DraftView {...environment} draft={draft} profile={profile} headingLevel={5} />;
}
