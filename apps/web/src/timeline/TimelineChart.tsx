import type { CSSProperties } from "react";
import { useId } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import type { TimelineItem, TimelineLayout } from "./layout";
import styles from "./Timeline.module.css";
import { actionLabel, laneName, milestoneLabel, titleOf } from "./text";

/** Custom properties that place an element on the axis. The CSS turns days into lengths. */
function place(values: Record<string, number>): CSSProperties {
  return Object.fromEntries(
    Object.entries(values).map(([name, value]) => [`--${name}`, String(value)]),
  ) as CSSProperties;
}

/** A week label: the Monday, with the year on the first week and the first week of a year. */
function weekLabel(week: string, index: number): string {
  const [year, month, day] = week.split("-");
  const startsYear = index === 0 || (month === "01" && Number(day) <= 7);
  return startsYear ? `${day}.${month}.${year}` : `${day}.${month}.`;
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
  const id = useId();
  const days = timeline.weeks.length * 7;
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
          {timeline.weeks.map((week, index) => (
            <div key={week} className={styles.weekLine} style={place({ at: index * 7 })} />
          ))}
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
            {timeline.weeks.map((week, index) => (
              <span key={week} className={styles.weekLabel} style={place({ at: index * 7 })}>
                {weekLabel(week, index)}
              </span>
            ))}
            {timeline.today !== null && (
              <span className={styles.todayTag} style={place({ at: timeline.today })}>
                {t("timeline-today")}
              </span>
            )}
          </div>
        </div>

        {timeline.milestones.length > 0 && (
          <section className={styles.lane} aria-labelledby={`${id}-milestones`}>
            <h3 id={`${id}-milestones`} className={styles.laneHeading}>
              <span className={styles.laneName}>{t("timeline-milestones")}</span>
            </h3>
            <ul className={styles.rows}>
              {timeline.milestones.map(({ milestone, at }) => (
                <li key={milestone.id} className={styles.row}>
                  <span className={styles.label} aria-hidden="true">
                    <span className={styles.recordId}>{milestone.local_id}</span>
                    <span className={styles.rowTitle}>{titleOf(milestone)}</span>
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
          </section>
        )}

        {timeline.lanes.map((lane) => {
          const laneId = `${id}-${lane.workstream?.id ?? "none"}`;
          return (
            <section key={laneId} className={styles.lane} aria-labelledby={laneId}>
              <h3 id={laneId} className={styles.laneHeading}>
                <span className={styles.laneName}>{laneName(lane.workstream)}</span>
              </h3>
              <ul className={styles.rows}>
                {lane.items.map((item) => (
                  <Row key={item.action.id} item={item} base={base} />
                ))}
              </ul>
            </section>
          );
        })}
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
      </span>
      <span className={styles.track}>
        <span
          className={item.kind === "bar" ? styles.bar : styles.point}
          data-status={action.status}
          style={place(item.kind === "bar" ? { from: item.from, to: item.to } : { at: item.at })}
        >
          <Link to={`${base}/actions`} className={styles.mark} aria-label={actionLabel(action)}>
            <span className={styles.markShape} />
          </Link>
        </span>
      </span>
    </li>
  );
}
