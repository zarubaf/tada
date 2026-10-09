import { IconAlertTriangle } from "@tabler/icons-react";
import { type RefObject, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type Api,
  type Document,
  type DocumentVersion,
  type DraftRendering,
  problemMessage,
} from "../api/client";
import { EventPage } from "../events/EventPage";
import { useEventContext } from "../events/eventContext";
import { loadOrganizationMembers } from "../events/eventMembers";
import { LOCALE, t } from "../i18n";
import { Link, useParams } from "../router/Router";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { FileLink } from "../ui/FileLink";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { Page } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./DocumentPage.module.css";
import { DraftView } from "./DraftView";
import { formatSize, hashPrefix } from "./format";
import { useCanApprove } from "./useCanApprove";
import { diffPath, isApprovable, newestVersion, previousDraft } from "./versions";

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
          <>
            <span className={styles.muted}>{t("document-draft-no-download")}</span>
            <DiffLink documentId={documentId} versions={versionsOf(state)} version={v} />
          </>
        ),
    },
  ];

  if (state.kind === "loaded") {
    return (
      <EventPage api={api} eventId={state.document.event_id}>
        <DocumentBody
          api={api}
          reload={load}
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

/** The versions of a loaded page, or none. */
function versionsOf(state: State): DocumentVersion[] {
  return state.kind === "loaded" ? state.versions : [];
}

/** The link to the difference between a draft and the draft before it. */
function DiffLink({
  documentId,
  versions,
  version,
}: {
  documentId: string;
  versions: DocumentVersion[];
  version: DocumentVersion;
}) {
  const previous = previousDraft(versions, version);
  return previous ? (
    <Link
      to={diffPath(documentId, previous, version)}
      aria-label={t("document-diff-link-to", { number: previous.number })}
    >
      {t("document-diff-link")}
    </Link>
  ) : null;
}

type RenderingState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; draft: DraftRendering };

/** Loads the rendering of a draft version. Without a version, it loads nothing. */
function useRendering(api: Api, versionId: string | undefined) {
  const [state, setState] = useState<RenderingState>({ kind: "loading" });
  const latest = useRef(0);

  /** Resolves to true when the draft loaded. */
  const load = useCallback(async () => {
    if (!versionId) {
      return true;
    }
    const request = ++latest.current;
    const done = (next: RenderingState) => {
      if (request === latest.current) {
        setState(next);
      }
      return next.kind === "loaded";
    };
    try {
      const { data, error } = await api.GET("/api/v1/document-versions/{version_id}/rendering", {
        params: { path: { version_id: versionId } },
      });
      return done(
        data
          ? { kind: "loaded", draft: data.draft }
          : { kind: "failed", message: problemMessage(error), requestId: error?.request_id },
      );
    } catch {
      return done({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api, versionId]);

  useEffect(() => {
    setState({ kind: "loading" });
    void load();
  }, [load]);

  return { state, load, loading: () => setState({ kind: "loading" }) };
}

/** The content of a loaded document. It sits in the layout of the event. */
function DocumentBody({
  api,
  reload,
  document,
  versions,
  columns,
  heading,
}: {
  api: Api;
  /** Loads the document and its versions again. Resolves to true when they loaded. */
  reload: () => Promise<boolean>;
  document: Document;
  versions: DocumentVersion[];
  columns: Column<DocumentVersion>[];
  heading: RefObject<HTMLHeadingElement | null>;
}) {
  const focusAfterCommit = useFocusAfterCommit();
  // The page arrived: focus goes to the heading of the document.
  useEffect(() => focusAfterCommit(() => heading.current), [focusAfterCommit, heading]);
  const { event, profile } = useEventContext();
  const newest = useMemo(() => newestVersion(versions), [versions]);
  const preview = previewKind(newest);
  const rendering = useRendering(api, newest?.kind === "draft" ? newest.id : undefined);
  const previewHeading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => previewHeading.current);
  const canApprove = useCanApprove(api, document.event_id);
  const [approving, setApproving] = useState(false);
  const [approved, setApproved] = useState<string>();
  const [failed, setFailed] = useState<string>();

  /** The button stays while it runs and when the call fails, so focus stays on it. */
  const approve = async () => {
    if (!newest || approving) {
      return;
    }
    setApproving(true);
    setApproved(undefined);
    setFailed(undefined);
    try {
      const { data, error } = await api.POST("/api/v1/document-versions/{version_id}/approve", {
        params: { path: { version_id: newest.id } },
        body: { expected_version: document.version },
      });
      if (data) {
        setApproved(t("document-approved", { number: data.number }));
        await reload();
        // The button left with the approval.
        focusAfterCommit(() => heading.current);
        return;
      }
      setFailed(
        error?.code === "invalid-transition"
          ? t("document-approve-invalid")
          : problemMessage(error),
      );
      if (error?.code === "invalid-transition" || error?.code === "record-version-conflict") {
        // Someone else changed the document: the page shows its current state. If the button
        // left with it, focus would fall to the page: it moves to the heading instead.
        await reload();
        focusAfterCommit(() =>
          window.document.activeElement === window.document.body ? heading.current : undefined,
        );
      }
    } catch {
      setFailed(problemMessage(undefined));
    } finally {
      setApproving(false);
    }
  };

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
      <LiveRegion kind="status">{approved}</LiveRegion>
      <LiveRegion kind="alert">{failed}</LiveRegion>
      {/* An absent value means that the server does not know: the page then says nothing. */}
      {document.facts_changed === true && (
        <section className={styles.changed} aria-labelledby="document-facts-changed-title">
          <h3 id="document-facts-changed-title" className={styles.changedTitle}>
            <IconAlertTriangle size={16} stroke={1.5} aria-hidden="true" />
            {t("document-facts-changed-title")}
          </h3>
          <p>{t("document-facts-changed-text")}</p>
        </section>
      )}
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
        <h3
          id="document-preview-title"
          ref={previewHeading}
          tabIndex={-1}
          className={styles.heading}
        >
          {t("document-preview-title")}
        </h3>
        {newest && isApprovable(newest) && canApprove && (
          <div>
            <Button variant="primary" isPending={approving} onPress={() => void approve()}>
              {t("document-approve")}
            </Button>
          </div>
        )}
        {newest?.kind === "draft" && rendering.state.kind === "loading" && <Skeleton />}
        {newest?.kind === "draft" && rendering.state.kind === "failed" && (
          <InlineError
            message={rendering.state.message}
            requestId={rendering.state.requestId}
            announce={retried ? "focus" : "alert"}
            onRetry={() =>
              retry(() => {
                rendering.loading();
                return rendering.load();
              })
            }
          />
        )}
        {newest?.kind === "draft" && rendering.state.kind === "loaded" && (
          <DraftView
            api={api}
            eventId={document.event_id}
            timeZone={event.time_zone}
            draft={rendering.state.draft}
            profile={profile}
          />
        )}
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
        {newest?.kind === "upload" && !preview && <p>{t("document-preview-none")}</p>}
      </section>
    </>
  );
}
