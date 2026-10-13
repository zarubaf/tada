import type { CSSProperties } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { ActionStatusLabel, MilestoneStatusLabel } from "../work/WorkStatus";
import { type AxisLabel, axisLabels, type TimelineItem, type TimelineLayout } from "./layout";
import styles from "./Timeline.module.css";
import { actionLabel, laneName, milestoneLabel, titleOf } from "./text";

/** Custom properties that place an element on the axis. The CSS turns days into lengths. */
function place(values: Record<string, number>): CSSProperties {
  return Object.fromEntries(
    Object.entries(values).map(([name, value]) => [`--${name}`, String(value)]),
  ) as CSSProperties;
}

const monthFormat = new Intl.DateTimeFormat("de-CH", { month: "short", timeZone: "UTC" });
const monthYearFormat = new Intl.DateTimeFormat("de-CH", {
  month: "short",
  year: "numeric",
  timeZone: "UTC",
});

/** The text of a label of the axis: the month, with the year where the layout says so. */
function labelText({ date, withYear }: AxisLabel): string {
  const day = new Date(`${date}T00:00:00Z`);
  return (withYear ? monthYearFormat : monthFormat).format(day);
}

/**
 * The lanes on the time axis (doc/design/layout-and-responsiveness.md, „Timeline“). The axis
 * scrolls inside its own region, so a span of years never scrolls the page. Each bar and point is
 * a link with the full name of its action; the lines and the band are decoration that the legend
 * names.
 */
export function TimelineChart({
  timeline,
  base,
  assumption,
}: {
  timeline: TimelineLayout;
  /** The path of the event. */
  base: string;
  /** The event window is an assumption: its band has dotted edges. */
  assumption: boolean;
}) {
  const days = timeline.weeks.length * 7;
  const labels = axisLabels(timeline.weeks);
  return (
    // A scrolled region must take the focus, so that the keyboard can scroll it.
    // biome-ignore lint/a11y/noNoninteractiveTabindex: the region scrolls; WCAG 2.1.1 needs the focus.
    <section className={styles.scroller} aria-label={t("timeline-axis")} tabIndex={0}>
      <div className={styles.canvas} style={place({ weeks: timeline.weeks.length, days })}>
        <div className={styles.overlay} aria-hidden="true">
          {timeline.window && (
            <div
              className={styles.band}
              data-band
              data-assumption={assumption || undefined}
              style={place({ from: timeline.window.from, to: timeline.window.to })}
            />
          )}
          {timeline.milestones.map(({ milestone, at }) => (
            <div key={milestone.id} className={styles.milestoneLine} style={place({ at })} />
          ))}
          {timeline.today !== null && (
            <div className={styles.todayLine} style={place({ at: timeline.today })} />
          )}
        </div>

        <div className={styles.axis} aria-hidden="true">
          <div className={styles.label} />
          <div className={styles.track}>
            {labels.map((label) => (
              <span key={label.date} className={styles.weekLabel} style={place({ at: label.at })}>
                {labelText(label)}
              </span>
            ))}
          </div>
        </div>

        {timeline.milestones.length > 0 && (
          <div className={styles.lane}>
            <h3 className={styles.laneHeading}>
              <span className={styles.laneName}>{t("timeline-milestones")}</span>
            </h3>
            <ul className={styles.rows}>
              {timeline.milestones.map(({ milestone, at }) => (
                <li key={milestone.id} className={styles.row}>
                  <span className={styles.label} aria-hidden="true">
                    <span className={styles.recordId}>{milestone.local_id}</span>
                    <span className={styles.rowTitle}>{titleOf(milestone)}</span>
                    <MilestoneStatusLabel status={milestone.status} />
                  </span>
                  <span className={styles.track}>
                    <span
                      className={styles.diamond}
                      data-status={milestone.status}
                      style={place({ at })}
                    />
                    <span className={styles.hidden}>{milestoneLabel(milestone)}</span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {timeline.lanes.map((lane) => {
          return (
            <div key={lane.workstream?.id ?? "none"} className={styles.lane}>
              <h3 className={styles.laneHeading}>
                <span className={styles.laneName}>{laneName(lane.workstream)}</span>
              </h3>
              <ul className={styles.rows}>
                {lane.items.map((item) => (
                  <Row key={item.action.id} item={item} base={base} />
                ))}
              </ul>
            </div>
          );
        })}

        {timeline.today !== null && (
          <div className={styles.footer} aria-hidden="true">
            <div className={styles.label} />
            <div className={styles.track}>
              <span className={styles.todayTag} style={place({ at: timeline.today })}>
                {t("timeline-today")}
              </span>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function Row({ item, base }: { item: TimelineItem; base: string }) {
  const { action } = item;
  return (
    <li className={styles.row}>
      {/* The link names the action; the label repeats it for the eye. */}
      <span className={styles.label} aria-hidden="true">
        <span className={styles.recordId}>{action.local_id}</span>
        <span className={styles.rowTitle}>{titleOf(action)}</span>
        <ActionStatusLabel status={action.status} />
      </span>
      <span className={styles.track}>
        <span
          className={item.kind === "bar" ? styles.bar : styles.point}
          data-status={action.status}
          style={place(item.kind === "bar" ? { from: item.from, to: item.to } : { at: item.at })}
        >
          <Link
            to={`${base}/actions#${action.local_id}`}
            className={styles.mark}
            aria-label={actionLabel(action)}
          >
            <span className={styles.markShape} />
          </Link>
        </span>
      </span>
    </li>
  );
}
