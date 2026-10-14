import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import { Link } from "../router/Router";
import { Avatar } from "../ui/Avatar";
import { Chip } from "../ui/Chip";
import { type Column, DataTable } from "../ui/DataTable";
import { LiveRegion } from "../ui/LiveRegion";
import { RecordId } from "../ui/RecordId";
import { redactedOr } from "../ui/Redacted";
import { StatusLabel } from "../ui/StatusLabel";
import { TypeIcon } from "../ui/TypeIcon";
import type { BoardProps } from "./Board";
import { presentationOf } from "./recordStatus";
import { WorkCard } from "./WorkCard";
import styles from "./WorkTable.module.css";
import { SORT_KEYS, type SortKey, sortBy } from "./workFilters";
import type { WorkItem } from "./workItem";
import { recordHref } from "./workLinks";
import { useWorkSort } from "./workView";

/**
 * The columns of the table, in order. A column is not always a sort key: the column to select joins
 * in front and has none.
 */
const COLUMN_KEYS = ["id", "kind", "title", "status", "owner", "due", "area"] as const;
type ColumnKey = (typeof COLUMN_KEYS)[number];

/** The sort key that a column id names, or nothing for a column that does not sort. */
function sortKeyOf(id: string): SortKey | undefined {
  return SORT_KEYS.find((key) => key === id);
}

function Title({ item, eventId }: { item: WorkItem; eventId: string }) {
  const href = recordHref(eventId, item);
  const title = redactedOr(item.redacted, item.title);
  // A long title ends in an ellipsis in its one line; the title attribute holds the whole text.
  const whole = item.redacted ? undefined : item.title;
  return href === undefined ? (
    <span className={styles.title} title={whole}>
      {title}
    </span>
  ) : (
    <Link to={href} className={styles.link} title={whole}>
      {title}
    </Link>
  );
}

function Due({ item, today }: { item: WorkItem; today: string }) {
  if (item.date === undefined) {
    return null;
  }
  // A closed record is never overdue; neither is the day on which a decision was made.
  const open = item.kind !== "decision" && item.column !== "done" && item.column !== "canceled";
  return (
    <>
      {item.kind === "decision" && <span className={styles.hidden}>{t("work-card-decided")} </span>}
      <Chip variant="date" date={item.date} today={today} overdue={open} />
    </>
  );
}

function columnsFor({
  eventId,
  today,
  workstreamName,
}: Pick<BoardProps, "eventId" | "today" | "workstreamName">): Column<WorkItem>[] {
  const cells: Record<ColumnKey, (item: WorkItem) => React.ReactNode> = {
    id: (item) => <RecordId>{item.readableId}</RecordId>,
    kind: (item) => <TypeIcon kind={item.kind} size={14} />,
    title: (item) => <Title item={item} eventId={eventId} />,
    status: (item) => {
      const { glyph, tone, labelKey } = presentationOf(item);
      return (
        <StatusLabel icon={glyph} tone={tone} column>
          {t(labelKey)}
        </StatusLabel>
      );
    },
    owner: (item) =>
      item.owner.name === "" ? null : (
        <span className={styles.owner}>
          <Avatar name={item.owner.name} id={item.owner.id} decorative />
          <span className={styles.ownerName} title={item.owner.name}>
            {item.owner.name}
          </span>
        </span>
      ),
    due: (item) => <Due item={item} today={today} />,
    area: (item) =>
      item.workstreamId === undefined ? null : (
        <span className={styles.area}>
          <Chip variant="workstream">{workstreamName(item.workstreamId)}</Chip>
        </span>
      ),
  };
  return COLUMN_KEYS.map((key) => ({
    id: key,
    header: t(`work-table-${key}`),
    cell: cells[key],
    sortable: sortKeyOf(key) !== undefined,
    ...(key === "id" ? { mono: true } : {}),
    ...(key === "title" ? { grow: true } : {}),
  }));
}

/** Tells the member the result of a sort, once the sort has changed and not on the first view. */
function useSortAnnouncement(text: string): string | undefined {
  const last = useRef(text);
  const [announcement, setAnnouncement] = useState<string>();
  useEffect(() => {
    if (last.current !== text) {
      last.current = text;
      setAnnouncement(text);
    }
  }, [text]);
  return announcement;
}

/**
 * The table view of the „Arbeit“ page (ADR 0082): one dense row for each record, sortable by each
 * column. Below 40rem there is no table, only the cards of the board in one list; they follow the
 * sort too.
 */
export default function WorkTable({ items, narrow, ...rest }: BoardProps) {
  const [sort, setSort] = useWorkSort();
  const { workstreamName } = rest;
  const rows = sortBy(items, sort, (item) =>
    item.workstreamId === undefined ? undefined : workstreamName(item.workstreamId),
  );
  const announcement = useSortAnnouncement(
    t("work-sorted", { column: t(`work-table-${sort.key}`), direction: sort.dir }),
  );
  const onSort = (key: string) => {
    const next = sortKeyOf(key);
    if (next === undefined) {
      return;
    }
    setSort({ key: next, dir: sort.key === next && sort.dir === "asc" ? "desc" : "asc" });
  };
  return (
    <>
      <LiveRegion kind="status" visuallyHidden>
        {announcement}
      </LiveRegion>
      {narrow ? (
        <ul className={styles.cards}>
          {rows.map((item) => (
            <WorkCard
              key={item.id}
              item={item}
              eventId={rest.eventId}
              today={rest.today}
              workstreamName={
                item.workstreamId === undefined ? undefined : workstreamName(item.workstreamId)
              }
            />
          ))}
        </ul>
      ) : (
        <DataTable
          label={t("work-table-label")}
          columns={columnsFor(rest)}
          rows={rows}
          rowKey={(item) => item.id}
          sort={{ columnId: sort.key, direction: sort.dir }}
          onSort={onSort}
          stacked={false}
        />
      )}
    </>
  );
}
