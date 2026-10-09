import { type RefObject, useCallback, useEffect, useRef, useState } from "react";
import {
  type Api,
  type Document,
  type DocumentVersion,
  type LineChange,
  problemMessage,
  type VersionDiff,
} from "../api/client";
import { EventPage } from "../events/EventPage";
import { type ProfileState, useEventContext } from "../events/eventContext";
import { formatLabel } from "../facts/formatValue";
import { t } from "../i18n";
import { Link, useParams } from "../router/Router";
import { type Column, DataTable } from "../ui/DataTable";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Page } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./DocumentDiffPage.module.css";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | {
      kind: "loaded";
      document: Document;
      versions: DocumentVersion[];
      /** Absent when the address names no two versions. */
      diff: VersionDiff | undefined;
      from: string | undefined;
      to: string | undefined;
    };

/** The two versions of the address: `?from=<version ID>&to=<version ID>`. */
function requestedVersions(): { from: string | undefined; to: string | undefined } {
  const query = new URLSearchParams(window.location.search);
  return { from: query.get("from") || undefined, to: query.get("to") || undefined };
}

/** The label of the field of a fact, as far as the profile knows it. */
function factLabel(profile: ProfileState, factId: string): string {
  if (profile.kind !== "loaded") {
    return t("document-diff-fact-unknown");
  }
  const fact = profile.profile.facts.find((candidate) => candidate.id === factId);
  const field = fact && profile.fields.find((candidate) => candidate.id === fact.field_id);
  return field ? formatLabel(field.label) : t("document-diff-fact-unknown");
}

/** The change of a line in words. Color and the marker only support them. */
function changeText(kind: string): string {
  switch (kind) {
    case "added":
      return t("document-diff-added");
    case "removed":
      return t("document-diff-removed");
    default:
      return t("document-diff-unchanged");
  }
}

const MARKERS: Record<string, string> = { added: "+", removed: "−" };

const columns: Column<LineChange>[] = [
  {
    id: "change",
    header: t("document-diff-column-change"),
    cell: (line) => (
      <>
        <span aria-hidden="true">{MARKERS[line.kind] ?? ""}</span>
        <span className={styles.words}>{changeText(line.kind)}</span>
      </>
    ),
  },
  {
    id: "old",
    header: t("document-diff-column-old"),
    cell: (line) => line.old_line ?? "",
    numeric: true,
  },
  {
    id: "new",
    header: t("document-diff-column-new"),
    cell: (line) => line.new_line ?? "",
    numeric: true,
  },
  {
    id: "text",
    header: t("document-diff-column-text"),
    cell: (line) => (
      <span className={styles.text} data-kind={line.kind}>
        {line.text}
      </span>
    ),
  },
];

/** The facts that the two drafts cite, and the lines that differ. It sits in the layout of the event. */
function DiffBody({
  document,
  versions,
  diff,
  from,
  to,
  heading,
}: {
  document: Document;
  versions: DocumentVersion[];
  diff: VersionDiff | undefined;
  from: string | undefined;
  to: string | undefined;
  heading: RefObject<HTMLHeadingElement | null>;
}) {
  const { profile } = useEventContext();
  const focusAfterCommit = useFocusAfterCommit();
  // The page arrived: focus goes to the heading.
  useEffect(() => focusAfterCommit(() => heading.current), [focusAfterCommit, heading]);
  const numberOf = (id: string | undefined) =>
    versions.find((version) => version.id === id)?.number ?? "";
  const facts = diff?.facts;
  const noFacts =
    facts !== undefined &&
    facts.changed.length === 0 &&
    facts.added.length === 0 &&
    facts.removed.length === 0;
  return (
    <>
      <Link to={`/documents/${encodeURIComponent(document.id)}`}>{t("document-diff-back")}</Link>
      <header className={styles.header}>
        <p className={styles.key}>{document.readable_id}</p>
        <h2 ref={heading} tabIndex={-1} className={styles.title}>
          {numberOf(from) !== "" && numberOf(to) !== ""
            ? t("document-diff-title", { from: numberOf(from), to: numberOf(to) })
            : t("document-diff-title-plain")}
        </h2>
      </header>
      {!diff ? (
        <p>{t("document-diff-missing")}</p>
      ) : (
        <>
          <section className={styles.section} aria-labelledby="document-diff-facts">
            <h3 id="document-diff-facts" className={styles.heading}>
              {t("document-diff-facts")}
            </h3>
            {noFacts ? (
              <p>{t("document-diff-facts-none")}</p>
            ) : (
              <ul className={styles.list}>
                {diff.facts.changed.map((change) => (
                  <li key={`changed:${change.fact_id}`}>
                    {t("document-diff-fact-changed", {
                      label: factLabel(profile, change.fact_id),
                      from: change.from,
                      to: change.to,
                    })}
                  </li>
                ))}
                {diff.facts.added.map((fact) => (
                  <li key={`added:${fact.fact_id}`}>
                    {t("document-diff-fact-added", {
                      label: factLabel(profile, fact.fact_id),
                      version: fact.version,
                    })}
                  </li>
                ))}
                {diff.facts.removed.map((fact) => (
                  <li key={`removed:${fact.fact_id}`}>
                    {t("document-diff-fact-removed", {
                      label: factLabel(profile, fact.fact_id),
                      version: fact.version,
                    })}
                  </li>
                ))}
              </ul>
            )}
          </section>
          <section className={styles.section} aria-labelledby="document-diff-lines">
            <h3 id="document-diff-lines" className={styles.heading}>
              {t("document-diff-lines")}
            </h3>
            {diff.lines.every((line) => line.kind === "unchanged") && (
              <p>{t("document-diff-lines-same")}</p>
            )}
            {diff.lines.length > 0 && (
              <DataTable
                label={t("document-diff-lines")}
                columns={columns}
                rows={diff.lines}
                rowKey={(line) => `${line.kind}:${line.old_line ?? ""}:${line.new_line ?? ""}`}
              />
            )}
          </section>
        </>
      )}
    </>
  );
}

/**
 * The comparison of two draft versions of a document (ADR 0051): the facts that they cite in
 * another version, and each line with its change. The address names the versions:
 * `/documents/:documentId/diff?from=<older version ID>&to=<newer version ID>`.
 */
export function DocumentDiffPage({ api }: { api: Api }) {
  const { documentId = "" } = useParams();
  const [state, setState] = useState<State>({ kind: "loading" });
  const latest = useRef(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  /** Resolves to true when the page loaded. */
  const load = useCallback(async () => {
    const request = ++latest.current;
    const done = (next: State) => {
      if (request === latest.current) {
        setState(next);
      }
      return next.kind === "loaded";
    };
    const { from, to } = requestedVersions();
    const path = { document_id: documentId };
    try {
      const [document, versions, diff] = await Promise.all([
        api.GET("/api/v1/documents/{document_id}", { params: { path } }),
        api.GET("/api/v1/documents/{document_id}/versions", { params: { path } }),
        from && to
          ? api.GET("/api/v1/documents/{document_id}/diff", {
              params: { path, query: { from, to } },
            })
          : undefined,
      ]);
      if (document.data && versions.data && (!diff || diff.data)) {
        return done({
          kind: "loaded",
          document: document.data,
          versions: versions.data.items,
          diff: diff?.data,
          from,
          to,
        });
      }
      const error = document.error ?? versions.error ?? diff?.error;
      return done({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      return done({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api, documentId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "loaded") {
    return (
      <EventPage api={api} eventId={state.document.event_id}>
        <DiffBody
          document={state.document}
          versions={state.versions}
          diff={state.diff}
          from={state.from}
          to={state.to}
          heading={heading}
        />
      </EventPage>
    );
  }
  return (
    <Page>
      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("document-loading")}>
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          announce={retried ? "focus" : "alert"}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
        />
      )}
    </Page>
  );
}
