import { type ReactNode, useMemo, useRef } from "react";
import type { Api } from "../api/client";
import { formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { Page, PageTitle } from "../ui/Page";
import { RecordId } from "../ui/RecordId";
import { Redacted } from "../ui/Redacted";
import { Skeleton } from "../ui/Skeleton";
import { CloseEvent } from "./CloseEvent";
import { EventLoadError } from "./EventLoadError";
import styles from "./EventPage.module.css";
import { EventState } from "./EventState";
import { EventContext, type ProfileState, useEventProfile } from "./eventContext";
import { KnowledgeValue } from "./KnowledgeValue";
import { useEventState } from "./useEventState";

/**
 * The page of one event: the header with the name, the control to close it and the page in
 * `children`. It owns the route `/events/:eventId`. The header shows the key, the name and the date
 * window of the event with its state of knowledge. The destinations of an event are the items of
 * the sidebar (ADR 0082), so the page has no navigation of its own. The pages read the event and
 * its profile through `useEventContext`.
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
  const heading = useRef<HTMLHeadingElement>(null);
  const { state, setState, load, retried, retry } = useEventState(
    api,
    eventId,
    () => heading.current,
  );
  const { profile, reloadProfile } = useEventProfile(api, eventId);
  const context = useMemo(
    () => (state.kind === "loaded" ? { event: state.event, profile, reloadProfile } : undefined),
    [state, profile, reloadProfile],
  );

  return (
    <Page>
      {state.kind === "loading" && (
        <div className={styles.skeleton} role="status" aria-label={t("event-loading")}>
          <Skeleton />
          <Skeleton />
        </div>
      )}
      {state.kind === "failed" && (
        <EventLoadError failure={state} retried={retried} onRetry={retry} />
      )}
      {state.kind === "loaded" && context && (
        <EventContext value={context}>
          <CloseEvent
            api={api}
            event={state.event}
            onChanged={(event) => setState({ kind: "loaded", event })}
            onStale={() => void load()}
          >
            {({ control, notice }) => (
              <>
                <header className={styles.header}>
                  <div className={styles.titleRow}>
                    {/* A redacted event keeps its name (ADR 0072). */}
                    <PageTitle ref={heading}>{state.event.name}</PageTitle>
                    {control && <div className={styles.control}>{control}</div>}
                  </div>
                  <div className={styles.meta}>
                    <RecordId>{state.event.key}</RecordId>
                    <EventState status={state.event.status} />
                    <DateWindow profile={profile} />
                  </div>
                </header>
                {notice}
              </>
            )}
          </CloseEvent>
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
    <p>
      <span className={styles.datesLabel}>
        {field ? formatLabel(field.label) : t("field-date_window")}:{" "}
      </span>
      <KnowledgeValue state={state}>
        {state === "unknown" ? undefined : fact?.redacted ? (
          <Redacted />
        ) : (
          formatValue(fact ?? {}, field?.value_type)
        )}
      </KnowledgeValue>
    </p>
  );
}
