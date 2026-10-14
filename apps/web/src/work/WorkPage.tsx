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
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { StandaloneLink } from "../ui/StandaloneLink";
import { BoardSkeleton } from "./BoardSkeleton";
import { RECORD_KINDS } from "./recordStatus";
import { type KindState, useWork } from "./useWork";
import { useWorkFilters } from "./useWorkFilters";
import styles from "./WorkPage.module.css";
import { WorkToolbar } from "./WorkToolbar";
import { WorkViews } from "./WorkViews";
import { WorkViewTabs } from "./WorkViewTabs";
import { countsFor, filterWork, NO_FILTERS, serializeFilters } from "./workFilters";
import { useWorkView } from "./workView";

/**
 * „Arbeit“ (ADR 0082): all work records of an event on one page, with the filters in the query
 * string. The tabs choose the view: board, table or timeline; the filters are the same in all.
 */
export function WorkPage({ api }: { api: Api }) {
  const eventId = useParams().eventId ?? "";
  const work = useWork(api, eventId);
  const me = useSession().user.id;
  const narrow = useNarrow();
  const [filters, setFilters] = useWorkFilters();
  const [view, setView] = useWorkView();
  const heading = useRef<HTMLHeadingElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const { retry } = useRetry(() => heading.current);
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
  // six alerts. Only a failure that is new is announced, and the region is empty when none is. An
  // event that failed to load shows its own alert instead.
  const failures = [
    ...(work.directory.kind === "failed" ? [t("work-directory-failed")] : []),
    ...failed.map((kind) => t("work-kind-failed", { kind: t(`work-type-${kind}`) })),
  ];
  const failuresKey = failures.join("\n");
  const announced = useRef<readonly string[]>([]);
  const eventFailed = event.state.kind === "failed";
  // biome-ignore lint/correctness/useExhaustiveDependencies: `failuresKey` stands for `failures`
  useEffect(() => {
    if (eventFailed) {
      return;
    }
    const fresh = failures.filter((text) => !announced.current.includes(text));
    announced.current = failures;
    setAnnouncement(fresh.length > 0 ? fresh.join(" ") : undefined);
  }, [failuresKey, eventFailed]);
  // A retry that fails again changes no failure, so it says so. The region is empty while the
  // retry runs, so that the same text twice is a change again.
  const retrying = useRetrying(retry, {
    onStart: () => setAnnouncement(undefined),
    onFailedAgain: () => setAnnouncement(t("work-retry-failed")),
  });

  // The reset button leaves with the filters, so focus goes to the search; behind the closed
  // sheet of a phone there is none, so it goes to the heading.
  const reset = () => {
    setFilters(NO_FILTERS);
    focusAfterCommit(() => search.current ?? heading.current);
  };

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
        <WorkViewTabs view={view} onChange={setView}>
          <WorkToolbar
            filters={filters}
            onChange={setFilters}
            counts={counts}
            directory={work.directory}
            me={me}
            narrow={narrow}
            onReset={reset}
            searchRef={search}
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
              action={<Button onPress={reset}>{t("work-filter-reset")}</Button>}
            />
          )}
          {!loading && shown.length > 0 && (
            <WorkViews
              view={view}
              items={shown}
              eventId={eventId}
              today={today}
              workstreamName={workstreamName}
              narrow={narrow}
              timeline={{
                api,
                filters,
                workstreams:
                  work.directory.kind === "loaded" ? work.directory.directory.workstreams : [],
                timeZone: event.state.kind === "loaded" ? event.state.event.time_zone : undefined,
                onReset: reset,
              }}
            />
          )}
        </WorkViewTabs>
      </div>
    </Page>
  );
}

/**
 * The retries that run. A retry keeps its banner on screen, so a second press is ignored while it
 * runs; the button stays in the page and keeps focus when the retry fails again.
 */
function useRetrying(
  retry: (load: () => Promise<boolean>) => void,
  on: { onStart: () => void; onFailedAgain: () => void },
) {
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
        on.onStart();
        retry(() =>
          load()
            .then((loaded) => {
              if (!loaded) {
                on.onFailedAgain();
              }
              return loaded;
            })
            .finally(() => change(key, false)),
        );
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
