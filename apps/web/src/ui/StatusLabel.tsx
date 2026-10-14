import type { Icon } from "@tabler/icons-react";
import { glyphIcon, type StatusGlyph, type StatusTone, ToneIcon } from "./StatusIcon";
import styles from "./StatusLabel.module.css";

export interface StatusLabelProps {
  /** A glyph of a record status (StatusIcon), or any other icon. */
  icon: Icon | StatusGlyph;
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
export function StatusLabel({ icon, tone, children, column = false }: StatusLabelProps) {
  return (
    <span className={styles.label} data-tone={tone} data-column={column || undefined}>
      {typeof icon === "string" ? (
        <ToneIcon icon={glyphIcon(icon)} tone={tone} stroke={2} />
      ) : (
        <ToneIcon icon={icon} tone={tone} />
      )}
      {children}
    </span>
  );
}
