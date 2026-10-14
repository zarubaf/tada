import { IconDiamond } from "@tabler/icons-react";
import { type ReactNode, useRef } from "react";
import type { Action, Milestone, Workstream } from "../api/client";
import type { ProfileState } from "../events/eventContext";
import { formatDate, formatLabel, formatValue } from "../facts/formatValue";
import { t } from "../i18n";
import { KnowledgeState } from "../ui/KnowledgeState";
import { StandaloneLink } from "../ui/StandaloneLink";
import { type EventWindow, eventWindow, layout } from "./layout";
import styles from "./Timeline.module.css";
import { TimelineChart } from "./TimelineChart";
import { TimelineList } from "./TimelineList";
import { useWide } from "./useWide";

/** The event window of a profile, or nothing while the profile loads or has no window. */
export function windowOf(profile: ProfileState): EventWindow | null {
  return profile.kind === "loaded" ? eventWindow(profile.profile.facts) : null;
}

export interface TimelinePlanProps {
  /** The actions to place; the caller has applied its filters. */
  actions: Action[];
  milestones: Milestone[];
  workstreams: Workstream[];
  profile: ProfileState;
  /** Today as a civil date in the time zone of the event. */
  today: string;
  /** The path of the event. */
  base: string;
  nameOf: (userId: string) => string;
  /** The filters are active: the plan says that they hide everything, and the list stays quiet. */
  filtered: boolean;
  /** Shown first, whatever else the plan shows: a heading, a live region or the filters. */
  lead?: ReactNode;
  /** Shown instead of the legend, the axis and the list: a skeleton or an empty state. */
  fallback?: ReactNode;
}

/**
 * The plan of an event: the legend, the time axis on wide containers and the list by week on
 * narrow ones (doc/design/layout-and-responsiveness.md, „Timeline“). It takes the records as
 * props, so that the page „Zeitplan“ and the timeline view of „Arbeit“ share it. It is the
 * container that decides between axis and list, so `lead` and `fallback` sit inside it.
 */
export function TimelinePlan({
  actions,
  milestones,
  workstreams,
  profile,
  today,
  base,
  nameOf,
  filtered,
  lead,
  fallback,
}: TimelinePlanProps) {
  const root = useRef<HTMLDivElement>(null);
  const wide = useWide(root);
  const dates = windowOf(profile);
  const timeline = layout({ actions, milestones, workstreams, window: dates, today });
  const placed = timeline.lanes.some((lane) => lane.items.length > 0);
  return (
    <div ref={root} className={styles.timeline}>
      {lead}
      {fallback ?? (
        <>
          <Legend
            dates={dates}
            profile={profile}
            today={today}
            undated={timeline.undated}
            base={base}
          />
          {!placed && filtered && <p className={styles.noMatch}>{t("timeline-no-match")}</p>}
          {timeline.weeks.length > 0 && (
            <>
              <div className={styles.chart}>
                <TimelineChart
                  timeline={timeline}
                  base={base}
                  assumption={dates?.state === "assumption"}
                />
              </div>
              <TimelineList
                timeline={timeline}
                base={base}
                nameOf={nameOf}
                focusable={!wide}
                filtered={filtered}
              />
            </>
          )}
        </>
      )}
    </div>
  );
}

/** What the band, the line and the diamond mean, and the actions that have no place. */
function Legend({
  dates,
  profile,
  today,
  undated,
  base,
}: {
  dates: EventWindow | null;
  profile: ProfileState;
  today: string;
  undated: number;
  base: string;
}) {
  const key = dates?.field_key ?? "date_window";
  const field = profile.kind === "loaded" ? profile.fields.find((f) => f.key === key) : undefined;
  return (
    <ul className={styles.legend} aria-label={t("timeline-legend")}>
      {profile.kind === "loaded" && (
        <li className={styles.legendItem}>
          <span className={styles.bandSwatch} aria-hidden="true" />
          <span className={styles.legendName}>
            {field ? formatLabel(field.label) : t(`field-${key}`)}
          </span>
          {dates ? (
            <KnowledgeState state={dates.state}>
              {formatValue(dates.fact, field?.value_type)}
            </KnowledgeState>
          ) : (
            <KnowledgeState state="unknown" />
          )}
        </li>
      )}
      <li className={styles.legendItem}>
        <span className={styles.todaySwatch} aria-hidden="true" />
        <span className={styles.legendName}>{t("timeline-today")}</span>
        <span className={styles.dates}>{formatDate(today)}</span>
      </li>
      <li className={styles.legendItem}>
        <IconDiamond className={styles.entryIcon} size={16} stroke={1.5} aria-hidden="true" />
        <span className={styles.legendName}>{t("timeline-milestones")}</span>
      </li>
      {undated > 0 && (
        <li className={styles.legendItem}>
          <StandaloneLink to={`${base}/actions`}>
            {t("timeline-undated", { count: undated })}
          </StandaloneLink>
        </li>
      )}
    </ul>
  );
}
