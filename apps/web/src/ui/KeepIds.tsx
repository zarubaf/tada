import type { ReactNode } from "react";
import styles from "./KeepIds.module.css";

/** A readable ID of a record: three or four capital letters, a hyphen and the number. */
const READABLE_ID = /\b([A-Z]{3,4}-\d{3,})\b/;

/**
 * A sentence in which a readable ID such as „DEC-001“ stays on one line: the hyphen is a place
 * where a line may break, and an ID that breaks there is read as two words.
 */
export function KeepIds({ children }: { children: string }): ReactNode {
  return children.split(READABLE_ID).map((part, index) =>
    index % 2 === 1 ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: the parts never reorder
      <span key={index} className={styles.id}>
        {part}
      </span>
    ) : (
      part
    ),
  );
}
