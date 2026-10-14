import { IconEyeOff } from "@tabler/icons-react";
import { useId, useState } from "react";
import { Button as AriaButton } from "react-aria-components";
import { t } from "../i18n";
import { IconButton } from "../ui/IconButton";
import { Select } from "../ui/Select";
import { type StatusGlyph, StatusIcon, type StatusTone } from "../ui/StatusIcon";
import styles from "./Board.module.css";
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

function Column({
  group,
  onHide,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { group: Group; onHide?: () => void }) {
  const titleId = useId();
  const { glyph, tone } = COLUMN_ICONS[group.column];
  return (
    <section className={styles.column} aria-labelledby={titleId}>
      <header className={styles.head}>
        <StatusIcon glyph={glyph} tone={tone} size={14} />
        <h2 id={titleId} className={styles.name}>
          {t(`work-board-column-${group.column}`)}
          <span className={styles.count}>{group.items.length}</span>
        </h2>
        {onHide && (
          <span className={styles.hide}>
            <IconButton icon={IconEyeOff} label={t("work-board-canceled-hide")} onPress={onHide} />
          </span>
        )}
      </header>
      <Cards group={group} {...rest} />
    </section>
  );
}

function WideBoard({
  groups,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { groups: Group[] }) {
  const [canceledShown, setCanceledShown] = useState(false);
  return (
    <div className={styles.board}>
      {groups.map((group) => {
        if (group.column !== "canceled") {
          return <Column key={group.column} group={group} {...rest} />;
        }
        return canceledShown ? (
          <Column
            key={group.column}
            group={group}
            onHide={() => setCanceledShown(false)}
            {...rest}
          />
        ) : (
          <AriaButton
            key={group.column}
            className={styles.rail}
            aria-expanded="false"
            onPress={() => setCanceledShown(true)}
          >
            <StatusIcon glyph="canceled" tone="muted" size={14} />
            <span className={styles.railLabel}>{t("work-board-column-canceled")}</span>
            <span className={styles.count}>{group.items.length}</span>
          </AriaButton>
        );
      })}
    </div>
  );
}

function NarrowBoard({
  groups,
  ...rest
}: Omit<BoardProps, "items" | "narrow"> & { groups: Group[] }) {
  const [chosen, setChosen] = useState<BoardColumn>("open");
  const group = groups.find((candidate) => candidate.column === chosen) ?? groups[0];
  return (
    <div className={styles.narrow}>
      <Select
        label={t("work-board-status")}
        value={chosen}
        onChange={(id) => setChosen(id as BoardColumn)}
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
