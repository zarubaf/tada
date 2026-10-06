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
  /** The accessible name of the table. */
  label: string;
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
}

/** A read-only table with a sticky header (doc/design/components.md, „Registers and tables“). */
export function DataTable<T>({ label, columns, rows, rowKey }: DataTableProps<T>) {
  return (
    <div className={styles.container}>
      <table className={styles.table} aria-label={label}>
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
          {rows.map((row) => (
            <tr key={rowKey(row)}>
              {columns.map((column) => (
                <td
                  key={column.id}
                  data-mono={column.mono || undefined}
                  data-numeric={column.numeric || undefined}
                >
                  {column.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
