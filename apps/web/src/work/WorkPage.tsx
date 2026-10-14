import { IconAlertTriangle } from "@tabler/icons-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Api } from "../api/client";
import { EventLoadError } from "../events/EventLoadError";
import { useEventState } from "../events/useEventState";
import { todayLocal } from "../facts/dueDate";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { useSession } from "../session/SessionProvider";
import { useNarrow } from "../shell/useNarrow";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { useRetry } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { StandaloneLink } from "../ui/StandaloneLink";
import { Board } from "./Board";
import { BoardSkeleton } from "./BoardSkeleton";
import { RECORD_KINDS } from "./recordStatus";
import { type KindState, useWork } from "./useWork";
import { useWorkFilters } from "./useWorkFilters";
import styles from "./WorkPage.module.css";
import { WorkToolbar } from "./WorkToolbar";
import { countsFor, filterWork, NO_FILTERS, serializeFilters } from "./workFilters";

/**
 * „Arbeit“ (ADR 0082): all work records of an event on one page, with the filters in the query
 * string. The board is the only view for now; the table and the timeline come as further views.
 */
export function WorkPage({ api }: { api: Api }) {
  const eventId = useParams().eventId ?? "";
  const work = useWork(api, eventId);
  const me = useSession().user.id;
  const narrow = useNarrow();
  const [filters, setFilters] = useWorkFilters();
  const heading = useRef<HTMLHeadingElement>(null);
  const { retry } = useRetry(() => heading.current);
  const retrying = useRetrying(retry);
  const event = useEventState(api, eventId, () => heading.current);
  const today = todayLocal();

  const failed = RECORD_KINDS.filter((kind) => work.kinds[kind].kind === "failed");
  const loading =
    work.directory.kind === "loading" ||
    RECORD_KINDS.some((kind) => work.kinds[kind].kind === "loading");
  const ctx = useMemo(() => ({ me, today }), [me, today]);
  const shown = useMemo(() => filterWork(work.items, filters, ctx), [work.items, filters, ctx]);
  const counts = useMemo(
    () => (loading ? undefined : countsFor(work.items, filters, ctx)),
    [loading, work.items, filters, ctx],
  );

  // The result of a change of the filters is announced; the first view and a load are not.
  const key = serializeFilters(filters).toString();
  const lastKey = useRef(key);
  const [announcement, setAnnouncement] = useState<string>();
  useEffect(() => {
    if (lastKey.current !== key) {
      lastKey.current = key;
      setAnnouncement(t("work-count", { count: shown.length }));
    }
  }, [key, shown.length]);

  // The failures go to the same polite region, so that six failures are one announcement and not
  // six alerts. An event that failed to load shows its own alert instead.
  const failures = [
    ...(work.directory.kind === "failed" ? [t("work-directory-failed")] : []),
    ...failed.map((kind) => t("work-kind-failed", { kind: t(`work-type-${kind}`) })),
  ].join(" ");
  const eventFailed = event.state.kind === "failed";
  useEffect(() => {
    if (failures !== "" && !eventFailed) {
      setAnnouncement(failures);
    }
  }, [failures, eventFailed]);

  const workstreamName = (id: string) =>
    work.directory.kind === "loaded"
      ? work.directory.directory.workstreams.find((stream) => stream.id === id)?.name
      : undefined;

  const empty = !loading && failed.length === 0 && work.items.length === 0;
  const filteredOut = !loading && work.items.length > 0 && shown.length === 0;

  if (event.state.kind === "failed") {
    return (
      <Page>
        <div className={styles.page}>
          <PageTitle ref={heading}>{t("work-title")}</PageTitle>
          <EventLoadError failure={event.state} retried={event.retried} onRetry={event.retry} />
        </div>
      </Page>
    );
  }

  return (
    <Page>
      <div className={styles.page}>
        <PageTitle ref={heading}>{t("work-title")}</PageTitle>
        <WorkToolbar
          filters={filters}
          onChange={setFilters}
          counts={counts}
          directory={work.directory}
          me={me}
          narrow={narrow}
        />
        <LiveRegion kind="status" visuallyHidden>
          {announcement}
        </LiveRegion>
        {work.directory.kind === "failed" && (
          <FailureBanner
            message={t("work-directory-failed")}
            detail={work.directory.message}
            busy={retrying.has("directory")}
            onRetry={() => retrying.start("directory", work.reloadDirectory)}
          />
        )}
        {failed.map((kind) => (
          <FailureBanner
            key={kind}
            message={t("work-kind-failed", { kind: t(`work-type-${kind}`) })}
            detail={failureMessage(work.kinds[kind])}
            busy={retrying.has(kind)}
            onRetry={() => retrying.start(kind, () => work.reloadKind(kind))}
          />
        ))}
        {loading && <BoardSkeleton narrow={narrow} />}
        {empty && (
          <EmptyState
            title={t("work-empty-title")}
            text={t("work-empty-text")}
            action={
              <StandaloneLink to={`/events/${encodeURIComponent(eventId)}/actions`}>
                {t("work-empty-new-action")}
              </StandaloneLink>
            }
          />
        )}
        {filteredOut && (
          <EmptyState
            title={t("work-empty-filtered-title")}
            text={t("work-empty-filtered-text")}
            action={
              <Button onPress={() => setFilters(NO_FILTERS)}>{t("work-filter-reset")}</Button>
            }
          />
        )}
        {!loading && shown.length > 0 && (
          <Board
            items={shown}
            eventId={eventId}
            today={today}
            workstreamName={workstreamName}
            narrow={narrow}
          />
        )}
      </div>
    </Page>
  );
}

/**
 * The retries that run. A retry keeps its banner on screen, so a second press is ignored while it
 * runs; the button stays in the page and keeps focus when the retry fails again.
 */
function useRetrying(retry: (load: () => Promise<boolean>) => void) {
  const [running, setRunning] = useState<ReadonlySet<string>>(new Set());
  const change = (key: string, on: boolean) =>
    setRunning((current) => {
      const next = new Set(current);
      next[on ? "add" : "delete"](key);
      return next;
    });
  return {
    has: (key: string) => running.has(key),
    start: (key: string, load: () => Promise<boolean>) => {
      if (!running.has(key)) {
        change(key, true);
        retry(() => load().finally(() => change(key, false)));
      }
    },
  };
}

function failureMessage(state: KindState<unknown>): string | undefined {
  return state.kind === "failed" ? state.message : undefined;
}

function FailureBanner({
  message,
  detail,
  busy,
  onRetry,
}: {
  message: string;
  detail: string | undefined;
  /** True while the retry runs. */
  busy: boolean;
  onRetry: () => void;
}) {
  return (
    <div aria-busy={busy}>
      <Banner icon={IconAlertTriangle} action={<Button onPress={onRetry}>{t("retry")}</Button>}>
        <p>
          <strong>{message}</strong>
        </p>
        {detail && <p>{detail}</p>}
      </Banner>
    </div>
  );
}
