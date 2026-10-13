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
  focusable,
  filtered,
}: {
  timeline: TimelineLayout;
  /** The path of the event. */
  base: string;
  nameOf: (userId: string) => string;
  /** False where the axis shows the same items and takes the keyboard: the links leave the tab order. */
  focusable: boolean;
  /** The filters are active: the page already says that they hide everything. */
  filtered: boolean;
}) {
  const id = useId();
  const groups = weekGroups(timeline);
  const tabIndex = focusable ? undefined : -1;
  return (
    <section className={styles.list} aria-label={t("timeline-by-week")}>
      {groups.length === 0 && !filtered && (
        <p className={styles.noMatch}>{t("timeline-list-empty")}</p>
      )}
      {groups.map(({ week, entries }) => (
        <div key={week} className={styles.week}>
          <h3 id={`${id}-${week}`} className={styles.weekHeading}>
            {t("timeline-week", { date: formatDate(week) })}
          </h3>
          <ul className={styles.entries} aria-labelledby={`${id}-${week}`}>
            {entries.map((entry) =>
              entry.kind === "action" ? (
                <li key={entry.item.action.id} className={styles.entry}>
                  <StandaloneLink
                    to={`${base}/actions#${entry.item.action.local_id}`}
                    className={styles.entryTitle}
                    tabIndex={tabIndex}
                  >
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
                  <StandaloneLink
                    to={`${base}/milestones#${entry.milestone.local_id}`}
                    className={styles.entryTitle}
                    tabIndex={tabIndex}
                  >
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
