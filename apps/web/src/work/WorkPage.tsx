import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import type { Api } from "../api/client";
import { EventLoadError } from "../events/EventLoadError";
import { useEventState } from "../events/useEventState";
import { todayLocal } from "../facts/dueDate";
import { t } from "../i18n";
import { useParams } from "../router/Router";
import { useSession } from "../session/SessionProvider";
import { useNarrow } from "../shell/useNarrow";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit, useRetry } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Page, PageTitle } from "../ui/Page";
import { BoardSkeleton } from "./BoardSkeleton";
import { BulkBar } from "./BulkBar";
import { useBoardView, visibleItems } from "./boardView";
import { displayOrder, neighbors } from "./displayOrder";
import { FailureBanner } from "./FailureBanner";
import { useRecordRoute } from "./record/recordRoute";
import { RECORD_KINDS } from "./recordStatus";
import { useBulkChange } from "./useBulkChange";
import { type KindState, useWork } from "./useWork";
import { useWorkFilters } from "./useWorkFilters";
import { lockSelection, useWorkSelection, type WorkSelection } from "./useWorkSelection";
import styles from "./WorkPage.module.css";
import { WorkToolbar } from "./WorkToolbar";
import { WorkViews } from "./WorkViews";
import { WorkViewTabs } from "./WorkViewTabs";
import { countsFor, filterWork, groupByColumn, NO_FILTERS, serializeFilters } from "./workFilters";
import { opensInPanel } from "./workLinks";
import { canCreate } from "./workPermissions";
import { useWorkSort, useWorkView } from "./workView";

// The panel loads on demand: the board does not pay for the editors of a record (ADR 0082).
const RecordPanel = lazy(() => import("./record/RecordPanel"));

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
  const [sort] = useWorkSort();
  const record = useRecordRoute();
  const layout = useRef<HTMLDivElement>(null);
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
  // A closed event is read-only (ADR 0071); the event decides after it has loaded.
  const writable = event.state.kind === "loaded" && event.state.event.status !== "closed";
  const [boardView] = useBoardView();
  const counts = useMemo(
    () => (loading ? undefined : countsFor(work.items, filters, ctx)),
    [loading, work.items, filters, ctx],
  );

  const [announcement, setAnnouncement] = useState<string>();
  // The bulk change lives here and not in the bar: a run goes on when the bar leaves (Escape, a
  // filter, another event), and its result still has to be said.
  const banner = useRef<HTMLDivElement>(null);
  const dock = useRef<HTMLDivElement>(null);
  // The run is created before the selection, which it locks; its result reaches the selection here.
  const selectionRef = useRef<WorkSelection>(undefined);
  // Focus follows the end of a run only if it is in the bar or nowhere: a member who went on to
  // the search field must not have focus pulled away in the middle of a word.
  const focusIsFree = () => {
    const active = document.activeElement;
    return !active || active === document.body || dock.current?.contains(active) === true;
  };
  const bulk = useBulkChange({
    scope: eventId,
    commands: work.commands,
    refresh: work.refresh,
    items: work.items,
    onFinished: ({ changed, failedKeys }) => {
      const selection = selectionRef.current;
      if (failedKeys.size === 0) {
        setAnnouncement(t("work-bulk-done", { count: changed }));
        // The bar leaves with the selection, so focus moves now and not after the commit.
        if (focusIsFree()) {
          heading.current?.focus();
        }
        selection?.clear();
        return;
      }
      selection?.keepOnly(failedKeys);
      setAnnouncement(t("work-bulk-failed", { count: failedKeys.size }));
      focusAfterCommit(() => (focusIsFree() ? banner.current : null));
    },
  });
  const { progress, reloading } = bulk.state;
  const running = progress !== undefined;
  // Only what the view shows can stay selected, so the bulk bar never acts on a record out of sight.
  // The timeline shows none, but a run, and the failures it left, keep their records selected for
  // the return to the table or the board: the bar is not in the timeline to say them.
  const pending = running || (progress === undefined && bulk.state.failures.length > 0);
  const selectIn = view === "timeline" && pending ? "table" : view;
  const visible = useMemo(
    () => visibleItems(groupByColumn(shown), boardView, narrow, selectIn),
    [shown, boardView, narrow, selectIn],
  );
  const selection = useWorkSelection(eventId, visible, writable, running);
  selectionRef.current = selection;
  // Ending the selection ends the failures that belonged to it, once nothing runs or loads.
  const idle = !running && !reloading;
  const { reset: resetBulk } = bulk;
  useEffect(() => {
    if (!selection.selecting && idle) {
      resetBulk();
    }
  }, [selection.selecting, idle, resetBulk]);
  // While the change runs the selection holds still, so that the result meets the same records.
  const held = useMemo(
    () => (running ? lockSelection(selection) : selection),
    [running, selection],
  );

  // The result of a change of the filters is announced; the first view and a load are not.
  const key = serializeFilters(filters).toString();
  const lastKey = useRef(key);
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

  // The panel steps through what the view shows, in the order it shows it.
  const order = displayOrder({
    items: shown,
    view,
    boardView,
    narrow,
    sort,
    areaOf: (item) =>
      item.workstreamId === undefined ? undefined : workstreamName(item.workstreamId),
  });
  const { route } = record;
  const openId = route.mode === "open" ? route.id : undefined;
  const writableDirectory = work.directory.kind === "loaded" ? work.directory.directory : undefined;
  const mayCreate = canCreate(writableDirectory, me, writable);
  // The record that the create state made: the panel puts focus on its title once.
  const [made, setMade] = useState<string>();
  const around = openId === undefined ? undefined : neighbors(order.filter(opensInPanel), openId);
  // Focus goes back to the card or the row of the record, else to the heading of the page.
  const returnFocus = () =>
    (openId === undefined
      ? undefined
      : layout.current?.querySelector<HTMLElement>(`a[href$="#${CSS.escape(openId)}"]`)) ??
    heading.current;

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
    <div ref={layout} className={styles.layout}>
      <div className={styles.main}>
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
                onCreate={mayCreate ? () => record.startCreate("action") : undefined}
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
                    mayCreate ? (
                      <Button onPress={() => record.startCreate("action")}>
                        {t("work-empty-new-action")}
                      </Button>
                    ) : undefined
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
                <div className={styles.scope}>
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
                        work.directory.kind === "loaded"
                          ? work.directory.directory.workstreams
                          : [],
                      timeZone:
                        event.state.kind === "loaded" ? event.state.event.time_zone : undefined,
                      onReset: reset,
                    }}
                    selection={held}
                  />
                  {view !== "timeline" && (selection.selecting || running) && (
                    <BulkBar
                      selection={held}
                      bulk={bulk}
                      work={work}
                      narrow={narrow}
                      announce={setAnnouncement}
                      bannerRef={banner}
                      dockRef={dock}
                    />
                  )}
                </div>
              )}
            </WorkViewTabs>
          </div>
        </Page>
      </div>
      {route.mode !== "closed" && !loading && event.state.kind === "loaded" && (
        <Suspense fallback={null}>
          <RecordPanel
            work={work}
            eventId={eventId}
            route={
              route.mode === "create"
                ? { mode: "create", kind: route.kind }
                : { mode: "open", readableId: route.id }
            }
            me={me}
            mayCreate={mayCreate}
            focusTitle={openId !== undefined && openId === made}
            onCreated={(readableId) => {
              setMade(readableId);
              record.open(readableId);
            }}
            writable={writable}
            timeZone={event.state.event.time_zone}
            today={today}
            previous={around?.previous}
            next={around?.next}
            place={around?.place}
            onOpen={(readableId) => {
              setMade(undefined);
              record.open(readableId);
            }}
            onClose={() => {
              setMade(undefined);
              record.close();
            }}
            returnFocusTo={returnFocus}
          />
        </Suspense>
      )}
    </div>
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
