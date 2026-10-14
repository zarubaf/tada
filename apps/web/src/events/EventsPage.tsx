import { useCallback, useEffect, useRef, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { LOCALE, t } from "../i18n";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { LinkButton } from "../ui/LinkButton";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import { StandaloneLink } from "../ui/StandaloneLink";
import styles from "./EventsPage.module.css";
import { useCanCreateEvent } from "./useCanCreateEvent";

const createdFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });

const columns: Column<Event>[] = [
  { id: "key", header: t("events-column-key"), cell: (event) => event.key, mono: true },
  // A redacted event keeps its name (ADR 0072).
  {
    id: "name",
    header: t("events-column-name"),
    cell: (event) => (
      <StandaloneLink inRow to={`/events/${encodeURIComponent(event.id)}`}>
        {event.name}
      </StandaloneLink>
    ),
  },
  { id: "time-zone", header: t("events-column-time-zone"), cell: (event) => event.time_zone },
  {
    id: "created",
    header: t("events-column-created"),
    cell: (event) => (
      <time dateTime={event.created_at}>{createdFormat.format(new Date(event.created_at))}</time>
    ),
    numeric: true,
  },
];

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; events: Event[]; nextCursor: string | undefined; loadingMore: boolean };

/** „Anlässe“: the events that the member can see, in the order of their keys. */
export function EventsPage({ api }: { api: Api }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const canCreate = useCanCreateEvent();
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retried, retry } = useRetry(() => heading.current);
  // The failure of „Weitere laden“: the button stays and keeps focus, so the region announces it.
  const [failure, setFailure] = useState<string>();
  const [confirmation, setConfirmation] = useState<string>();

  /** The first page. Resolves to true when it loaded. */
  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/events", { params: { query: {} } });
      if (data) {
        setState({
          kind: "loaded",
          events: data.items,
          nextCursor: data.next_cursor ?? undefined,
          loadingMore: false,
        });
        return true;
      }
      setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
    return false;
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  /** The next page: the loaded rows stay, and a failure keeps the cursor for the next press. */
  const loadMore = async (events: Event[], cursor: string) => {
    setFailure(undefined);
    setConfirmation(undefined);
    setState({ kind: "loaded", events, nextCursor: cursor, loadingMore: true });
    let message: string;
    try {
      const { data, error } = await api.GET("/api/v1/events", { params: { query: { cursor } } });
      if (data) {
        const nextCursor = data.next_cursor ?? undefined;
        setState({
          kind: "loaded",
          events: [...events, ...data.items],
          nextCursor,
          loadingMore: false,
        });
        setConfirmation(t("events-loaded-more"));
        if (nextCursor === undefined) {
          // The last page arrived and the button leaves: focus goes to the heading.
          focusAfterCommit(() => heading.current);
        }
        return;
      }
      message = problemMessage(error);
    } catch {
      message = problemMessage(undefined);
    }
    setState({ kind: "loaded", events, nextCursor: cursor, loadingMore: false });
    setFailure(message);
  };

  return (
    <Page>
      <div className={styles.toolbar}>
        <PageTitle ref={heading}>{t("events-title")}</PageTitle>
        {canCreate && (
          <LinkButton to="/events/new" primary>
            {t("events-create")}
          </LinkButton>
        )}
      </div>
      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("events-loading")}>
          <Skeleton />
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && state.events.length === 0 && (
        <EmptyState title={t("events-empty-title")} text={t("events-empty-text")} />
      )}
      {state.kind === "loaded" && state.events.length > 0 && (
        <>
          <DataTable
            label={t("events-title")}
            columns={columns}
            rows={state.events}
            rowKey={(event) => event.id}
          />
          <LiveRegion kind="alert">{failure}</LiveRegion>
          <LiveRegion kind="status">{confirmation}</LiveRegion>
          {state.nextCursor !== undefined && (
            <div>
              <Button
                isPending={state.loadingMore}
                onPress={() => {
                  const { events, nextCursor } = state;
                  if (!state.loadingMore && nextCursor !== undefined) {
                    void loadMore(events, nextCursor);
                  }
                }}
              >
                {t("events-load-more")}
              </Button>
            </div>
          )}
        </>
      )}
    </Page>
  );
}
