import { IconDiamond } from "@tabler/icons-react";
import { useRef, useState } from "react";
import type { Action, ActionStatus, Api } from "../api/client";
import { type ProfileState, useEventContext } from "../events/eventContext";
import { formatDate, formatLabel } from "../facts/formatValue";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { useFocusAfterCommit } from "../ui/focus";
import { KnowledgeState } from "../ui/KnowledgeState";
import { LiveRegion } from "../ui/LiveRegion";
import { Select } from "../ui/Select";
import { StandaloneLink } from "../ui/StandaloneLink";
import { DirectoryGate } from "../work/DirectoryGate";
import { type Directory, useDirectory } from "../work/directory";
import { type EventWindow, eventWindow, layout } from "./layout";
import styles from "./Timeline.module.css";
import { TimelineChart } from "./TimelineChart";
import { TimelineList } from "./TimelineList";

const STATUSES: readonly ActionStatus[] = ["open", "in-progress", "blocked", "done", "canceled"];
const ALL = "all";

/** Today as a civil date in the time zone of the event, so that „heute“ is the day of the event. */
function todayIn(timeZone: string, now = new Date()): string {
  try {
    // `en-CA` writes a date as 2030-05-18.
    return new Intl.DateTimeFormat("en-CA", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(now);
  } catch {
    return now.toISOString().slice(0, 10);
  }
}

/**
 * „Zeitplan“ of an event (doc/design/layout-and-responsiveness.md, „Timeline“): the dated actions
 * in one lane for each workstream, the milestones, the event window and today. Read-only.
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

  const dates = profile.kind === "loaded" ? eventWindow(profile.profile.facts) : null;
  const today = todayIn(event.time_zone);
  const { actions, milestones, workstreams } = directory;
  const shown = actions.filter(
    (action) =>
      (owner === ALL || action.owner_user_id === owner) &&
      (status === ALL || action.status === status),
  );
  // The filters hide actions; the milestones and the window stay as the frame of the plan.
  const timeline = layout({ actions: shown, milestones, workstreams, window: dates, today });
  // Without any date the page shows its empty state, whatever the filters hide.
  const complete = layout({ actions, milestones, workstreams, window: dates, today });

  const filtered = owner !== ALL || status !== ALL;
  const hidden = actions.length - shown.length;
  const placed = timeline.lanes.some((lane) => lane.items.length > 0);
  const reset = () => {
    setOwner(ALL);
    setStatus(ALL);
    // The reset button leaves with the filters; focus goes to the heading of the page.
    focusAfterCommit(() => heading.current);
  };

  return (
    <div className={styles.timeline}>
      <LiveRegion kind="status">
        {filtered && hidden > 0 ? t("timeline-hidden", { count: hidden }) : ""}
      </LiveRegion>
      <h2 ref={heading} tabIndex={-1} className={styles.heading}>
        {t("timeline-title")}
      </h2>
      {complete.weeks.length === 0 ? (
        <EmptyState
          title={t("timeline-empty-title")}
          text={t("timeline-empty-text")}
          action={
            <StandaloneLink to={`${base}/actions`}>{t("timeline-empty-action")}</StandaloneLink>
          }
        />
      ) : (
        <>
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
              <TimelineList timeline={timeline} base={base} nameOf={directory.nameOf} />
            </>
          )}
        </>
      )}
    </div>
  );
}

/** The owners of the actions, by name. */
function owners(actions: Action[], directory: Directory) {
  const ids = [...new Set(actions.map((action) => action.owner_user_id))];
  return ids
    .map((id) => ({ id, label: directory.nameOf(id) || id }))
    .sort((a, b) => a.label.localeCompare(b.label, "de-CH"));
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
              {`${formatDate(dates.start)} – ${formatDate(dates.end)}`}
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
