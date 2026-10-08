import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { type Api, type Document, problemMessage } from "../api/client";
import { uploadDocument, uploadMessage } from "../api/upload";
import { LOCALE, t } from "../i18n";
import { Link, useParams } from "../router/Router";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { FileButton } from "../ui/FileButton";
import { useFocusAfterCommit } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LiveRegion } from "../ui/LiveRegion";
import { Skeleton } from "../ui/Skeleton";
import { TextField } from "../ui/TextField";
import styles from "./DocumentsPage.module.css";
import { formatSize } from "./format";
import { searchParam } from "./search";

const createdFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined; retried: boolean }
  | { kind: "loaded"; items: Document[]; nextCursor: string | undefined; loadingMore: boolean };

const columns: Column<Document>[] = [
  { id: "id", header: t("documents-column-id"), cell: (d) => d.readable_id, mono: true },
  {
    id: "name",
    header: t("documents-column-name"),
    cell: (d) => <Link to={`/documents/${encodeURIComponent(d.id)}`}>{d.name}</Link>,
  },
  {
    id: "type",
    header: t("documents-column-type"),
    cell: (d) => d.newest_version.media_type ?? t("document-draft"),
  },
  {
    id: "size",
    header: t("documents-column-size"),
    cell: (d) =>
      typeof d.newest_version.size_bytes === "number"
        ? formatSize(d.newest_version.size_bytes)
        : "",
    numeric: true,
  },
  {
    id: "version",
    header: t("documents-column-version"),
    cell: (d) => d.newest_version.number,
    numeric: true,
  },
  {
    id: "created",
    header: t("documents-column-created"),
    cell: (d) => (
      <time dateTime={d.newest_version.created_at}>
        {createdFormat.format(new Date(d.newest_version.created_at))}
      </time>
    ),
    numeric: true,
  },
];

/** „Dokumente“ of an event: a flat list with a search field and the upload (Documents template). */
export function DocumentsPage({ api }: { api: Api }) {
  const { eventId = "" } = useParams();
  const [state, setState] = useState<State>({ kind: "loading" });
  // The text in the field, and the text of the search that the list shows.
  const [text, setText] = useState("");
  const [query, setQuery] = useState<string>();
  const [uploading, setUploading] = useState(false);
  const [uploaded, setUploaded] = useState<string>();
  const [uploadFailure, setUploadFailure] = useState<string>();
  const heading = useRef<HTMLHeadingElement>(null);
  // The newest request. The answer of an older one is dropped, so that it cannot replace the list.
  const latest = useRef(0);
  // Retries so far. After a retry, focus must not fall to the body when the error leaves.
  const retries = useRef(0);
  // A file goes up: a second pick does nothing, also before the next render.
  const uploadRunning = useRef(false);
  const search = useRef<HTMLDivElement>(null);
  const focusAfterCommit = useFocusAfterCommit();

  const load = useCallback(
    async (q: string | undefined, cursor: string | undefined, previous: Document[]) => {
      const params = { path: { event_id: eventId }, query: { q, cursor } };
      const request = ++latest.current;
      const retried = retries.current > 0;
      try {
        const { data, error } = await api.GET("/api/v1/events/{event_id}/documents", { params });
        if (request !== latest.current) {
          return;
        }
        if (data) {
          const nextCursor = data.next_cursor ?? undefined;
          setState({
            kind: "loaded",
            items: [...previous, ...data.items],
            nextCursor,
            loadingMore: false,
          });
          if ((cursor !== undefined && nextCursor === undefined) || retried) {
            // The last page arrived and the button leaves, or the retry button left with the
            // error: focus goes to the heading.
            retries.current = 0;
            focusAfterCommit(() => heading.current);
          }
        } else {
          setState({
            kind: "failed",
            message: problemMessage(error),
            requestId: error?.request_id,
            retried,
          });
        }
      } catch {
        if (request === latest.current) {
          setState({
            kind: "failed",
            message: problemMessage(undefined),
            requestId: undefined,
            retried,
          });
        }
      }
    },
    [api, eventId, focusAfterCommit],
  );

  useEffect(() => {
    void load(query, undefined, []);
  }, [load, query]);

  const onSearch = (event: FormEvent) => {
    event.preventDefault();
    const next = searchParam(text);
    if (next === query) {
      void load(query, undefined, []);
    } else {
      setQuery(next);
    }
  };

  const reset = () => {
    setText("");
    setQuery(undefined);
    // The reset button leaves with the empty result: focus goes to the search field.
    focusAfterCommit(() => search.current?.querySelector("input"));
  };

  const upload = async (file: File) => {
    if (uploadRunning.current) {
      return;
    }
    uploadRunning.current = true;
    setUploading(true);
    setUploaded(undefined);
    setUploadFailure(undefined);
    const result = await uploadDocument(api, eventId, file);
    if ("document" in result) {
      setUploaded(t("documents-uploaded", { name: result.document.name }));
      void load(query, undefined, []);
    } else {
      // The button stays and keeps focus; the alert carries the message.
      setUploadFailure(uploadMessage(result.error, result.response));
    }
    uploadRunning.current = false;
    setUploading(false);
  };

  return (
    <section className={styles.documents} aria-labelledby="documents-title">
      <div className={styles.toolbar}>
        <h2 id="documents-title" ref={heading} tabIndex={-1} className={styles.heading}>
          {t("documents-title")}
        </h2>
        <FileButton isPending={uploading} onSelect={(file) => void upload(file)}>
          {t("documents-upload")}
        </FileButton>
      </div>
      <p className={styles.help}>{t("documents-upload-limits")}</p>
      <LiveRegion kind="status" className={styles.status}>
        {uploading ? t("documents-uploading") : uploaded}
      </LiveRegion>
      <LiveRegion kind="alert" className={styles.failure}>
        {uploadFailure}
      </LiveRegion>
      <search>
        <form className={styles.search} onSubmit={onSearch}>
          <div ref={search}>
            <TextField
              label={t("documents-search")}
              type="search"
              value={text}
              onChange={setText}
            />
          </div>
          <Button type="submit">{t("documents-search-submit")}</Button>
        </form>
      </search>
      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("documents-loading")}>
          <Skeleton />
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          takeFocus={state.retried}
          onRetry={() => {
            retries.current += 1;
            setState({ kind: "loading" });
            void load(query, undefined, []);
          }}
        />
      )}
      {state.kind === "loaded" && state.items.length === 0 && query === undefined && (
        <EmptyState title={t("documents-none-title")} text={t("documents-none-text")} />
      )}
      {state.kind === "loaded" && state.items.length === 0 && query !== undefined && (
        <EmptyState
          title={t("documents-no-match")}
          text={t("documents-no-match-text")}
          action={<Button onPress={reset}>{t("documents-search-reset")}</Button>}
        />
      )}
      {state.kind === "loaded" && state.items.length > 0 && (
        <>
          <DataTable
            label={t("documents-title")}
            columns={columns}
            rows={state.items}
            rowKey={(d) => d.id}
          />
          {state.nextCursor !== undefined && (
            <Button
              isPending={state.loadingMore}
              onPress={() => {
                setState({ ...state, loadingMore: true });
                void load(query, state.nextCursor, state.items);
              }}
            >
              {t("documents-more")}
            </Button>
          )}
        </>
      )}
    </section>
  );
}
