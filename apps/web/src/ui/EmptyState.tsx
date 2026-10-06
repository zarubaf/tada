import type { ReactNode } from "react";
import styles from "./EmptyState.module.css";

export interface EmptyStateProps {
  title: string;
  text: string;
  /** The first action, if the member can take one here. */
  action?: ReactNode;
}

/** A title, one sentence and one action. No illustration (doc/design/components.md). */
export function EmptyState({ title, text, action }: EmptyStateProps) {
  return (
    <div className={styles.empty}>
      <p className={styles.title}>{title}</p>
      <p className={styles.text}>{text}</p>
      {action}
    </div>
  );
}
