import { useEffect, useState } from "react";
import type { Api, Author, Fact, FactEvidence } from "../api/client";
import { loadOrganizationMembers } from "../events/eventMembers";
import { formatDateTime } from "../facts/formatValue";
import { t } from "../i18n";
import { KnowledgeState } from "../ui/KnowledgeState";
import { Sheet } from "../ui/Sheet";
import { authorKindName } from "./authorKind";
import styles from "./EvidencePanel.module.css";
import { Excerpt } from "./Excerpt";

export interface EvidencePanelProps {
  api: Api;
  eventId: string;
  /** The IANA time zone of the event, for the times. */
  timeZone: string;
  /** The label of the field. */
  title: string;
  /** The value as `formatValue` shows it. */
  valueText: string;
  fact: Fact;
  onClose: () => void;
}

/** The names that the panel looks up. A name that fails to load stays absent. */
interface Lookups {
  members: Map<string, string>;
  /** The document and its version number for each source version. */
  sources: Map<string, { name: string; number: number }>;
}

/** Loads the member names and the documents of the event once, when the panel opens. */
function useLookups(api: Api, eventId: string): Lookups {
  const [lookups, setLookups] = useState<Lookups>({ members: new Map(), sources: new Map() });
  useEffect(() => {
    let current = true;
    void (async () => {
      const [members, documents] = await Promise.all([
        loadOrganizationMembers(api).catch(() => undefined),
        api
          .GET("/api/v1/events/{event_id}/documents", {
            params: { path: { event_id: eventId }, query: { limit: 200 } },
          })
          .catch(() => undefined),
      ]);
      if (!current) {
        return;
      }
      const sources = new Map<string, { name: string; number: number }>();
      for (const document of documents?.data?.items ?? []) {
        const { source_version_id, number } = document.newest_version;
        if (source_version_id) {
          sources.set(source_version_id, { name: document.name, number });
        }
      }
      setLookups({
        members: new Map(
          members && "members" in members
            ? members.members.map((member) => [member.user_id, member.display_name])
            : [],
        ),
        sources,
      });
    })();
    return () => {
      current = false;
    };
  }, [api, eventId]);
  return lookups;
}

function authorName(author: Author, members: Map<string, string>): string {
  if (author.kind === "member") {
    return members.get(author.id) ?? t("evidence-author-unknown");
  }
  return authorKindName(author.kind);
}

function Source({
  evidence,
  lookups,
  timeZone,
}: {
  evidence: FactEvidence;
  lookups: Lookups;
  timeZone: string;
}) {
  const { passage } = evidence;
  const source = lookups.sources.get(evidence.source_version_id);
  return (
    <li className={styles.source}>
      <Excerpt quote={passage.quote} />
      <p className={styles.meta}>
        {source
          ? t("evidence-document-version", { name: source.name, number: source.number })
          : t("evidence-source-version", { id: evidence.source_version_id.slice(0, 8) })}
      </p>
      {typeof passage.page === "number" && (
        <p className={styles.meta}>{t("evidence-page", { page: passage.page })}</p>
      )}
      <p className={styles.meta}>
        <time dateTime={evidence.captured_at}>
          {t("evidence-captured", { time: formatDateTime(evidence.captured_at, timeZone) })}
        </time>
      </p>
    </li>
  );
}

/**
 * The provenance of one value: its state, the passages of the sources with the source version and
 * the capture time, and who accepted the value and when (doc/design/components.md). It is a sheet.
 * It is open while it is mounted.
 */
export function EvidencePanel({
  api,
  eventId,
  timeZone,
  title,
  valueText,
  fact,
  onClose,
}: EvidencePanelProps) {
  const lookups = useLookups(api, eventId);
  return (
    <Sheet title={title} onClose={onClose}>
      <p className={styles.value}>
        <KnowledgeState state={fact.state} showLabel>
          {fact.state === "unknown" ? undefined : valueText}
        </KnowledgeState>
      </p>
      <dl className={styles.accepted}>
        <div>
          <dt>{t("evidence-accepted-by")}</dt>
          <dd>{authorName(fact.accepted_by, lookups.members)}</dd>
        </div>
        <div>
          <dt>{t("evidence-accepted-at")}</dt>
          <dd>
            <time dateTime={fact.accepted_at}>{formatDateTime(fact.accepted_at, timeZone)}</time>
          </dd>
        </div>
      </dl>
      <section className={styles.sources} aria-labelledby="evidence-sources">
        <h3 id="evidence-sources" className={styles.heading}>
          {t("evidence-sources")}
        </h3>
        {fact.evidence.length === 0 ? (
          <p>{t("evidence-none")}</p>
        ) : (
          <ul className={styles.list}>
            {fact.evidence.map((evidence) => (
              <Source
                key={`${evidence.source_version_id}:${evidence.passage.start}:${evidence.passage.end}`}
                evidence={evidence}
                lookups={lookups}
                timeZone={timeZone}
              />
            ))}
          </ul>
        )}
      </section>
    </Sheet>
  );
}
