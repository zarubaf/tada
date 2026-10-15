import { type ReactNode, useMemo } from "react";
import type { Api } from "../api/client";
import { EventLoadError } from "../events/EventLoadError";
import { EventContext, useEventProfile } from "../events/eventContext";
import { useEventState } from "../events/useEventState";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { Page } from "../ui/Page";
import { Skeleton } from "../ui/Skeleton";
import styles from "./EventScope.module.css";

/**
 * The event that the pages of the documents belong to: it loads the event and its profile and
 * gives them to the pages through `EventContext`. Unlike `EventPage` it adds no header: the
 * documents pages have their own toolbar row with the `h1` (ADR 0082), and the event home keeps
 * the header of the event and the control to close it.
 */
export function EventScope({ api, children }: { api: Api; children: ReactNode }) {
  const { eventId = "" } = useParams();
  // After a failed load the page has no heading yet: focus goes to the heading that the page adds.
  const { state, retried, retry } = useEventState(api, eventId, () =>
    document.querySelector<HTMLElement>("main h1"),
  );
  const { profile, reloadProfile } = useEventProfile(api, eventId);
  const context = useMemo(
    () => (state.kind === "loaded" ? { event: state.event, profile, reloadProfile } : undefined),
    [state, profile, reloadProfile],
  );

  if (state.kind === "loaded" && context) {
    return <EventContext value={context}>{children}</EventContext>;
  }
  return (
    <Page width="flush">
      <div className={styles.state}>
        {state.kind === "loading" && (
          <div className={styles.skeleton} role="status" aria-label={t("event-loading")}>
            <Skeleton />
            <Skeleton />
          </div>
        )}
        {state.kind === "failed" && (
          <EventLoadError failure={state} retried={retried} onRetry={retry} />
        )}
      </div>
    </Page>
  );
}
