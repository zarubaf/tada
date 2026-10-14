import { IconArrowDown, IconArrowUp } from "@tabler/icons-react";
import type { ReactNode } from "react";
import styles from "./DataTable.module.css";

export interface Column<T> {
  id: string;
  header: string;
  cell: (row: T) => ReactNode;
  /** Monospace text, for IDs and keys. */
  mono?: boolean;
  /** Tabular figures, for numbers, dates and times. */
  numeric?: boolean;
  /** The column takes the width that is left, and a long text ends in an ellipsis (`stacked={false}`). */
  grow?: boolean;
  /** The header is a button that sorts by this column. It needs `onSort` on the table. */
  sortable?: boolean;
}

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
   * reachable, for example to sort, sets `false`: it keeps its columns and scrolls inside itself.
   */
  stacked?: boolean;
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
 * columns with their names as visible labels. The table never scrolls sideways (ADR 0023).
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
  return (
    <div className={styles.container} data-fixed={stacked ? undefined : ""}>
      <table
        className={styles.table}
        data-stacked={stacked ? "" : undefined}
        data-fixed={stacked ? undefined : ""}
      >
        <caption className={styles.caption}>{label}</caption>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.id} scope="col" aria-sort={ariaSort(column, sort)}>
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
                    <span className={styles.label}>{column.header}</span>
                    {column.cell(row)}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
