import type { Icon } from "@tabler/icons-react";
import styles from "./StatusLabel.module.css";

export type StatusTone = "neutral" | "progress" | "warning" | "success" | "danger" | "muted";

export interface StatusLabelProps {
  icon: Icon;
  tone: StatusTone;
  children: string;
  /**
   * The label is one of several in a column, for example of a table. A label with a box then starts
   * its text at the same left edge as a label without a box.
   */
  column?: boolean;
}

/**
 * A workflow status: an icon and its text (doc/design/components.md). The text always shows;
 * the tone only supports it.
 */
export function StatusLabel({ icon: Icon, tone, children, column = false }: StatusLabelProps) {
  return (
    <span className={styles.label} data-tone={tone} data-column={column || undefined}>
      <Icon className={styles.icon} size={16} stroke={1.5} aria-hidden="true" />
      {children}
    </span>
  );
}
