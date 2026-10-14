import { useRef, useState } from "react";
import type { Action, ActionStatus, Api } from "../api/client";
import { useEventContext } from "../events/eventContext";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit } from "../ui/focus";
import { LiveRegion } from "../ui/LiveRegion";
import { Select } from "../ui/Select";
import { SkeletonLines } from "../ui/Skeleton";
import { StandaloneLink } from "../ui/StandaloneLink";
import { DirectoryGate } from "../work/DirectoryGate";
import { type Directory, useDirectory } from "../work/directory";
import { isEmpty } from "./layout";
import styles from "./Timeline.module.css";
import { TimelinePlan, windowOf } from "./TimelinePlan";
import { todayIn } from "./today";

const STATUSES: readonly ActionStatus[] = ["open", "in-progress", "blocked", "done", "canceled"];
const ALL = "all";

/**
 * „Zeitplan“ of an event (doc/design/layout-and-responsiveness.md, „Timeline“): the dated actions
 * in one lane for each workstream, the milestones, the event window and today. Read-only. The
 * plan itself is `TimelinePlan`, which the timeline view of „Arbeit“ shows too.
 */
export function TimelinePage({ api }: { api: Api }) {
  const { event } = useEventContext();
  const { state, reload } = useDirectory(api, event.id, { milestones: true, actions: true });
  return (
    <DirectoryGate state={state} reload={reload}>
      {(directory) => <Timeline directory={directory} />}
    </DirectoryGate>
  );
}

function Timeline({ directory }: { directory: Directory }) {
  const { event, profile } = useEventContext();
  const [owner, setOwner] = useState(ALL);
  const [status, setStatus] = useState(ALL);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusAfterCommit = useFocusAfterCommit();
  const base = `/events/${encodeURIComponent(event.id)}`;

  const dates = windowOf(profile);
  const { actions, milestones, workstreams } = directory;
  const shown = actions.filter(
    (action) =>
      (owner === ALL || action.owner_user_id === owner) &&
      (status === ALL || action.status === status),
  );
  // Without any date the page shows its empty state, whatever the filters hide.
  const empty = isEmpty({ actions, milestones, window: dates });

  const filtered = owner !== ALL || status !== ALL;
  const hidden = actions.length - shown.length;
  const reset = () => {
    setOwner(ALL);
    setStatus(ALL);
    // The reset button leaves with the filters; focus goes to the heading of the page.
    focusAfterCommit(() => heading.current);
  };
  const plan = profile.kind !== "loading" && !empty;

  return (
    // The filters hide actions; the milestones and the window stay as the frame of the plan.
    <TimelinePlan
      actions={shown}
      milestones={milestones}
      workstreams={workstreams}
      profile={profile}
      today={todayIn(event.time_zone)}
      base={base}
      nameOf={directory.nameOf}
      filtered={filtered}
      lead={
        <>
          <LiveRegion kind="status">
            {filtered && hidden > 0 ? t("timeline-hidden", { count: hidden }) : ""}
          </LiveRegion>
          <h2 ref={heading} tabIndex={-1} className={styles.heading}>
            {t("timeline-title")}
          </h2>
          {plan && (
            <fieldset className={styles.toolbar} aria-label={t("timeline-filters")}>
              <Select
                label={t("timeline-filter-owner")}
                options={[
                  { id: ALL, label: t("timeline-filter-all") },
                  ...owners(actions, directory),
                ]}
                value={owner}
                onChange={setOwner}
              />
              <Select
                label={t("timeline-filter-status")}
                options={[
                  { id: ALL, label: t("timeline-filter-all") },
                  ...STATUSES.map((id) => ({ id, label: t(`action-status-${id}`) })),
                ]}
                value={status}
                onChange={setStatus}
              />
              {filtered && (
                <span className={styles.filterState}>
                  {hidden > 0 && (
                    <span className={styles.hiddenCount}>
                      {t("timeline-hidden", { count: hidden })}
                    </span>
                  )}
                  <Button variant="quiet" onPress={reset}>
                    {t("timeline-filter-reset")}
                  </Button>
                </span>
              )}
            </fieldset>
          )}
        </>
      }
      fallback={
        plan ? undefined : profile.kind === "loading" ? (
          // The window decides whether the page is empty: wait for it, so that no empty state flashes.
          <SkeletonLines label={t("work-loading")} />
        ) : (
          <EmptyState
            title={t("timeline-empty-title")}
            text={t("timeline-empty-text")}
            action={
              <StandaloneLink to={`${base}/actions`}>{t("timeline-empty-action")}</StandaloneLink>
            }
          />
        )
      }
    />
  );
}

/** The owners of the actions, by name. */
function owners(actions: Action[], directory: Directory) {
  const ids = [...new Set(actions.map((action) => action.owner_user_id))];
  return ids
    .map((id) => ({ id, label: directory.nameOf(id) || id }))
    .sort((a, b) => a.label.localeCompare(b.label, "de-CH"));
}
