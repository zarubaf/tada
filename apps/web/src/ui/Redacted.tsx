import type { ReactNode } from "react";
import { t } from "../i18n";
import styles from "./Redacted.module.css";

/** The tombstone of a legally redacted value: „entfernt“, muted and medium weight, without italics (doc/design/tokens.md) (ADR 0072). */
export function Redacted() {
  return <span className={styles.redacted}>{t("redacted")}</span>;
}

/** The text of a record, or „entfernt“ if the record is redacted. The flag decides, not the text. */
export function redactedOr(redacted: boolean | undefined, text: ReactNode): ReactNode {
  return redacted ? <Redacted /> : text;
}
