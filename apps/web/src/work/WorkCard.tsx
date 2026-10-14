import { useId } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { Avatar } from "../ui/Avatar";
import { Checkbox } from "../ui/Checkbox";
import { Chip } from "../ui/Chip";
import { RecordId } from "../ui/RecordId";
import { redactedOr } from "../ui/Redacted";
import { StatusLabel } from "../ui/StatusLabel";
import { TypeIcon } from "../ui/TypeIcon";
import { presentationOf } from "./recordStatus";
import type { WorkSelection } from "./useWorkSelection";
import styles from "./WorkCard.module.css";
import { mayBeOverdue, type WorkItem } from "./workItem";
import { useRecordHref } from "./workLinks";

export interface WorkCardProps {
  item: WorkItem;
  eventId: string;
  /** The civil date of today, for the overdue state of the date. */
  today: string;
  /** The name of the workstream of the item, if it has one. */
  workstreamName?: string | undefined;
  /** Present if the member may select the record (it can be changed). */
  select?: CardSelection | undefined;
}

export interface CardSelection {
  selected: boolean;
  /** True while any record is selected: then every checkbox shows. */
  selecting: boolean;
  /** True while a bulk change runs on the selection. */
  locked?: boolean;
  onChange: (selected: boolean) => void;
}

/** The selection of one card, or nothing if the member may not select the record. */
export function cardSelect(
  selection: WorkSelection | undefined,
  item: WorkItem,
): CardSelection | undefined {
  return selection?.canSelect(item)
    ? {
        selected: selection.isSelected(item),
        selecting: selection.selecting,
        locked: selection.locked,
        onChange: (on) => selection.set([item], on),
      }
    : undefined;
}

/** The ids of the parts that the card names or describes itself by. */
function partIds(base: string) {
  return {
    id: `${base}-id`,
    title: `${base}-title`,
    kind: `${base}-kind`,
    status: `${base}-status`,
    owner: `${base}-owner`,
    due: `${base}-due`,
    area: `${base}-area`,
  };
}

/**
 * One work record on the board (doc/design/components.md, „Work card“): kind and ID, owner, title
 * and chips. The whole card is one link to the record. A risk has no page yet, so its card has the
 * same look and is a group, not a link. The name is the ID and the title; kind, status, owner,
 * date and area are the description, each part its own element so that they read apart.
 */
export function WorkCard({ item, eventId, today, workstreamName, select }: WorkCardProps) {
  const ids = partIds(useId());
  const href = useRecordHref(eventId)(item);
  const { glyph, tone, labelKey } = presentationOf(item);
  const hasOwner = item.owner.name !== "";
  const hasArea = workstreamName !== undefined;
  const describedBy = [
    ids.kind,
    ids.status,
    hasOwner && ids.owner,
    item.date !== undefined && ids.due,
    hasArea && ids.area,
  ]
    .filter(Boolean)
    .join(" ");
  const naming = {
    "aria-labelledby": `${ids.id} ${ids.title}`,
    "aria-describedby": describedBy,
  };
  const body = (
    <>
      <span className={styles.top}>
        <span className={styles.lead}>
          <TypeIcon kind={item.kind} size={14} decorative />
        </span>
        <span id={ids.kind} className={styles.hidden}>
          {t(`record-kind-${item.kind}`)}
        </span>
        <span id={ids.id}>
          <RecordId>{item.readableId}</RecordId>
        </span>
        <span className={styles.owner}>
          {hasOwner && (
            <span id={ids.owner} className={styles.hidden}>
              {t("work-card-owner", { name: item.owner.name })}
            </span>
          )}
          <Avatar name={item.owner.name} id={item.owner.id} size={20} decorative />
        </span>
      </span>
      <span id={ids.title} className={styles.title}>
        {redactedOr(item.redacted, item.title)}
      </span>
      <span className={styles.chips}>
        <span id={ids.status}>
          <StatusLabel icon={glyph} tone={tone}>
            {t(labelKey)}
          </StatusLabel>
        </span>
        {item.date !== undefined && (
          <span id={ids.due}>
            <span className={styles.hidden}>
              {item.kind === "decision" ? t("work-card-decided") : t("work-card-due")}
            </span>{" "}
            <Chip variant="date" date={item.date} today={today} overdue={mayBeOverdue(item)} />
          </span>
        )}
        {hasArea && (
          <span id={ids.area}>
            <Chip variant="workstream">{workstreamName}</Chip>
          </span>
        )}
      </span>
    </>
  );
  return (
    <li
      className={styles.item}
      {...(select && { "data-selectable": "" })}
      {...(select?.selecting && { "data-selecting": "" })}
      {...(select?.selected && { "data-selected": "" })}
    >
      {select && (
        // A sibling of the link and before it, never inside it: the card stays one link. The
        // CSS shows the box on hover, on focus within, on touch and while a selection exists.
        <span className={styles.check}>
          <Checkbox
            label={t("work-select-item", { id: item.readableId, title: item.title })}
            labelHidden
            isSelected={select.selected}
            isDisabled={select.locked}
            onChange={select.onChange}
          />
        </span>
      )}
      {href === undefined ? (
        // biome-ignore lint/a11y/useSemanticElements: a fieldset groups form controls, not a card
        <div role="group" className={styles.card} {...naming}>
          {body}
        </div>
      ) : (
        <Link to={href} className={`${styles.card} ${styles.link}`} {...naming}>
          {body}
        </Link>
      )}
    </li>
  );
}
