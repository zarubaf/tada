import { type RefObject, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Api, type Document, type DocumentVersion, problemMessage } from "../api/client";
import { EventPage } from "../events/EventPage";
import { loadOrganizationMembers } from "../events/eventMembers";
import { LOCALE, t } from "../i18n";
import { Link, useParams } from "../router/Router";
import { type Column, DataTable } from "../ui/DataTable";
import { FileLink } from "../ui/FileLink";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { Page } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./DocumentPage.module.css";
import { formatSize, hashPrefix } from "./format";

const createdFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; document: Document; versions: DocumentVersion[] };

/**
 * The download address of a version. A plain link, not a call of the client: the browser streams
 * the file to the disk, and `inline` asks for the browser view (PDF and plain text only).
 */
function contentUrl(version: DocumentVersion, inline = false): string {
  const base = `/api/v1/document-versions/${encodeURIComponent(version.id)}/content`;
  return inline ? `${base}?disposition=inline` : base;
}

/** The server shows these types inline; every other type is a download only (ADR 0009). */
function previewKind(version: DocumentVersion | undefined): "pdf" | "text" | undefined {
  if (version?.kind !== "upload") {
    return undefined;
  }
  // The detected type can carry parameters, for example `text/plain; charset=utf-8`.
  const mediaType = version.media_type?.split(";")[0]?.trim().toLowerCase();
  if (mediaType === "application/pdf") {
    return "pdf";
  }
  return mediaType === "text/plain" ? "text" : undefined;
}

/** A version of a document: the file, or the draft status. */
function versionLabel(version: DocumentVersion): string {
  if (version.kind === "draft") {
    return t("document-draft-status", {
      status: t(`document-status-${version.status ?? "draft"}`),
    });
  }
  const size = typeof version.size_bytes === "number" ? ` · ${formatSize(version.size_bytes)}` : "";
  return `${version.file_name ?? ""}${size}`;
}

/** The page of one document: its versions, the download and the preview of the newest version. */
export function DocumentPage({ api }: { api: Api }) {
  const { documentId = "" } = useParams();
  const [state, setState] = useState<State>({ kind: "loading" });
  // The names of the uploaders. A member list that fails to load leaves the names unknown.
  const [names, setNames] = useState<Map<string, string>>(new Map());

  // The newest request: the answer of an older one is dropped.
  const latest = useRef(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);

  /** Resolves to true when the document loaded. */
  const load = useCallback(async () => {
    const path = { document_id: documentId };
    const request = ++latest.current;
    const failed = (message: string, requestId: string | undefined) => {
      if (request === latest.current) {
        setState({ kind: "failed", message, requestId });
      }
      return false;
    };
    try {
      const [document, versions] = await Promise.all([
        api.GET("/api/v1/documents/{document_id}", { params: { path } }),
        api.GET("/api/v1/documents/{document_id}/versions", { params: { path } }),
      ]);
      if (request !== latest.current) {
        return false;
      }
      if (document.data && versions.data) {
        setState({ kind: "loaded", document: document.data, versions: versions.data.items });
        return true;
      }
      const error = document.error ?? versions.error;
      return failed(problemMessage(error), error?.request_id);
    } catch {
      return failed(problemMessage(undefined), undefined);
    }
  }, [api, documentId]);

  useEffect(() => {
    void load();
  }, [load]);

  const loaded = state.kind === "loaded";
  useEffect(() => {
    if (!loaded) {
      return;
    }
    loadOrganizationMembers(api)
      .then((result) => {
        if ("members" in result) {
          setNames(new Map(result.members.map((m) => [m.user_id, m.display_name])));
        }
      })
      .catch(() => undefined);
  }, [api, loaded]);

  const columns: Column<DocumentVersion>[] = [
    {
      id: "number",
      header: t("document-column-number"),
      cell: (v) => v.number,
      numeric: true,
    },
    { id: "file", header: t("document-column-file"), cell: versionLabel },
    {
      id: "hash",
      header: t("document-column-hash"),
      cell: (v) => hashPrefix(v.sha256),
      mono: true,
    },
    {
      id: "uploader",
      header: t("document-column-uploader"),
      cell: (v) => names.get(v.uploaded_by) ?? t("document-uploader-unknown"),
    },
    {
      id: "created",
      header: t("document-column-created"),
      cell: (v) => (
        <time dateTime={v.created_at}>{createdFormat.format(new Date(v.created_at))}</time>
      ),
      numeric: true,
    },
    {
      id: "actions",
      header: t("document-column-actions"),
      cell: (v) =>
        v.kind === "upload" ? (
          <FileLink
            download
            href={contentUrl(v)}
            aria-label={t("document-download-of", { name: v.file_name ?? "", number: v.number })}
          >
            {t("document-download")}
          </FileLink>
        ) : (
          // A draft has no file: its download answers not-found.
          <span className={styles.muted}>{t("document-draft-no-download")}</span>
        ),
    },
  ];

  if (state.kind === "loaded") {
    return (
      <EventPage api={api} eventId={state.document.event_id}>
        <DocumentBody
          document={state.document}
          versions={state.versions}
          columns={columns}
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

/** The content of a loaded document. It sits in the layout of the event. */
function DocumentBody({
  document,
  versions,
  columns,
  heading,
}: {
  document: Document;
  versions: DocumentVersion[];
  columns: Column<DocumentVersion>[];
  heading: RefObject<HTMLHeadingElement | null>;
}) {
  const focusAfterCommit = useFocusAfterCommit();
  // The page arrived: focus goes to the heading of the document.
  useEffect(() => focusAfterCommit(() => heading.current), [focusAfterCommit, heading]);
  const newest = useMemo(() => [...versions].sort((a, b) => b.number - a.number)[0], [versions]);
  const preview = previewKind(newest);
  return (
    <>
      <Link to={`/events/${encodeURIComponent(document.event_id)}/documents`}>
        {t("document-back")}
      </Link>
      <header className={styles.header}>
        <p className={styles.key}>{document.readable_id}</p>
        <h2 ref={heading} tabIndex={-1} className={styles.title}>
          {document.name}
        </h2>
      </header>
      <section className={styles.section} aria-labelledby="document-versions-title">
        <h3 id="document-versions-title" className={styles.heading}>
          {t("document-versions-title")}
        </h3>
        <DataTable
          label={t("document-versions-title")}
          columns={columns}
          rows={versions}
          rowKey={(v) => v.id}
        />
        <p className={styles.help}>{t("documents-no-scan")}</p>
      </section>
      <section className={styles.section} aria-labelledby="document-preview-title">
        <h3 id="document-preview-title" className={styles.heading}>
          {t("document-preview-title")}
        </h3>
        {newest && preview === "text" && (
          <iframe
            className={styles.frame}
            title={t("document-preview-of", { name: newest.file_name ?? "" })}
            src={contentUrl(newest, true)}
          />
        )}
        {newest && preview === "pdf" && (
          // The server sends a download with `sandbox`, which a browser may refuse to show in a
          // frame. A link to a new tab keeps the CSP unchanged (ADR 0043).
          <div>
            <FileLink newTab href={contentUrl(newest, true)}>
              {t("document-preview-open")}
            </FileLink>
          </div>
        )}
        {newest?.kind === "draft" && <p>{t("document-preview-draft")}</p>}
        {newest?.kind === "upload" && !preview && <p>{t("document-preview-none")}</p>}
      </section>
    </>
  );
}
