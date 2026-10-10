import { t } from "../i18n";
import styles from "./Redacted.module.css";

/** The tombstone of a legally redacted value: „entfernt“, muted and italic (ADR 0072). */
export function Redacted() {
  return <span className={styles.redacted}>{t("redacted")}</span>;
}
