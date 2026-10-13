import { IconDiamond } from "@tabler/icons-react";
import { useId } from "react";
import { formatDate } from "../facts/formatValue";
import { t } from "../i18n";
import { StandaloneLink } from "../ui/StandaloneLink";
import { ActionStatusLabel, MilestoneStatusLabel } from "../work/WorkStatus";
import { type TimelineLayout, weekGroups } from "./layout";
import styles from "./Timeline.module.css";
import { actionWhen, laneName, titleOf } from "./text";

/**
 * The timeline as a list grouped by week (doc/design/layout-and-responsiveness.md, „Timeline“):
 * the layout of narrow containers. Each row shows its status as text.
 */
export function TimelineList({
  timeline,
  base,
  nameOf,
}: {
  timeline: TimelineLayout;
  /** The path of the event. */
  base: string;
  nameOf: (userId: string) => string;
}) {
  const id = useId();
  return (
    <section className={styles.list} aria-label={t("timeline-by-week")}>
      {weekGroups(timeline).map(({ week, entries }) => (
        <div key={week} className={styles.week}>
          <h3 id={`${id}-${week}`} className={styles.weekHeading}>
            {t("timeline-week", { date: formatDate(week) })}
          </h3>
          <ul className={styles.entries} aria-labelledby={`${id}-${week}`}>
            {entries.map((entry) =>
              entry.kind === "action" ? (
                <li key={entry.item.action.id} className={styles.entry}>
                  <StandaloneLink to={`${base}/actions`} className={styles.entryTitle}>
                    <span className={styles.recordId}>{entry.item.action.local_id}</span>{" "}
                    {titleOf(entry.item.action)}
                  </StandaloneLink>
                  <span className={styles.meta}>
                    <span className={styles.dates}>{actionWhen(entry.item.action)}</span>
                    <span>{laneName(entry.workstream)}</span>
                    <span>{nameOf(entry.item.action.owner_user_id)}</span>
                    <ActionStatusLabel status={entry.item.action.status} />
                  </span>
                </li>
              ) : (
                <li key={entry.milestone.id} className={styles.entry}>
                  <StandaloneLink to={`${base}/milestones`} className={styles.entryTitle}>
                    <IconDiamond
                      className={styles.entryIcon}
                      size={16}
                      stroke={1.5}
                      aria-hidden="true"
                    />
                    <span className={styles.recordId}>{entry.milestone.local_id}</span>{" "}
                    {titleOf(entry.milestone)}
                  </StandaloneLink>
                  <span className={styles.meta}>
                    <span className={styles.dates}>{formatDate(entry.milestone.date)}</span>
                    <span>{nameOf(entry.milestone.owner_user_id)}</span>
                    <MilestoneStatusLabel status={entry.milestone.status} />
                  </span>
                </li>
              ),
            )}
          </ul>
        </div>
      ))}
    </section>
  );
}
