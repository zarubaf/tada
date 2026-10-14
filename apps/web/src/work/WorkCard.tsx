import { t } from "../i18n";
import { Link } from "../router/Router";
import { Avatar } from "../ui/Avatar";
import { Chip } from "../ui/Chip";
import { RecordId } from "../ui/RecordId";
import { redactedOr } from "../ui/Redacted";
import { StatusLabel } from "../ui/StatusLabel";
import { TypeIcon } from "../ui/TypeIcon";
import { statusPresentation } from "./recordStatus";
import styles from "./WorkCard.module.css";
import type { WorkItem } from "./workItem";
import { recordHref } from "./workLinks";

export interface WorkCardProps {
  item: WorkItem;
  eventId: string;
  /** The civil date of today, for the overdue state of the date. */
  today: string;
  /** The name of the workstream of the item, if it has one. */
  workstreamName?: string | undefined;
}

/** A closed record is never overdue; neither is the day on which a decision was made. */
function mayBeOverdue(item: WorkItem): boolean {
  return item.kind !== "decision" && item.column !== "done" && item.column !== "canceled";
}

function Chips({ item, today, workstreamName }: Omit<WorkCardProps, "eventId">) {
  const { glyph, tone, labelKey } = statusPresentation(item.kind, item.status as never);
  return (
    <span className={styles.chips}>
      <StatusLabel icon={glyph} tone={tone}>
        {t(labelKey)}
      </StatusLabel>
      {item.date !== undefined && (
        <>
          <span className={styles.hidden}>
            {item.kind === "decision" ? t("work-card-decided") : t("work-card-due")}{" "}
          </span>
          <Chip variant="date" date={item.date} today={today} overdue={mayBeOverdue(item)} />
        </>
      )}
      {workstreamName !== undefined && <Chip variant="workstream">{workstreamName}</Chip>}
    </span>
  );
}

/**
 * One work record on the board (doc/design/components.md, „Work card“): kind and ID, owner, title
 * and chips. The whole card is one link to the record. A risk has no page yet, so its card has the
 * same look and is no link.
 */
export function WorkCard({ item, eventId, ...rest }: WorkCardProps) {
  const href = recordHref(eventId, item);
  const body = (
    <>
      <span className={styles.top}>
        <TypeIcon kind={item.kind} size={14} decorative />
        <span className={styles.hidden}>{t(`record-kind-${item.kind}`)} </span>
        <RecordId>{item.readableId}</RecordId>
        <span className={styles.owner}>
          {item.owner.name !== "" && (
            <span className={styles.hidden}>{t("work-card-owner", { name: item.owner.name })}</span>
          )}
          <Avatar name={item.owner.name} id={item.owner.id} size={24} decorative />
        </span>
      </span>
      <span className={styles.title}>{redactedOr(item.redacted, item.title)}</span>
      <Chips item={item} {...rest} />
    </>
  );
  return (
    <li className={styles.item}>
      {href === undefined ? (
        <div className={styles.card}>{body}</div>
      ) : (
        <Link to={href} className={`${styles.card} ${styles.link}`}>
          {body}
        </Link>
      )}
    </li>
  );
}
