import { useMemo } from "react";
import type { Api, Workstream } from "../api/client";
import { useEventProfile } from "../events/eventContext";
import { todayLocal } from "../facts/dueDate";
import { t } from "../i18n";
import { isEmpty } from "../timeline/layout";
import { TimelinePlan, windowOf } from "../timeline/TimelinePlan";
import { todayIn } from "../timeline/today";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { SkeletonLines } from "../ui/Skeleton";
import { StandaloneLink } from "../ui/StandaloneLink";
import { FailureBanner } from "./FailureBanner";
import { onlyOffTimeline, timelineRecords } from "./timelineRecords";
import { isFiltered } from "./WorkFilterControls";
import type { WorkFilters } from "./workFilters";
import type { WorkItem } from "./workItem";

export interface WorkTimelineProps {
  api: Api;
  eventId: string;
  /** The work that the filters let through, of every kind. */
  items: WorkItem[];
  filters: WorkFilters;
  workstreams: Workstream[];
  /** The time zone of the event, so that „heute“ is its day. */
  timeZone: string | undefined;
  /** Clears all filters. */
  onReset: () => void;
}

/**
 * The timeline view of the „Arbeit“ page (ADR 0082): the dated actions and the milestones of the
 * filtered work on the plan of the event. The facts of the event (its window) load when the view
 * opens, not with the page. Other kinds are not on the timeline.
 */
export default function WorkTimeline(props: WorkTimelineProps) {
  if (onlyOffTimeline(props.filters.kinds)) {
    return (
      <EmptyState
        title={t("work-timeline-only-title")}
        text={t("work-timeline-only")}
        action={<Button onPress={props.onReset}>{t("work-filter-reset")}</Button>}
      />
    );
  }
  return <LoadedTimeline {...props} />;
}

function LoadedTimeline({
  api,
  eventId,
  items,
  filters,
  workstreams,
  timeZone,
  onReset,
}: WorkTimelineProps) {
  const { profile, reloadProfile } = useEventProfile(api, eventId);
  const { actions, milestones } = useMemo(() => timelineRecords(items), [items]);
  const names = useMemo(
    () => new Map(items.map((item) => [item.owner.id, item.owner.name])),
    [items],
  );
  const base = `/events/${encodeURIComponent(eventId)}`;
  // The window decides whether there is anything to show: wait for it, so that no empty state flashes.
  const waiting = profile.kind === "loading";
  const empty = !waiting && isEmpty({ actions, milestones, window: windowOf(profile) });
  const filtered = isFiltered(filters);
  return (
    <TimelinePlan
      actions={actions}
      milestones={milestones}
      workstreams={workstreams}
      profile={profile}
      today={timeZone === undefined ? todayLocal() : todayIn(timeZone)}
      base={base}
      nameOf={(userId) => names.get(userId) ?? ""}
      filtered={filtered}
      lead={
        profile.kind === "failed" && (
          <FailureBanner
            message={t("work-timeline-failed")}
            detail={profile.message}
            busy={false}
            onRetry={() => void reloadProfile()}
          />
        )
      }
      fallback={
        waiting ? (
          <SkeletonLines label={t("work-loading")} />
        ) : empty && filtered ? (
          <EmptyState
            title={t("work-timeline-only-title")}
            text={t("timeline-no-match")}
            action={<Button onPress={onReset}>{t("work-filter-reset")}</Button>}
          />
        ) : empty ? (
          <EmptyState
            title={t("timeline-empty-title")}
            text={t("timeline-empty-text")}
            action={
              <StandaloneLink to={`${base}/actions`}>{t("timeline-empty-action")}</StandaloneLink>
            }
          />
        ) : undefined
      }
    />
  );
}
