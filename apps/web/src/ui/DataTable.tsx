import { IconArrowDown, IconArrowUp } from "@tabler/icons-react";
import type { ReactNode } from "react";
import styles from "./DataTable.module.css";

/**
 * The header of a column: text, or a node such as a checkbox that selects all rows. A node has no
 * text of its own, so the column needs a `label`: its accessible name, and its name in a two-line row.
 */
type ColumnHeader = { header: string; label?: string } | { header: ReactNode; label: string };

export type Column<T> = ColumnHeader & {
  id: string;
  cell: (row: T) => ReactNode;
  /** Monospace text, for IDs and keys. */
  mono?: boolean;
  /** Tabular figures, for numbers, dates and times. */
  numeric?: boolean;
  /** The column takes the width that is left, and a long text ends in an ellipsis (`stacked={false}`). */
  grow?: boolean;
  /** The header is a button that sorts by this column. It needs `onSort` on the table. */
  sortable?: boolean;
};

export interface TableSort {
  columnId: string;
  direction: "asc" | "desc";
}

export interface DataTableProps<T> {
  /** The name of the table: its caption, for assistive technology only. */
  label: string;
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  /** The `id` of a row, so that a link can name it. A row with an id can take focus. */
  rowId?: (row: T) => string;
  /** The row that a link named: it has a mark for a moment. */
  linkedId?: string;
  /** The column that the rows are sorted by. The caller sorts the rows; the table only shows it. */
  sort?: TableSort;
  /** Called with the column of a pressed header. */
  onSort?: (columnId: string) => void;
  /**
   * Below 28rem each row becomes a two-line list row (the default). A table whose headers must stay
   * reachable, for example to sort, sets `false`: it keeps its columns and, in a container narrower
   * than they are, scrolls sideways inside itself, in a region that the keyboard reaches.
   */
  stacked?: boolean;
}

/** The text that names a column: its label, or its header when that is text. */
function nameOf<T>(column: Column<T>): string | undefined {
  return column.label ?? (typeof column.header === "string" ? column.header : undefined);
}

function ariaSort<T>(column: Column<T>, sort: TableSort | undefined) {
  if (sort?.columnId !== column.id) {
    return undefined;
  }
  return sort.direction === "asc" ? "ascending" : "descending";
}

/** The arrow of a sort button: the direction when the column is sorted, a hint on hover otherwise. */
function SortIcon({ sort }: { sort: TableSort | undefined }) {
  const Arrow = sort?.direction === "desc" ? IconArrowDown : IconArrowUp;
  return <Arrow className={styles.sortIcon} size={14} stroke={1.5} aria-hidden="true" />;
}

/**
 * A read-only table with a sticky header (doc/design/components.md, „Registers and tables“).
 * In a container below 28rem, each row is a two-line list row: the first column, then the other
 * columns with their names as visible labels, and so it never scrolls sideways (ADR 0023). A
 * table with `stacked={false}` keeps its columns and scrolls inside itself where they do not fit,
 * in a region that the keyboard reaches.
 */
export function DataTable<T>({
  label,
  columns,
  rows,
  rowKey,
  rowId,
  linkedId,
  sort,
  onSort,
  stacked = true,
}: DataTableProps<T>) {
  const table = (
    <table
      className={styles.table}
      data-stacked={stacked ? "" : undefined}
      data-fixed={stacked ? undefined : ""}
    >
      <caption className={styles.caption}>{label}</caption>
      <thead>
        <tr>
          {columns.map((column) => (
            <th
              key={column.id}
              scope="col"
              aria-label={column.label}
              aria-sort={ariaSort(column, sort)}
            >
              {column.sortable && onSort ? (
                <button type="button" className={styles.sort} onClick={() => onSort(column.id)}>
                  {column.header}
                  <SortIcon sort={sort?.columnId === column.id ? sort : undefined} />
                </button>
              ) : (
                column.header
              )}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const id = rowId?.(row);
          return (
            <tr
              key={rowKey(row)}
              id={id}
              tabIndex={id === undefined ? undefined : -1}
              data-linked={id !== undefined && id === linkedId ? "" : undefined}
            >
              {columns.map((column) => (
                <td
                  key={column.id}
                  data-mono={column.mono || undefined}
                  data-numeric={column.numeric || undefined}
                  data-grow={column.grow || undefined}
                >
                  {/* Shown only in a two-line row, where the column headers are not visible. */}
                  <span className={styles.label}>{nameOf(column)}</span>
                  {column.cell(row)}
                </td>
              ))}
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  return (
    <div className={styles.container}>
      {stacked ? (
        table
      ) : (
        // A region that scrolls takes focus, so that the keyboard reaches the columns it hides.
        // biome-ignore lint/a11y/noNoninteractiveTabindex: the region is a scroll container
        <section className={styles.scroller} aria-label={label} tabIndex={0}>
          {table}
        </section>
      )}
    </div>
  );
}
