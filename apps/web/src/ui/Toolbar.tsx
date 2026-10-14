import type { ReactNode } from "react";
import { Toolbar as AriaToolbar } from "react-aria-components";
import styles from "./Toolbar.module.css";

export interface ToolbarProps {
  /** The accessible name of the toolbar. */
  label: string;
  children: ReactNode;
}

/**
 * The row of filter and display controls above a list (doc/design/components.md). The row is one
 * tab stop; the arrow keys move between its buttons and menus.
 */
export function Toolbar({ label, children }: ToolbarProps) {
  return (
    <AriaToolbar className={styles.toolbar} aria-label={label}>
      {children}
    </AriaToolbar>
  );
}
