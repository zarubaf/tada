import { IconEyeOff } from "@tabler/icons-react";
import { useId } from "react";
import { t } from "../i18n";
import { Select } from "../ui/Select";
import { type StatusGlyph, StatusIcon, type StatusTone } from "../ui/StatusIcon";
import styles from "./Board.module.css";
import { useBoardView } from "./boardView";
import type { BoardColumn } from "./recordStatus";
import { WorkCard } from "./WorkCard";
import { groupByColumn } from "./workFilters";
import type { WorkItem } from "./workItem";

/** The icon of each column: the icon of the status group (ADR 0082). */
const COLUMN_ICONS: Record<BoardColumn, { glyph: StatusGlyph; tone: StatusTone }> = {
  open: { glyph: "open", tone: "neutral" },
  "in-progress": { glyph: "progress", tone: "progress" },
  blocked: { glyph: "blocked", tone: "danger" },
  done: { glyph: "done", tone: "success" },
  canceled: { glyph: "canceled", tone: "muted" },
};

export interface BoardProps {
  /** The items that the filters let through. */
  items: WorkItem[];
  eventId: string;
  /** The civil date of today. */
  today: string;
  workstreamName: (workstreamId: string) => string | undefined;
  /** Below 40rem the board is one list with a status picker (ADR 0023). */
  narrow: boolean;
}

interface Group {
  column: BoardColumn;
  items: WorkItem[];
}

function Cards({ group, ...rest }: Omit<BoardProps, "items" | "narrow"> & { group: Group }) {
  const { eventId, today, workstreamName } = rest;
  return group.items.length === 0 ? (
    <p className={styles.empty}>{t("work-board-column-empty")}</p>
  ) : (
    <ul className={styles.cards}>
      {group.items.map((item) => (
        <WorkCard
          key={item.id}
          item={item}
          eventId={eventId}
          today={today}
          workstreamName={
            item.workstreamId === undefined ? undefined : workstreamName(item.workstreamId)
          }
        />
      ))}
    </ul>
  );
}

/**
 * The count of a column. It shows as a number; the name of the heading says „4 Einträge“ after a
 * comma, so that it does not read „Offen4“.
 */
function Count({ items }: { items: number }) {
  return (
    <>
      <span className={styles.count} aria-hidden="true">
        {items}
      </span>
      <span className={styles.hidden}>, {t("work-count", { count: items })}</span>
    </>
  );
}

function Column({ group, ...rest }: Omit<BoardProps, "items" | "narrow"> & { group: Group }) {
  const titleId = useId();
  const { glyph, tone } = COLUMN_ICONS[group.column];
  return (
    <section className={styles.column} aria-labelledby={titleId}>
      <header className={styles.head}>
        <StatusIcon glyph={glyph} tone={tone} size={14} />
        <h2 id={titleId} className={styles.name}>
          {t(`work-board-column-${group.column}`)}
          <Count items={group.items.length} />
        </h2>
      </header>
      <Cards group={group} {...rest} />
    </section>
  );
}

/**
 * „Abgebrochen“: a rail that opens as a column. One button does both, so it keeps focus and says
 * `aria-expanded`; it sits in the heading, as the disclosure pattern asks.
 */
function CanceledColumn({
  group,
  shown,
  onToggle,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { group: Group; shown: boolean; onToggle: () => void }) {
  const titleId = useId();
  const panelId = useId();
  return (
    <section className={shown ? styles.column : styles.rail} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.canceledHead}>
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={shown}
          aria-controls={panelId}
          onClick={onToggle}
        >
          <StatusIcon glyph="canceled" tone="muted" size={14} />
          <span className={styles.toggleLabel}>{t("work-board-column-canceled")}</span>
          <Count items={group.items.length} />
          {shown && (
            <IconEyeOff className={styles.hideIcon} size={14} stroke={1.5} aria-hidden="true" />
          )}
        </button>
      </h2>
      <div id={panelId} hidden={!shown}>
        {shown && <Cards group={group} {...rest} />}
      </div>
    </section>
  );
}

function WideBoard({
  groups,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { groups: Group[] }) {
  const [view, setView] = useBoardView();
  return (
    <div className={styles.board}>
      {groups.map((group) =>
        group.column === "canceled" ? (
          <CanceledColumn
            key={group.column}
            group={group}
            shown={view.canceled}
            onToggle={() => setView({ canceled: !view.canceled })}
            {...rest}
          />
        ) : (
          <Column key={group.column} group={group} {...rest} />
        ),
      )}
    </div>
  );
}

function NarrowBoard({
  groups,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { groups: Group[] }) {
  const [view, setView] = useBoardView();
  // Without a choice the list shows the first status that has records, so that a search that
  // matches only done records does not open on an empty „Offen“.
  const chosen = view.column ?? (groups.find((g) => g.items.length > 0) ?? groups[0])?.column;
  const group = groups.find((candidate) => candidate.column === chosen);
  return (
    <div className={styles.narrow}>
      <Select
        label={t("work-board-status")}
        value={chosen}
        onChange={(id) => setView({ column: id as BoardColumn })}
        options={groups.map((g) => ({
          id: g.column,
          label: `${t(`work-board-column-${g.column}`)} (${g.items.length})`,
        }))}
      />
      {group && <Cards group={group} {...rest} />}
    </div>
  );
}

/**
 * The board of the „Arbeit“ page (ADR 0082): the columns Offen, In Arbeit, Blockiert and Erledigt,
 * and Abgebrochen collapsed. Below 40rem it is one list with a status picker. It has no drag and
 * drop; a status changes in the record.
 */
export function Board({ items, narrow, ...rest }: BoardProps) {
  const groups = groupByColumn(items);
  return narrow ? (
    <NarrowBoard groups={groups} {...rest} />
  ) : (
    <WideBoard groups={groups} {...rest} />
  );
}
