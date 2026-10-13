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
}

/**
 * A read-only table with a sticky header (doc/design/components.md, „Registers and tables“).
 * In a container below 28rem, each row is a two-line list row: the first column, then the other
 * columns with their names as visible labels. The table never scrolls sideways (ADR 0023).
 */
export function DataTable<T>({ label, columns, rows, rowKey, rowId, linkedId }: DataTableProps<T>) {
  return (
    <div className={styles.container}>
      <table className={styles.table}>
        <caption className={styles.caption}>{label}</caption>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.id} scope="col">
                {column.header}
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
