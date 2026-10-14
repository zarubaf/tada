import type { ReactNode } from "react";
import styles from "./PageToolbar.module.css";

/**
 * The toolbar row of a page (ADR 0082): one row of `--size-toolbar` with the controls of the page,
 * edge to edge with a border below. It sits inside a `Page` and takes the gutter of the page; it
 * wraps into a second row where the controls do not fit, and never overflows.
 */
export function PageToolbar({ children }: { children: ReactNode }) {
  // The attribute names the row for the measuring checks of the browser, which must not count parents.
  return (
    <div className={styles.row} data-page-toolbar="">
      {children}
    </div>
  );
}
