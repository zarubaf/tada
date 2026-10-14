import styles from "./RecordId.module.css";

/** The readable ID of a record, for example `ACT-042`, in the monospace face (ADR 0021). */
export function RecordId({ children }: { children: string }) {
  return <span className={styles.id}>{children}</span>;
}
