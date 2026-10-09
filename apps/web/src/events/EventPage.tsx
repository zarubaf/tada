import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { useRetry } from "../ui/focus";
import { InlineError } from "../ui/InlineError";
import { KnowledgeState } from "../ui/KnowledgeState";
import { NavLink, SubNav } from "../ui/NavLink";
import { Page, PageTitle } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventPage.module.css";
import { EventContext, type ProfileState, useEventProfile } from "./eventContext";

type State =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; event: Event };

/**
 * The page of one event: the header with the name, the sub-navigation and the sub-page in
 * `children`. It owns the route `/events/:eventId`. The header shows the key, the name and the date
 * window of the event with its state of knowledge. The sub-pages read the event and its profile
 * through `useEventContext`.
 */
export function EventPage({
  api,
  children,
  eventId: eventIdProp,
}: {
  api: Api;
  children: ReactNode;
  /** For a page whose address has no event, for example a document: the event it belongs to. */
  eventId?: string;
}) {
  const params = useParams();
  const eventId = eventIdProp ?? params.eventId ?? "";
  const [state, setState] = useState<State>({ kind: "loading" });
  const heading = useRef<HTMLHeadingElement>(null);
  const { retried, retry } = useRetry(() => heading.current);
  const { profile, reloadProfile } = useEventProfile(api, eventId);
  const context = useMemo(
    () => (state.kind === "loaded" ? { event: state.event, profile, reloadProfile } : undefined),
    [state, profile, reloadProfile],
  );

  /** Resolves to true when the event loaded. */
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
      return data !== undefined;
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
      return false;
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
          onRetry={() =>
            retry(() => {
              setState({ kind: "loading" });
              return load();
            })
          }
          announce={retried ? "focus" : "alert"}
        />
      )}
      {state.kind === "loaded" && context && (
        <EventContext value={context}>
          <header className={styles.header}>
            <p className={styles.key}>{state.event.key}</p>
            <PageTitle ref={heading}>{state.event.name}</PageTitle>
            <DateWindow profile={profile} />
          </header>
          <SubNav label={t("event-nav")}>
            <NavLink to={base} exact>
              {t("event-nav-overview")}
            </NavLink>
            <NavLink to={`${base}/members`}>{t("event-nav-members")}</NavLink>
            <NavLink to={`${base}/documents`} within="/documents">
              {t("event-nav-documents")}
            </NavLink>
          </SubNav>
          {children}
        </EventContext>
      )}
    </Page>
  );
}

/** The date window of the event with its state. Nothing shows until the profile has loaded. */
function DateWindow({ profile }: { profile: ProfileState }) {
  if (profile.kind !== "loaded") {
    return null;
  }
  const fact = profile.profile.facts.find((candidate) => candidate.field_key === "date_window");
  const field = profile.fields.find((candidate) => candidate.key === "date_window");
  const state = fact?.state ?? "unknown";
  return (
    <p className={styles.dates}>
      <span className={styles.datesLabel}>
        {field ? formatLabel(field.label) : t("field-date_window")}:{" "}
      </span>
      <KnowledgeState state={state}>
        {state === "unknown" ? undefined : formatValue(fact ?? {}, field?.value_type)}
      </KnowledgeState>
    </p>
  );
}
