import { useCallback, useEffect, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { LOCALE, t } from "../i18n";
import { useOptionalSession } from "../session/SessionProvider";
import { Button } from "../ui/Button";
import { type Column, DataTable } from "../ui/DataTable";
import { EmptyState } from "../ui/EmptyState";
import { InlineError } from "../ui/InlineError";
import { LinkButton } from "../ui/LinkButton";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventsPage.module.css";

const createdFormat = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });

const columns: Column<Event>[] = [
  { id: "key", header: t("events-column-key"), cell: (event) => event.key, mono: true },
  { id: "name", header: t("events-column-name"), cell: (event) => event.name },
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
  // The server decides; the button only hides an action that would fail.
  const role = useOptionalSession()?.organization?.role;
  const canCreate = role === "owner" || role === "admin";

  const load = useCallback(
    async (cursor: string | undefined, previous: Event[]) => {
      const query = cursor === undefined ? {} : { cursor };
      try {
        const { data, error } = await api.GET("/api/v1/events", { params: { query } });
        if (data) {
          setState({
            kind: "loaded",
            events: [...previous, ...data.items],
            nextCursor: data.next_cursor ?? undefined,
            loadingMore: false,
          });
        } else {
          setState({
            kind: "failed",
            message: problemMessage(error),
            requestId: error?.request_id,
          });
        }
      } catch {
        setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
      }
    },
    [api],
  );

  useEffect(() => {
    void load(undefined, []);
  }, [load]);

  const retry = () => {
    setState({ kind: "loading" });
    void load(undefined, []);
  };

  return (
    <main id="main" className={styles.page}>
      <div className={styles.toolbar}>
        <h1 className={styles.title}>{t("events-title")}</h1>
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
        <InlineError message={state.message} requestId={state.requestId} onRetry={retry} />
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
          {state.nextCursor !== undefined && (
            <Button
              isDisabled={state.loadingMore}
              onPress={() => {
                setState({ ...state, loadingMore: true });
                void load(state.nextCursor, state.events);
              }}
            >
              {t("events-load-more")}
            </Button>
          )}
        </>
      )}
    </main>
  );
}
