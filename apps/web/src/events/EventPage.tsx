import { type ReactNode, useCallback, useEffect, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { InlineError } from "../ui/InlineError";
import { NavLink, SubNav } from "../ui/NavLink";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventPage.module.css";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; event: Event };

/**
 * The page of one event: the header with the name, the sub-navigation and the sub-page in
 * `children`. It owns the route `/events/:eventId`. The header shows the key and the name only.
 */
export function EventPage({ api, children }: { api: Api; children: ReactNode }) {
  const { eventId = "" } = useParams();
  const [state, setState] = useState<State>({ kind: "loading" });

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/events/{event_id}", {
        params: { path: { event_id: eventId } },
      });
      setState(
        data
          ? { kind: "loaded", event: data }
          : { kind: "failed", message: problemMessage(error), requestId: error?.request_id },
      );
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api, eventId]);

  useEffect(() => {
    void load();
  }, [load]);

  const base = `/events/${encodeURIComponent(eventId)}`;
  return (
    <Page>
      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("event-loading")}>
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() => {
            setState({ kind: "loading" });
            void load();
          }}
        />
      )}
      {state.kind === "loaded" && (
        <>
          <header className={styles.header}>
            <p className={styles.key}>{state.event.key}</p>
            <PageTitle>{state.event.name}</PageTitle>
          </header>
          <SubNav label={t("event-nav")}>
            <NavLink to={base} exact>
              {t("event-nav-overview")}
            </NavLink>
            <NavLink to={`${base}/members`}>{t("event-nav-members")}</NavLink>
          </SubNav>
          {children}
        </>
      )}
    </Page>
  );
}
