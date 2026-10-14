import type { ReactNode } from "react";
import { Toolbar as AriaToolbar } from "react-aria-components";
import styles from "./Toolbar.module.css";

export interface ToolbarProps {
  /** The accessible name of the toolbar. */
  label: string;
  children: ReactNode;
  /** Another look for the row, for example a bar that floats; the class wins over the default. */
  className?: string;
}

/**
 * The row of filter and display controls above a list (doc/design/components.md). The row is one
 * tab stop; the arrow keys move between its buttons and menus. A control that refuses a press
 * with `aria-disabled` stays in that order, so that its reason can be read.
 */
export function Toolbar({ label, children, className }: ToolbarProps) {
  return (
    <AriaToolbar
      className={className ? `${styles.toolbar} ${className}` : styles.toolbar}
      aria-label={label}
    >
      {children}
    </AriaToolbar>
  );
}
